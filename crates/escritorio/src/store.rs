//! Estado editable de Escritorio: los ajustes guardados en `escritorio.lua`,
//! los cambios pendientes, el valor que pone Omarchy cuando Escritorio no
//! fija nada, y la aplicación segura (copia de seguridad, escritura
//! atómica, recarga de Hyprland y vuelta atrás si informa errores).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use lizarbe_core::form::FieldStore;
use lizarbe_core::fsutil::Transaction;
use lizarbe_core::hypr;
use lizarbe_core::schema::FieldDef;
use serde_json::Value;

use crate::catalog;
use crate::hyprfile::{self, Values};
use crate::paths::Paths;

/// Un ajuste: opción de Hyprland o ajuste propio `x:`.
#[derive(Debug, Clone, PartialEq)]
pub struct Bind(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub key: String,
    pub old: Option<Value>,
    pub new: Option<Value>,
}

#[derive(Debug, Default)]
pub struct ApplyReport {
    pub backups: Vec<PathBuf>,
    pub reloaded: bool,
    /// Errores de Hyprland; si hay, los archivos se devolvieron a su estado.
    pub errors: Vec<String>,
}

pub struct Store {
    pub paths: Paths,
    /// Ajustes con los cambios pendientes.
    pub values: Values,
    /// Ajustes guardados en disco.
    orig: Values,
    raw: Option<String>,
    /// Valores en uso según Hyprland al cargar.
    effective: HashMap<String, Value>,
    /// Hay un Hyprland al que preguntar y que recargar.
    pub live: bool,
    /// `escritorio.lua` existe pero no lo escribió Escritorio.
    pub foreign_file: bool,
    /// Meca sigue cargando `hyprland-gui.lua` después de Escritorio.
    pub meca_active: bool,
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

impl Store {
    pub fn load(paths: Paths) -> Store {
        let mut s = Store {
            values: Values::new(),
            orig: Values::new(),
            raw: None,
            effective: HashMap::new(),
            live: false,
            foreign_file: false,
            meca_active: false,
            paths,
        };
        s.reload();
        s
    }

    /// Relee todo y descarta los cambios pendientes.
    pub fn reload(&mut self) {
        self.raw = read(&self.paths.escritorio_lua());
        let parsed = self.raw.as_deref().map(hyprfile::parse);
        self.foreign_file = matches!(parsed, Some(None));
        self.orig = parsed.flatten().unwrap_or_default();
        self.values = self.orig.clone();
        self.meca_active = read(&self.paths.hyprland_lua())
            .is_some_and(|t| t.lines().any(|l| l.trim() == "require(\"hyprland-gui\")"));
        self.live = hypr::available();
        self.effective.clear();
        if self.live {
            let keys = catalog::hypr_keys();
            let names: Vec<&str> = keys.iter().map(String::as_str).collect();
            for (k, r) in keys.iter().zip(hypr::getoptions(&names)) {
                if let Ok(opt) = r {
                    self.effective.insert(k.clone(), opt.value.to_json());
                }
            }
        }
    }

    /// Valor que queda si Escritorio no fija `def`: el que Hyprland usa ahora
    /// (si Escritorio aún no lo había guardado) o el de Omarchy.
    pub fn base(&self, def: &FieldDef) -> Option<Value> {
        if !self.orig.contains_key(&def.key)
            && let Some(v) = self.effective.get(&def.key)
        {
            return Some(v.clone());
        }
        def.default.clone()
    }

    pub fn dirty(&self) -> bool {
        self.values != self.orig
    }

    pub fn discard(&mut self) {
        self.values = self.orig.clone();
    }

    /// Quita los ajustes `keys` (vuelven al valor de Omarchy).
    pub fn restore(&mut self, keys: &[String]) {
        for k in keys {
            self.values.remove(k);
        }
    }

    pub fn changes(&self) -> Vec<Change> {
        let mut keys: Vec<&String> = self.orig.keys().chain(self.values.keys()).collect();
        keys.sort();
        keys.dedup();
        keys.into_iter()
            .filter(|k| self.orig.get(*k) != self.values.get(*k))
            .map(|k| Change {
                key: k.clone(),
                old: self.orig.get(k).cloned(),
                new: self.values.get(k).cloned(),
            })
            .collect()
    }

    /// `escritorio.lua` cambió en disco desde que se cargó.
    pub fn external_change(&self) -> bool {
        read(&self.paths.escritorio_lua()) != self.raw
    }

    pub fn apply(&mut self) -> Result<ApplyReport> {
        let mut report = ApplyReport::default();
        let mut tx = Transaction::new(&self.paths.backup_dir);
        tx.write(
            &self.paths.escritorio_lua(),
            &hyprfile::render(&self.values),
        )?;
        let main = self.paths.hyprland_lua();
        if let Some(text) = read(&main).and_then(|t| hyprfile::with_require(&t)) {
            tx.write(&main, &text)?;
        }
        if self.live && !self.paths.sandbox {
            if let Err(errors) = hypr::reload_checked() {
                tx.rollback()?;
                let _ = hypr::reload();
                report.errors = errors;
                return Ok(report);
            }
            report.reloaded = true;
        }
        report.backups = tx.commit(30);
        self.reload();
        Ok(report)
    }
}

impl FieldStore<Bind> for Store {
    fn get(&self, bind: &Bind) -> Option<Value> {
        self.values.get(&bind.0).cloned()
    }

    fn get_original(&self, bind: &Bind) -> Option<Value> {
        self.orig.get(&bind.0).cloned()
    }

    /// Igualar el valor de Omarchy equivale a no fijarlo: así el archivo
    /// solo guarda lo que de verdad cambia.
    fn set_field(&mut self, bind: &Bind, value: Value, default: Option<&Value>) {
        if Some(&value) == default {
            self.values.remove(&bind.0);
        } else {
            self.values.insert(bind.0.clone(), value);
        }
    }

    fn unset(&mut self, bind: &Bind) {
        self.values.remove(&bind.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sandbox() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("hyprland.lua"),
            "require(\"hypr.looknfeel\")\nrequire(\"default.hypr.toggles\")\n",
        )
        .unwrap();
        let store = Store::load(Paths::detect(Some(dir.path().to_path_buf())));
        (dir, store)
    }

    #[test]
    fn stores_only_differences() {
        let (_d, mut s) = sandbox();
        let b = Bind("general:gaps_in".into());
        s.set_field(&b, json!(8), Some(&json!(5)));
        assert!(s.dirty());
        s.set_field(&b, json!(5), Some(&json!(5)));
        assert!(!s.dirty(), "volver al valor de Omarchy no guarda nada");
    }

    #[test]
    fn apply_writes_file_and_require() {
        let (d, mut s) = sandbox();
        s.set_field(&Bind("general:gaps_in".into()), json!(8), Some(&json!(5)));
        let report = s.apply().unwrap();
        assert!(!report.reloaded, "en sandbox no se recarga Hyprland");
        let text = std::fs::read_to_string(d.path().join("escritorio.lua")).unwrap();
        assert!(text.contains("gaps_in = 8"));
        let main = std::fs::read_to_string(d.path().join("hyprland.lua")).unwrap();
        assert!(main.contains("require(\"hypr.escritorio\")\nrequire(\"default.hypr.toggles\")"));
        assert!(!s.dirty());
        assert_eq!(
            s.get_original(&Bind("general:gaps_in".into())),
            Some(json!(8))
        );

        // Quitarlo y aplicar deja el archivo sin ajustes.
        s.unset(&Bind("general:gaps_in".into()));
        s.apply().unwrap();
        let text = std::fs::read_to_string(d.path().join("escritorio.lua")).unwrap();
        assert!(!text.contains("hl.config"));
    }

    #[test]
    fn detects_meca_and_foreign_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("hyprland.lua"),
            "require(\"hyprland-gui\")\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("escritorio.lua"), "-- a mano\n").unwrap();
        let s = Store::load(Paths::detect(Some(dir.path().to_path_buf())));
        assert!(s.meca_active);
        assert!(s.foreign_file);
    }

    /// Comprueba contra el Hyprland en marcha que cada ajuste del catálogo
    /// existe y acepta el tipo de valor que genera Escritorio. Evalúa los
    /// valores actuales (no cambia nada visible); conviene ejecutar
    /// `hyprctl reload` después. `cargo test -- --ignored`.
    #[test]
    #[ignore = "necesita una sesión de Hyprland"]
    fn catalog_is_valid_in_hyprland() {
        let keys = catalog::hypr_keys();
        let names: Vec<&str> = keys.iter().map(String::as_str).collect();
        let mut values = Values::new();
        for (k, r) in keys.iter().zip(hypr::getoptions(&names)) {
            let opt = r.unwrap_or_else(|e| panic!("{k}: {e}"));
            values.insert(k.clone(), opt.value.to_json());
        }
        hypr::eval(&hyprfile::render(&values)).expect("hl.config con todas las claves");

        let mut errors = vec![];
        for key in ["x:anim_windows", "x:anim_workspaces", "x:anim_speed"] {
            let def = catalog::find(key).unwrap();
            let lizarbe_core::schema::Kind::Enum(opts) = def.kind else {
                unreachable!()
            };
            for o in opts {
                let v: Values = [(key.to_string(), o.value.clone())].into();
                if let Err(e) = hypr::eval(&hyprfile::render(&v)) {
                    errors.push(format!("{key}={}: {e}", o.value));
                }
            }
        }
        let v: Values = [("x:ws_persistent".to_string(), json!(2))].into();
        if let Err(e) = hypr::eval(&hyprfile::render(&v)) {
            errors.push(format!("x:ws_persistent: {e}"));
        }
        assert!(errors.is_empty(), "{errors:#?}");
    }

    #[test]
    fn changes_list_old_and_new() {
        let (_d, mut s) = sandbox();
        s.set_field(
            &Bind("decoration:rounding".into()),
            json!(6),
            Some(&json!(0)),
        );
        let c = s.changes();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].old, None);
        assert_eq!(c[0].new, Some(json!(6)));
        s.restore(&["decoration:rounding".to_string()]);
        assert!(!s.dirty());
    }
}
