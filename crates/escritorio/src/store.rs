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

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lizarbe_core::form::FieldStore;
use lizarbe_core::fsutil::Transaction;
use lizarbe_core::schema::FieldDef;
use lizarbe_core::{hypr, ipc};
use serde_json::{Value, json};

use crate::autostart;
use crate::binds;
use crate::capture;
use crate::catalog::{self, Ctx, mode_value, monitor_key};
use crate::hyprfile::{self, CustomBind, RenderCtx, Values};
use crate::i18n::t;
use crate::migrate;
use crate::paths::Paths;
use crate::sunset::{self, Night};
use crate::xcompose;

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
    /// Avisos de lo que no se pudo hacer, sin deshacer nada (idioma del menú o del sistema).
    pub warnings: Vec<String>,
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
    /// Escala de GTK de `monitors.lua` (`omarchy_gdk_scale`).
    mon_gdk: Option<i64>,
    /// Tema y tamaño del cursor en uso.
    cursor: (String, i64),
    /// Atajos en uso (con su acción, si se puede mover).
    pub binds: Vec<binds::Entry>,
    /// Programas de inicio con los cambios pendientes.
    pub autostart: Vec<autostart::Entry>,
    autostart_orig: Vec<autostart::Entry>,
    autostart_raw: Option<String>,
    /// Horario de la luz nocturna en `hyprsunset.conf` (o el de fábrica).
    night: Night,
    /// Atajos de texto con los cambios pendientes.
    pub xcompose: Vec<xcompose::Entry>,
    xcompose_orig: Vec<xcompose::Entry>,
    xcompose_raw: Option<String>,
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Resultado de importar los ajustes de Meca.
#[derive(Debug, Default)]
pub struct MigrateReport {
    pub settings: usize,
    pub binds: usize,
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
            mon_gdk: None,
            cursor: ("default".into(), 24),
            binds: vec![],
            autostart: vec![],
            autostart_orig: vec![],
            autostart_raw: None,
            night: Night::default(),
            xcompose: vec![],
            xcompose_orig: vec![],
            xcompose_raw: None,
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
        (self.mon_scale, self.mon_gdk) = read(&self.paths.monitors_lua())
            .map(|t| hyprfile::monitor_locals(&t))
            .unwrap_or_default();
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
        self.autostart_raw = read(&self.paths.autostart_lua());
        self.autostart_orig = self
            .autostart_raw
            .as_deref()
            .map(autostart::parse)
            .unwrap_or_default();
        self.autostart = self.autostart_orig.clone();
        self.night = read(&self.paths.hyprsunset_conf())
            .and_then(|t| sunset::parse(&t))
            .unwrap_or_default();
        self.xcompose_raw = read(&self.paths.xcompose());
        self.xcompose_orig = self
            .xcompose_raw
            .as_deref()
            .map(xcompose::parse)
            .unwrap_or_default();
        self.xcompose = self.xcompose_orig.clone();
        self.live = hypr::available();
        self.binds.clear();
        self.effective.clear();
        self.ctx.monitors.clear();
        if self.live {
            self.ctx.monitors = hypr::monitors(false).unwrap_or_default();
            let sources = binds::sources(&self.paths.omarchy_path, &self.paths.bindings_lua());
            self.binds = binds::entries(&binds::live(), &sources);
            // Los atajos que Escritorio ya movió o quitó no salen en la lista
            // en uso: se recuperan de los archivos para poder mostrarlos.
            for key in self.orig.keys().filter_map(|k| k.strip_prefix("x:bind:")) {
                if !self.binds.iter().any(|b| b.keys == key)
                    && let Some(src) = sources.iter().find(|s| s.keys == key)
                {
                    self.binds.push(binds::Entry {
                        keys: src.keys.clone(),
                        desc: src.desc.clone(),
                        origin: src.origin.clone(),
                        action: src.action.clone(),
                    });
                }
            }
            // Y los que Escritorio puso en su lugar no se repiten.
            let placed: Vec<String> = self
                .orig
                .iter()
                .filter(|(k, _)| k.starts_with("x:bind:"))
                .filter_map(|(_, v)| v.as_str().map(String::from))
                .chain(
                    hyprfile::custom_binds(&self.orig)
                        .into_iter()
                        .map(|c| c.keys),
                )
                .collect();
            self.binds.retain(|b| {
                !placed.contains(&b.keys) || self.orig.contains_key(&format!("x:bind:{}", b.keys))
            });
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
        match key {
            "n:on" => {
                return Some(json!(
                    self.autostart_orig
                        .iter()
                        .any(|e| e.cmd == "hyprsunset" && e.enabled)
                ));
            }
            "lg:ui" => return Some(json!(lizarbe_core::i18n::lang().code())),
            "n:start" => return Some(json!(self.night.start)),
            "n:end" => return Some(json!(self.night.end)),
            "n:temp" => return Some(json!(self.night.temp)),
            _ => {}
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

    /// Hay ajustes de Meca (`hyprland-gui.lua`) que importar.
    pub fn meca_pending(&self) -> bool {
        self.paths.gui_lua().is_file()
    }

    /// Importa lo que Meca tenía guardado, lo aplica y retira
    /// `hyprland-gui.lua` (con copia de seguridad) para que deje de pisar a
    /// Escritorio. Si Hyprland rechaza el resultado, todo vuelve a como estaba.
    pub fn migrate_meca(&mut self) -> Result<MigrateReport> {
        let gui = read(&self.paths.gui_lua()).unwrap_or_default();
        let mut report = MigrateReport::default();
        let imported = migrate::settings_from_gui(&gui);
        report.settings = imported.len();
        for (k, v) in imported {
            self.values.entry(k).or_insert(v);
        }
        let bindings_text = read(&self.paths.bindings_lua()).unwrap_or_default();
        let binds = migrate::parse_binds(&bindings_text);
        let mut custom = self.custom_binds();
        for c in &binds.custom {
            if !custom.iter().any(|x| x.keys == c.keys) {
                custom.push(c.clone());
                report.binds += 1;
            }
        }
        self.set_custom_binds(&custom);
        for (orig, new) in &binds.overrides {
            let v = new.as_ref().map_or(Value::Null, |k| json!(k));
            self.values.entry(format!("x:bind:{orig}")).or_insert(v);
            report.binds += 1;
        }

        // 1) Escritorio guarda lo importado (Meca sigue cargado: sin cambios visibles).
        let r = self.apply()?;
        if !r.errors.is_empty() {
            anyhow::bail!(r.errors.join("\n"));
        }

        // 2) Se retira Meca de la carga y de bindings.lua.
        let mut tx = Transaction::new(&self.paths.backup_dir);
        let main = self.paths.hyprland_lua();
        if let Some(text) = read(&main) {
            tx.write(&main, &migrate::without_gui_require(&text))?;
        }
        if !binds.custom.is_empty() || bindings_text.contains("MECA_KEYBINDS_META") {
            tx.write(
                &self.paths.bindings_lua(),
                &migrate::clean_bindings(&bindings_text, &binds),
            )?;
        }
        if self.live
            && !self.paths.sandbox
            && let Err(errors) = hypr::reload_checked()
        {
            tx.rollback()?;
            let _ = hypr::reload();
            anyhow::bail!(errors.join("\n"));
        }
        tx.commit(30);
        let stamp = lizarbe_core::fsutil::stamp();
        lizarbe_core::fsutil::backup(&self.paths.gui_lua(), &self.paths.backup_dir, &stamp)?;
        std::fs::remove_file(self.paths.gui_lua())?;
        self.reload();
        Ok(report)
    }

    /// `monitors.lua` todavía tiene la escala general de Omarchy.
    pub fn global_scale_editable(&self) -> bool {
        self.mon_scale.is_some()
    }

    /// Escala general pendiente o la de `monitors.lua`.
    fn global_scale(&self) -> Option<Value> {
        self.values
            .get("m:scale")
            .cloned()
            .or_else(|| self.mon_scale.clone())
    }

    /// Corrección pendiente de `GDK_SCALE` cuando no sigue a la escala real
    /// (la plantilla de Omarchy trae 2 aunque la pantalla use 1).
    pub fn gdk_change(&self) -> Option<Change> {
        let live: Vec<f64> = self.ctx.monitors.iter().map(|m| m.scale).collect();
        let want = hyprfile::gdk_for(&self.global_scale()?, &live)?;
        let have = self.mon_gdk?;
        (want != have).then(|| Change {
            key: "m:gdk".into(),
            old: Some(json!(have)),
            new: Some(json!(want)),
        })
    }

    pub fn dirty(&self) -> bool {
        self.gdk_change().is_some()
            || self.values != self.orig
            || self.autostart != self.autostart_orig
            || self.xcompose != self.xcompose_orig
    }

    pub fn discard(&mut self) {
        self.values = self.orig.clone();
        self.autostart = self.autostart_orig.clone();
        self.xcompose = self.xcompose_orig.clone();
    }

    pub fn discard_compose(&mut self) {
        self.xcompose = self.xcompose_orig.clone();
    }

    /// Deshace los cambios pendientes del inicio automático.
    pub fn discard_autostart(&mut self) {
        self.autostart = self.autostart_orig.clone();
    }

    pub fn custom_binds(&self) -> Vec<CustomBind> {
        hyprfile::custom_binds(&self.values)
    }

    pub fn set_custom_binds(&mut self, list: &[CustomBind]) {
        if list.is_empty() {
            self.values.remove("x:custom_binds");
        } else {
            self.values.insert(
                "x:custom_binds".into(),
                Value::Array(list.iter().map(CustomBind::to_json).collect()),
            );
        }
    }

    /// Teclas que quedarán en uso con los cambios pendientes, con la
    /// descripción del atajo que las usa.
    pub fn keys_in_use(&self) -> Vec<(String, String)> {
        let mut out = vec![];
        for b in &self.binds {
            match self.values.get(&format!("x:bind:{}", b.keys)) {
                None => out.push((b.keys.clone(), b.desc.clone())),
                Some(Value::String(new)) => out.push((new.clone(), b.desc.clone())),
                Some(_) => {}
            }
        }
        for c in self.custom_binds() {
            out.push((c.keys, c.desc));
        }
        out
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
        let mut out: Vec<Change> = keys
            .into_iter()
            .filter(|k| self.orig.get(*k) != self.values.get(*k))
            .map(|k| Change {
                key: k.clone(),
                old: self.orig.get(k).cloned(),
                new: self.values.get(k).cloned(),
            })
            .collect();
        // Inicio automático: un cambio por programa (activado, desactivado,
        // añadido o quitado).
        for o in &self.autostart_orig {
            let now = self.autostart.iter().find(|e| e.line == o.line);
            if now.map(|e| e.enabled) != Some(o.enabled) {
                out.push(Change {
                    key: format!("as:{}", o.cmd),
                    old: Some(Value::Bool(o.enabled)),
                    new: now.map(|e| Value::Bool(e.enabled)),
                });
            }
        }
        for e in self.autostart.iter().filter(|e| e.line.is_none()) {
            out.push(Change {
                key: format!("as:{}", e.cmd),
                old: None,
                new: Some(Value::Bool(e.enabled)),
            });
        }
        for o in &self.xcompose_orig {
            let now = self.xcompose.iter().find(|e| e.line == o.line);
            if now.map(|e| &e.text) != Some(&o.text) {
                out.push(Change {
                    key: format!("xc:{}", o.keys),
                    old: Some(json!(o.text)),
                    new: now.map(|e| json!(e.text)),
                });
            }
        }
        for e in self.xcompose.iter().filter(|e| e.line.is_none()) {
            out.push(Change {
                key: format!("xc:{}", e.keys),
                old: None,
                new: Some(json!(e.text)),
            });
        }
        out.extend(self.gdk_change());
        out
    }

    /// `escritorio.lua` cambió en disco desde que se cargó.
    pub fn external_change(&self) -> bool {
        read(&self.paths.escritorio_lua()) != self.raw
    }

    pub fn apply(&mut self) -> Result<ApplyReport> {
        let mut report = ApplyReport::default();
        let bind_actions = self
            .binds
            .iter()
            .filter_map(|b| {
                b.action
                    .clone()
                    .map(|a| (b.keys.clone(), (b.desc.clone(), a)))
            })
            .collect();
        let ctx = RenderCtx {
            bind_actions,
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
        // Luz nocturna: horario en hyprsunset.conf y arranque con la sesión.
        let night_changed = self.values.keys().any(|k| k.starts_with("n:"));
        if night_changed {
            let text = |k: &str, d: &str| {
                self.values
                    .get(k)
                    .and_then(Value::as_str)
                    .unwrap_or(d)
                    .to_string()
            };
            let night = Night {
                start: text("n:start", &self.night.start),
                end: text("n:end", &self.night.end),
                temp: self
                    .values
                    .get("n:temp")
                    .and_then(Value::as_i64)
                    .unwrap_or(self.night.temp),
            };
            tx.write(&self.paths.hyprsunset_conf(), &sunset::render(&night))?;
            if let Some(on) = self.values.get("n:on").and_then(Value::as_bool) {
                match self.autostart.iter_mut().find(|e| e.cmd == "hyprsunset") {
                    Some(e) => e.enabled = on,
                    None if on => self
                        .autostart
                        .push(autostart::Entry::new("hyprsunset", false)),
                    None => {}
                }
            }
        }
        if self.autostart != self.autostart_orig {
            let text =
                autostart::render(self.autostart_raw.as_deref().unwrap_or(""), &self.autostart);
            tx.write(&self.paths.autostart_lua(), &text)?;
        }
        // Capturas: carpeta y editor también para el resto de la sesión.
        let env_vars = capture::env_vars(&self.values);
        let env_before = capture::env_vars(&self.orig);
        if env_vars != env_before {
            let file = self.paths.uwsm_env();
            let current = read(&file).unwrap_or_default();
            let new = capture::render_env(&current, &env_vars);
            if new != current {
                tx.write(&file, &new)?;
            }
            report.warnings.push(t("cap.relogin"));
        }
        let compose_changed = self.xcompose != self.xcompose_orig;
        if compose_changed {
            let text = xcompose::render(self.xcompose_raw.as_deref().unwrap_or(""), &self.xcompose);
            tx.write(&self.paths.xcompose(), &text)?;
        }
        let gdk = self.gdk_change().and_then(|c| c.new?.as_i64());
        if (self.values.contains_key("m:scale") || gdk.is_some())
            && let Some(scale) = self.global_scale()
        {
            let live: Vec<f64> = self.ctx.monitors.iter().map(|m| m.scale).collect();
            let gdk = gdk.or_else(|| hyprfile::gdk_for(&scale, &live));
            let monitors = self.paths.monitors_lua();
            if let Some(text) =
                read(&monitors).and_then(|t| hyprfile::with_monitor_scale(&t, &scale, gdk))
            {
                tx.write(&monitors, &text)?;
                if gdk != self.mon_gdk {
                    report.warnings.push(t("gdk.relogin"));
                }
            }
        }
        let language = self
            .values
            .get("lg:ui")
            .and_then(Value::as_str)
            .map(String::from);
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
            if night_changed {
                let _ = ipc::run("omarchy-restart-hyprsunset", &[]);
            }
        }
        if let Some(code) = language
            && !self.paths.sandbox
        {
            report.warnings.extend(apply_language(&code));
        }
        if compose_changed && !self.paths.sandbox {
            let _ = ipc::run("omarchy-restart-xcompose", &[]);
        }
        report.backups = tx.commit(30);
        self.reload();
        Ok(report)
    }

    /// Archivo que Escritorio escribe con ese nombre (las copias guardan solo
    /// el nombre).
    fn target_of(&self, name: &str) -> Option<PathBuf> {
        let p = &self.paths;
        Some(match name {
            "escritorio.lua" => p.escritorio_lua(),
            "hyprland.lua" => p.hyprland_lua(),
            "monitors.lua" => p.monitors_lua(),
            "autostart.lua" => p.autostart_lua(),
            "bindings.lua" => p.bindings_lua(),
            "hyprsunset.conf" => p.hyprsunset_conf(),
            ".XCompose" => p.xcompose(),
            "env" => p.uwsm_env(),
            _ => return None,
        })
    }

    /// Cambios aplicados que se pueden deshacer, del más reciente al más
    /// antiguo. Cada uno tiene la copia de los archivos de justo antes.
    pub fn undo_points(&self) -> Vec<UndoPoint> {
        let Ok(rd) = std::fs::read_dir(&self.paths.backup_dir) else {
            return vec![];
        };
        let mut points: BTreeMap<String, Vec<(PathBuf, PathBuf)>> = BTreeMap::new();
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some((file, stamp)) = name.rsplit_once('.') else {
                continue;
            };
            if stamp.len() != 15 || !stamp.chars().all(|c| c.is_ascii_digit() || c == '-') {
                continue;
            }
            if let Some(target) = self.target_of(file) {
                points
                    .entry(stamp.to_string())
                    .or_default()
                    .push((e.path(), target));
            }
        }
        points
            .into_iter()
            .rev()
            .map(|(stamp, mut files)| {
                files.sort();
                UndoPoint { stamp, files }
            })
            .collect()
    }

    /// Devuelve los archivos a como estaban antes de ese cambio. Lo actual se
    /// guarda antes, así que también se puede deshacer.
    pub fn undo(&mut self, point: &UndoPoint) -> Result<ApplyReport> {
        let mut report = ApplyReport::default();
        let mut tx = Transaction::new(&self.paths.backup_dir);
        for (backup, target) in &point.files {
            let text =
                std::fs::read_to_string(backup).with_context(|| format!("{}", backup.display()))?;
            tx.write(target, &text)?;
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

/// Un cambio aplicado: su fecha (`20261007-171306`) y, por cada archivo, la
/// copia de antes y dónde va.
#[derive(Debug, Clone, PartialEq)]
pub struct UndoPoint {
    pub stamp: String,
    pub files: Vec<(PathBuf, PathBuf)>,
}

impl UndoPoint {
    /// `20261007-171306` → `07/10/2026 17:13:06`.
    pub fn when(&self) -> String {
        let s = &self.stamp;
        if s.len() != 15 {
            return s.clone();
        }
        format!(
            "{}/{}/{} {}:{}:{}",
            &s[6..8],
            &s[4..6],
            &s[0..4],
            &s[9..11],
            &s[11..13],
            &s[13..15]
        )
    }

    pub fn names(&self) -> String {
        self.files
            .iter()
            .filter_map(|(_, t)| t.file_name()?.to_str().map(String::from))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Idioma de Lizarbe: lo guarda para las apps (Escritorio y Widgets), traduce el
/// menú de Omarchy y cambia el idioma del sistema (`localectl`, aplica al volver
/// a iniciar sesión). Devuelve los avisos de lo que no se pudo.
fn apply_language(code: &str) -> Vec<String> {
    use lizarbe_core::i18n::{Lang, set_lang};
    let Some(lang) = Lang::parse(code) else {
        return vec![];
    };
    let mut prefs = lizarbe_core::prefs::Prefs::load();
    prefs.lang = lang.code().to_string();
    prefs.save();
    set_lang(lang);
    let mut errors = vec![];
    if ipc::run("lizarbe-menu-sync", &["--quiet", "--lang", code]).is_err() {
        errors.push(crate::i18n::t("lang.err_menu"));
    }
    let locale = if code == "es" {
        "es_ES.UTF-8"
    } else {
        "en_US.UTF-8"
    };
    if ipc::run("localectl", &["set-locale", &format!("LANG={locale}")]).is_err() {
        errors.push(crate::i18n::t("lang.err_system"));
    }
    errors
}

/// El tema del cursor cambia al momento en Hyprland y en las aplicaciones GTK.
fn apply_cursor(theme: &str, size: i64) {
    let size = size.to_string();
    let _ = ipc::run("hyprctl", &["setcursor", theme, &size]);
    let gs = "org.gnome.desktop.interface";
    let _ = ipc::run("gsettings", &["set", gs, "cursor-theme", theme]);
    let _ = ipc::run("gsettings", &["set", gs, "cursor-size", &size]);
}

/// Índice de un programa de inicio en una clave `as:<i>`.
fn autostart_index(key: &str) -> Option<usize> {
    key.strip_prefix("as:")?.parse().ok()
}

/// Índice de un atajo de texto en una clave `xc:<i>`.
fn xcompose_index(key: &str) -> Option<usize> {
    key.strip_prefix("xc:")?.parse().ok()
}

impl FieldStore<Bind> for Store {
    fn get(&self, bind: &Bind) -> Option<Value> {
        if let Some(i) = xcompose_index(&bind.0) {
            return self.xcompose.get(i).map(|e| json!(e.text));
        }
        if let Some(i) = autostart_index(&bind.0) {
            return self.autostart.get(i).map(|e| Value::Bool(e.enabled));
        }
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
        if let Some(i) = xcompose_index(&bind.0) {
            let e = self.xcompose.get(i)?;
            return self
                .xcompose_orig
                .iter()
                .find(|o| o.line.is_some() && o.line == e.line)
                .map(|o| json!(o.text));
        }
        if let Some(i) = autostart_index(&bind.0) {
            let e = self.autostart.get(i)?;
            return self
                .autostart_orig
                .iter()
                .find(|o| o.line.is_some() && o.line == e.line)
                .map(|o| Value::Bool(o.enabled));
        }
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
        if let Some(i) = xcompose_index(&bind.0) {
            if let (Some(e), Some(t)) = (self.xcompose.get_mut(i), value.as_str()) {
                e.text = t.to_string();
            }
            return;
        }
        if let Some(i) = autostart_index(&bind.0) {
            if let Some(e) = self.autostart.get_mut(i) {
                e.enabled = value.as_bool().unwrap_or(e.enabled);
            }
            return;
        }
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
        if bind.0 == "lg:ui" {
            // Pendiente solo si difiere del idioma que se usa ahora.
            if value.as_str() == Some(lizarbe_core::i18n::lang().code()) {
                self.values.remove(&bind.0);
            } else {
                self.values.insert(bind.0.clone(), value);
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
        if let Some(i) = xcompose_index(&bind.0) {
            let orig = self
                .get_original(bind)
                .and_then(|v| v.as_str().map(String::from));
            if let (Some(orig), Some(e)) = (orig, self.xcompose.get_mut(i)) {
                e.text = orig;
            }
            return;
        }
        if let Some(i) = autostart_index(&bind.0) {
            let orig = self.get_original(bind).and_then(|v| v.as_bool());
            if let (Some(e), Some(on)) = (self.autostart.get_mut(i), orig) {
                e.enabled = on;
            }
            return;
        }
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
    fn language_is_not_written_to_the_lua_file() {
        let (_t, mut s) = sandbox();
        let other = if lizarbe_core::i18n::lang().code() == "es" {
            "en"
        } else {
            "es"
        };
        let def = catalog::find("lg:ui", &Ctx::default()).unwrap();
        s.set_field(&Bind("lg:ui".into()), json!(other), def.default.as_ref());
        assert!(s.dirty(), "el cambio de idioma queda pendiente");
        s.apply().unwrap();
        let lua = std::fs::read_to_string(s.paths.escritorio_lua()).unwrap_or_default();
        assert!(!lua.contains("lg"), "{lua}");
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
    fn screenshot_settings_write_the_session_env_and_warn() {
        let (d, mut s) = sandbox();
        s.set_field(
            &Bind("x:cap_dir".into()),
            json!("/tmp/mis capturas"),
            Some(&json!("/home/x/Pictures")),
        );
        let report = s.apply().unwrap();
        let env = std::fs::read_to_string(d.path().join("uwsm-env")).unwrap();
        assert!(
            env.contains("export OMARCHY_SCREENSHOT_DIR=\"/tmp/mis capturas\""),
            "{env}"
        );
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.contains("iniciar sesión") || w.contains("log in"))
        );
        // volver al valor por defecto limpia el bloque
        s.unset(&Bind("x:cap_dir".into()));
        s.apply().unwrap();
        let env = std::fs::read_to_string(d.path().join("uwsm-env")).unwrap();
        assert!(!env.contains("OMARCHY_SCREENSHOT_DIR"), "{env}");
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
    fn undoes_an_applied_change() {
        let (d, mut s) = sandbox();
        s.set_field(&Bind("general:gaps_in".into()), json!(9), Some(&json!(5)));
        s.apply().unwrap();
        let first = std::fs::read_to_string(d.path().join("escritorio.lua")).unwrap();
        // Distinta fecha de copia.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        s.set_field(&Bind("general:gaps_in".into()), json!(12), Some(&json!(5)));
        s.apply().unwrap();
        let points = s.undo_points();
        let last = points.first().expect("hay una copia del último cambio");
        assert_eq!(last.names(), "escritorio.lua");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        s.undo(&last.clone()).unwrap();
        let now = std::fs::read_to_string(d.path().join("escritorio.lua")).unwrap();
        assert_eq!(now, first, "vuelve a como estaba antes del último cambio");
        assert_eq!(
            s.undo_points().len(),
            points.len() + 1,
            "deshacer también deja copia"
        );
    }

    #[test]
    fn undo_point_dates() {
        let p = UndoPoint {
            stamp: "20261007-171306".into(),
            files: vec![],
        };
        assert_eq!(p.when(), "07/10/2026 17:13:06");
    }

    #[test]
    fn fixes_gdk_scale_of_omarchy_template() {
        let (d, mut s) = sandbox();
        std::fs::write(
            d.path().join("monitors.lua"),
            "local omarchy_gdk_scale = 2\nlocal omarchy_monitor_scale = \"auto\"\n",
        )
        .unwrap();
        s.reload();
        s.ctx.monitors.clear();
        assert!(s.gdk_change().is_none(), "sin pantallas no se adivina");
        s.ctx.monitors = vec![lizarbe_core::hypr::Monitor::named("HDMI-A-1")];
        s.ctx.monitors[0].scale = 1.0;
        let c = s.gdk_change().expect("un 1080p con GDK_SCALE=2");
        assert_eq!((c.old, c.new), (Some(json!(2)), Some(json!(1))));
        assert!(s.dirty());
        s.apply().unwrap();
        let mon = std::fs::read_to_string(d.path().join("monitors.lua")).unwrap();
        assert!(mon.contains("local omarchy_gdk_scale = 1"));
        assert!(mon.contains("local omarchy_monitor_scale = \"auto\""));
    }

    #[test]
    fn autostart_changes_are_written() {
        let (d, mut s) = sandbox();
        std::fs::write(
            d.path().join("autostart.lua"),
            "-- inicio\n-- o.launch_on_start(\"hyprsunset\")\n",
        )
        .unwrap();
        s.reload();
        assert_eq!(s.autostart.len(), 1);
        s.set_field(&Bind("as:0".into()), json!(true), None);
        s.autostart
            .push(crate::autostart::Entry::new("blueman-applet", false));
        assert!(s.dirty());
        assert_eq!(s.changes().len(), 2);
        s.apply().unwrap();
        let text = std::fs::read_to_string(d.path().join("autostart.lua")).unwrap();
        assert_eq!(
            text,
            "-- inicio\no.launch_on_start(\"hyprsunset\")\no.launch_on_start(\"blueman-applet\")\n"
        );
        assert!(!s.dirty());
    }

    #[test]
    fn custom_binds_roundtrip() {
        let (_d, mut s) = sandbox();
        let c = CustomBind {
            keys: "SUPER + ALT + W".into(),
            desc: "WhatsApp".into(),
            action: "{ webapp = \"https://web.whatsapp.com/\" }".into(),
        };
        s.set_custom_binds(std::slice::from_ref(&c));
        assert_eq!(s.custom_binds(), vec![c]);
        assert!(s.keys_in_use().iter().any(|(k, _)| k == "SUPER + ALT + W"));
        s.set_custom_binds(&[]);
        assert!(!s.dirty());
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
