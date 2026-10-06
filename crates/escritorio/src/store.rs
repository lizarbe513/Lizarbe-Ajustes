//! Estado editable de Escritorio: los ajustes guardados en `escritorio.lua`,
//! los cambios pendientes, el valor que pone Omarchy cuando Escritorio no
//! fija nada, y la aplicación segura (copia de seguridad, escritura
//! atómica, recarga de Hyprland y vuelta atrás si informa errores).
//!
//! Casos especiales de claves:
//! - `kbopt:compose` y `kbopt:grp` son partes de la opción `input:kb_options`.
//! - `m:scale` es la escala general de `monitors.lua` (no se guarda en
//!   `escritorio.lua`, así el atajo de escala de Omarchy sigue funcionando).
//! - `x:mon:<pantalla>:<campo>` generan una regla `hl.monitor` completa.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use lizarbe_core::form::FieldStore;
use lizarbe_core::fsutil::Transaction;
use lizarbe_core::schema::FieldDef;
use lizarbe_core::{hypr, ipc};
use serde_json::{Value, json};

use crate::catalog::{self, Ctx, mode_value, monitor_key};
use crate::hyprfile::{self, RenderCtx, Values};
use crate::paths::Paths;

/// Un ajuste: opción de Hyprland o ajuste propio.
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
    /// Pantallas conectadas y temas de cursor instalados.
    pub ctx: Ctx,
    /// Escala general de `monitors.lua`; `None` si ya no tiene la variable.
    mon_scale: Option<Value>,
    /// Tema y tamaño del cursor en uso.
    cursor: (String, i64),
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Parte de `input:kb_options` que edita una clave `kbopt:`.
fn kb_prefix(key: &str) -> Option<&'static str> {
    match key {
        "kbopt:compose" => Some("compose:"),
        "kbopt:grp" => Some("grp:"),
        _ => None,
    }
}

/// `"compose:rwin,shift:both"` + `"compose:"` → `"rwin"` (o `"none"`).
pub fn kb_token(opts: &str, prefix: &str) -> String {
    opts.split(',')
        .find_map(|p| p.trim().strip_prefix(prefix))
        .unwrap_or("none")
        .to_string()
}

/// `opts` con la parte `prefix` cambiada a `token` (`"none"` la quita). La
/// tecla Compose va primero, como en Omarchy.
pub fn kb_with(opts: &str, prefix: &str, token: &str) -> String {
    let mut parts: Vec<String> = opts
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.starts_with(prefix))
        .map(String::from)
        .collect();
    if token != "none" && !token.is_empty() {
        let part = format!("{prefix}{token}");
        if prefix == "compose:" {
            parts.insert(0, part);
        } else {
            parts.push(part);
        }
    }
    parts.join(",")
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
            ctx: Ctx::default(),
            mon_scale: None,
            cursor: ("default".into(), 24),
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
        self.mon_scale =
            read(&self.paths.monitors_lua()).and_then(|t| hyprfile::monitor_locals(&t).0);
        self.cursor = (
            std::env::var("XCURSOR_THEME")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "default".into()),
            std::env::var("XCURSOR_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(24),
        );
        self.ctx.cursor_themes = catalog::cursor_themes();
        self.live = hypr::available();
        self.effective.clear();
        self.ctx.monitors.clear();
        if self.live {
            self.ctx.monitors = hypr::monitors(false).unwrap_or_default();
            let keys = catalog::hypr_keys();
            let names: Vec<&str> = keys.iter().map(String::as_str).collect();
            let plain = Ctx::default();
            for (k, r) in keys.iter().zip(hypr::getoptions(&names)) {
                if let Ok(opt) = r {
                    let v = opt.value.to_json();
                    let v = match catalog::find(k, &plain) {
                        Some(def) => catalog::coerce(&def.kind, v),
                        None => v,
                    };
                    self.effective.insert(k.clone(), v);
                }
            }
        }
    }

    /// Valor que queda si Escritorio no fija `def`: el que se usa ahora (si
    /// Escritorio aún no lo había guardado) o el de Omarchy.
    pub fn base(&self, def: &FieldDef) -> Option<Value> {
        let key = def.key.as_str();
        if let Some(prefix) = kb_prefix(key) {
            return Some(json!(kb_token(&self.kb_base(), prefix)));
        }
        if key == "m:scale" {
            return self.mon_scale.clone().or_else(|| def.default.clone());
        }
        if !self.orig.contains_key(key) {
            match key {
                "x:cursor_theme" => return Some(json!(self.cursor.0)),
                "x:cursor_size" => return Some(json!(self.cursor.1)),
                _ => {}
            }
            if let Some((name, part)) = monitor_key(key)
                && let Some(m) = self.ctx.monitors.iter().find(|m| m.name == name)
            {
                return Some(match part {
                    "mode" => json!(mode_value(m.width, m.height, m.refresh_rate)),
                    "scale" => json!(m.scale),
                    "transform" => json!(m.transform),
                    _ => json!("auto"),
                });
            }
            if let Some(v) = self.effective.get(key) {
                return Some(v.clone());
            }
        }
        def.default.clone()
    }

    /// `input:kb_options` sin los cambios de Escritorio.
    fn kb_base(&self) -> String {
        self.base(&catalog::kb_options())
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    }

    /// `monitors.lua` todavía tiene la escala general de Omarchy.
    pub fn global_scale_editable(&self) -> bool {
        self.mon_scale.is_some()
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
            match kb_prefix(k) {
                Some(_) => FieldStore::unset(self, &Bind(k.clone())),
                None => {
                    self.values.remove(k);
                }
            }
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
        let ctx = RenderCtx {
            monitors: self.ctx.monitors.clone(),
            global_scale: self
                .values
                .get("m:scale")
                .cloned()
                .or_else(|| self.mon_scale.clone())
                .unwrap_or(json!("auto")),
        };
        let cursor_changed = self
            .changes()
            .iter()
            .any(|c| c.key.starts_with("x:cursor_"));
        let cursor = (
            self.values
                .get("x:cursor_theme")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| self.cursor.0.clone()),
            self.values
                .get("x:cursor_size")
                .and_then(Value::as_i64)
                .unwrap_or(self.cursor.1),
        );

        let mut tx = Transaction::new(&self.paths.backup_dir);
        tx.write(
            &self.paths.escritorio_lua(),
            &hyprfile::render(&self.values, &ctx),
        )?;
        let main = self.paths.hyprland_lua();
        if let Some(text) = read(&main).and_then(|t| hyprfile::with_require(&t)) {
            tx.write(&main, &text)?;
        }
        if let Some(scale) = self.values.get("m:scale") {
            let monitors = self.paths.monitors_lua();
            if let Some(text) =
                read(&monitors).and_then(|t| hyprfile::with_monitor_scale(&t, scale))
            {
                tx.write(&monitors, &text)?;
            }
        }
        if self.live && !self.paths.sandbox {
            if let Err(errors) = hypr::reload_checked() {
                tx.rollback()?;
                let _ = hypr::reload();
                report.errors = errors;
                return Ok(report);
            }
            report.reloaded = true;
            if cursor_changed {
                apply_cursor(&cursor.0, cursor.1);
            }
        }
        report.backups = tx.commit(30);
        self.reload();
        Ok(report)
    }
}

/// El tema del cursor cambia al momento en Hyprland y en las aplicaciones GTK.
fn apply_cursor(theme: &str, size: i64) {
    let size = size.to_string();
    let _ = ipc::run("hyprctl", &["setcursor", theme, &size]);
    let gs = "org.gnome.desktop.interface";
    let _ = ipc::run("gsettings", &["set", gs, "cursor-theme", theme]);
    let _ = ipc::run("gsettings", &["set", gs, "cursor-size", &size]);
}

impl FieldStore<Bind> for Store {
    fn get(&self, bind: &Bind) -> Option<Value> {
        if let Some(prefix) = kb_prefix(&bind.0) {
            return self
                .values
                .get("input:kb_options")
                .and_then(Value::as_str)
                .map(|o| json!(kb_token(o, prefix)));
        }
        self.values.get(&bind.0).cloned()
    }

    fn get_original(&self, bind: &Bind) -> Option<Value> {
        if let Some(prefix) = kb_prefix(&bind.0) {
            return self
                .orig
                .get("input:kb_options")
                .and_then(Value::as_str)
                .map(|o| json!(kb_token(o, prefix)));
        }
        self.orig.get(&bind.0).cloned()
    }

    /// Igualar el valor de Omarchy equivale a no fijarlo: así el archivo
    /// solo guarda lo que de verdad cambia.
    fn set_field(&mut self, bind: &Bind, value: Value, default: Option<&Value>) {
        if let Some(prefix) = kb_prefix(&bind.0) {
            let base = self.kb_base();
            let current = self
                .values
                .get("input:kb_options")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| base.clone());
            let new = kb_with(&current, prefix, value.as_str().unwrap_or("none"));
            if new == base {
                self.values.remove("input:kb_options");
            } else {
                self.values.insert("input:kb_options".into(), json!(new));
            }
            return;
        }
        if Some(&value) == default {
            self.values.remove(&bind.0);
        } else {
            self.values.insert(bind.0.clone(), value);
        }
    }

    fn unset(&mut self, bind: &Bind) {
        if let Some(prefix) = kb_prefix(&bind.0) {
            let token = kb_token(&self.kb_base(), prefix);
            return self.set_field(bind, json!(token), None);
        }
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
        hypr::eval(&hyprfile::render(&values, &RenderCtx::default()))
            .expect("hl.config con todas las claves");

        let mut errors = vec![];
        for key in ["x:anim_windows", "x:anim_workspaces", "x:anim_speed"] {
            let def = catalog::find(key, &Ctx::default()).unwrap();
            let lizarbe_core::schema::Kind::Enum(opts) = def.kind else {
                unreachable!()
            };
            for o in opts {
                let v: Values = [(key.to_string(), o.value.clone())].into();
                if let Err(e) = hypr::eval(&hyprfile::render(&v, &RenderCtx::default())) {
                    errors.push(format!("{key}={}: {e}", o.value));
                }
            }
        }
        let v: Values = [("x:ws_persistent".to_string(), json!(2))].into();
        if let Err(e) = hypr::eval(&hyprfile::render(&v, &RenderCtx::default())) {
            errors.push(format!("x:ws_persistent: {e}"));
        }
        assert!(errors.is_empty(), "{errors:#?}");
    }

    #[test]
    fn edits_parts_of_kb_options() {
        assert_eq!(
            kb_token("compose:rwin,shift:both_capslock_cancel", "compose:"),
            "rwin"
        );
        assert_eq!(kb_token(",shift:both_capslock_cancel", "compose:"), "none");
        assert_eq!(
            kb_with(",shift:both_capslock_cancel", "compose:", "menu"),
            "compose:menu,shift:both_capslock_cancel"
        );
        assert_eq!(
            kb_with("compose:caps,grp:alts_toggle", "grp:", "none"),
            "compose:caps"
        );
        assert_eq!(
            kb_with("compose:caps", "grp:", "win_space_toggle"),
            "compose:caps,grp:win_space_toggle"
        );
    }

    #[test]
    fn kb_option_parts_are_sparse() {
        let (_d, mut s) = sandbox();
        let b = Bind("kbopt:compose".into());
        let base = s
            .base(&catalog::find("kbopt:compose", &s.ctx).unwrap())
            .unwrap();
        s.set_field(&b, json!("menu"), None);
        assert_eq!(FieldStore::get(&s, &b), Some(json!("menu")));
        assert!(s.values.contains_key("input:kb_options"));
        s.set_field(&b, base, None);
        assert!(!s.dirty(), "volver a la tecla de antes no guarda nada");
    }

    #[test]
    fn global_scale_goes_to_monitors_lua() {
        let (d, mut s) = sandbox();
        std::fs::write(
            d.path().join("monitors.lua"),
            "local omarchy_gdk_scale = 2\nlocal omarchy_monitor_scale = \"auto\"\n",
        )
        .unwrap();
        s.reload();
        assert!(s.global_scale_editable());
        s.set_field(&Bind("m:scale".into()), json!(1.25), Some(&json!("auto")));
        s.apply().unwrap();
        let mon = std::fs::read_to_string(d.path().join("monitors.lua")).unwrap();
        assert!(mon.contains("local omarchy_monitor_scale = 1.25"));
        assert!(mon.contains("local omarchy_gdk_scale = 1"));
        let esc = std::fs::read_to_string(d.path().join("escritorio.lua")).unwrap();
        assert!(!esc.contains("m:scale"));
        assert!(!s.dirty(), "tras aplicar, la escala sale de monitors.lua");
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
