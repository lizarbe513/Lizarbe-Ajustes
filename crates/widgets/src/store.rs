//! Estado editable: `shell.json` y `shell.toml` del usuario, con los cambios
//! pendientes, su diff legible y la aplicación segura (backup + escritura
//! atómica + recarga del shell).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use lizarbe_core::form::FieldStore;
use lizarbe_core::fsutil::Transaction;
use serde_json::{Value, json};
use toml_edit::DocumentMut;

use crate::omarchy::catalog::{Catalog, Plugin};
use crate::omarchy::paths::Paths;
use crate::omarchy::shell_json::{self as sj, Seg, key};
use crate::omarchy::shell_toml::{self as st, ThemeToml};
use crate::omarchy::{ipc, schema};

/// Dónde vive un valor editable.
#[derive(Debug, Clone, PartialEq)]
pub enum Bind {
    Json(Vec<Seg>),
    Toml(String, String),
}

/// Operaciones que se delegan a `omarchy plugin …` al aplicar (plugins
/// clonados, cuyo estado implica reglas internas del shell).
#[derive(Debug, Clone, PartialEq)]
pub enum PluginOp {
    Enable(String),
    Disable(String),
}

impl PluginOp {
    pub fn id(&self) -> &str {
        match self {
            PluginOp::Enable(id) | PluginOp::Disable(id) => id,
        }
    }
}

/// Qué parte de la configuración devuelve "Restaurar" a los valores de fábrica.
#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    /// Posición, transparencia, anclaje y barra en uso.
    Bar,
    /// Widgets de la barra y su orden.
    Layout,
    /// Ajustes de un widget concreto (sección, índice).
    Widget(usize, usize),
    /// Componentes de Omarchy desactivados y barra alternativa.
    Plugins,
    Idle,
    /// Apariencia: solo las claves principales o todo `shell.toml`.
    Appearance {
        all: bool,
    },
    /// Todo: `shell.json` de fábrica y sin ajustes de apariencia.
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub file: &'static str,
    pub path: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Default)]
pub struct ApplyReport {
    pub backups: Vec<PathBuf>,
    pub reloaded: bool,
    pub errors: Vec<String>,
}

pub struct Store {
    pub paths: Paths,
    pub catalog: Catalog,
    pub theme: ThemeToml,
    pub json: Value,
    json_orig: Value,
    json_raw: Option<String>,
    /// El usuario ya tiene `shell.json` propio (si no, se parte de los defaults).
    pub json_from_user: bool,
    pub json_error: Option<String>,
    pub toml: DocumentMut,
    toml_orig: String,
    toml_raw: Option<String>,
    pub toml_error: Option<String>,
    pub ops: Vec<PluginOp>,
}

impl FieldStore<Bind> for Store {
    fn get(&self, bind: &Bind) -> Option<Value> {
        Store::get(self, bind)
    }

    fn get_original(&self, bind: &Bind) -> Option<Value> {
        Store::get_original(self, bind)
    }

    fn set_field(&mut self, bind: &Bind, value: Value, default: Option<&Value>) {
        Store::set_field(self, bind, value, default)
    }

    fn unset(&mut self, bind: &Bind) {
        Store::unset(self, bind)
    }
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

impl Store {
    pub fn load(paths: Paths) -> Store {
        let mut s = Store {
            catalog: Catalog::default(),
            theme: ThemeToml::default(),
            json: json!({}),
            json_orig: json!({}),
            json_raw: None,
            json_from_user: false,
            json_error: None,
            toml: DocumentMut::new(),
            toml_orig: String::new(),
            toml_raw: None,
            toml_error: None,
            ops: vec![],
            paths,
        };
        s.reload();
        s
    }

    /// Relee todo desde disco y descarta los cambios pendientes.
    pub fn reload(&mut self) {
        self.catalog = Catalog::load(&self.paths);
        self.theme = ThemeToml::parse(&read(&self.paths.theme_shell_toml()).unwrap_or_default());

        self.json_raw = read(&self.paths.shell_json());
        self.json_error = None;
        let user = self.json_raw.as_deref().filter(|t| !t.trim().is_empty());
        self.json_from_user = user.is_some();
        let mut v = match user {
            Some(text) => match serde_json::from_str::<Value>(text) {
                Ok(v) => v,
                Err(e) => {
                    self.json_error = Some(e.to_string());
                    self.load_defaults()
                }
            },
            None => self.load_defaults(),
        };
        sj::normalize(&mut v);
        self.json_orig = v.clone();
        self.json = v;

        self.toml_raw = read(&self.paths.shell_toml());
        self.toml_error = None;
        let text = self.toml_raw.clone().unwrap_or_default();
        self.toml = match text.parse::<DocumentMut>() {
            Ok(d) => d,
            Err(e) => {
                self.toml_error = Some(e.to_string());
                DocumentMut::new()
            }
        };
        self.toml_orig = self.toml.to_string();
        self.ops.clear();
    }

    /// Devuelve una parte de la configuración a los valores de fábrica de
    /// Omarchy. Queda como cambio pendiente: no se escribe nada en disco.
    pub fn restore(&mut self, scope: &Scope) {
        let mut d = self.load_defaults();
        sj::normalize(&mut d);
        let bar = |k: &str| vec![key("bar"), key(k)];
        match scope {
            Scope::Bar => {
                for k in ["position", "transparent", "centerAnchor"] {
                    match sj::get(&d, &bar(k)).cloned() {
                        Some(v) => {
                            sj::set(&mut self.json, &bar(k), v);
                        }
                        None => sj::remove(&mut self.json, &bar(k)),
                    }
                }
                sj::remove(&mut self.json, &bar("id"));
            }
            Scope::Layout => {
                let layout = sj::get(&d, &bar("layout")).cloned().unwrap_or(json!({}));
                sj::set(&mut self.json, &bar("layout"), layout);
                sj::normalize(&mut self.json);
            }
            Scope::Widget(s, i) => {
                let path = sj::entry_path(*s, *i);
                if let Some(entry) = sj::get(&self.json, &path) {
                    // Un módulo personalizado conserva lo que lo define.
                    let kept: serde_json::Map<String, Value> = match entry {
                        Value::Object(o) => o
                            .iter()
                            .filter(|(k, _)| {
                                matches!(k.as_str(), "id" | "type" | "exec" | "source")
                            })
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                        other => [("id".to_string(), json!(sj::entry_id(other)))]
                            .into_iter()
                            .collect(),
                    };
                    sj::set(&mut self.json, &path, Value::Object(kept));
                }
            }
            Scope::Plugins => {
                sj::remove(&mut self.json, &[key("disabledPlugins")]);
                sj::remove(&mut self.json, &bar("id"));
                self.ops.clear();
            }
            Scope::Idle => match sj::get(&d, &[key("idle")]).cloned() {
                Some(v) => {
                    sj::set(&mut self.json, &[key("idle")], v);
                }
                None => sj::remove(&mut self.json, &[key("idle")]),
            },
            Scope::Appearance { all } => {
                let keys: Vec<(String, String)> = if *all {
                    toml_pairs(&self.toml)
                        .into_iter()
                        .map(|(s, k, _)| (s, k))
                        .collect()
                } else {
                    st::SIMPLE_KEYS
                        .iter()
                        .map(|(s, k)| (s.to_string(), k.to_string()))
                        .collect()
                };
                for (s, k) in keys {
                    st::user_unset(&mut self.toml, &s, &k);
                }
            }
            Scope::All => {
                self.json = d;
                self.ops.clear();
                self.restore(&Scope::Appearance { all: true });
            }
        }
    }

    fn load_defaults(&self) -> Value {
        read(&self.paths.default_shell_json())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| json!({}))
    }

    /// `shell.json` tal como se cargó (sin cambios pendientes).
    pub fn json_original(&self) -> &Value {
        &self.json_orig
    }

    pub fn json_dirty(&self) -> bool {
        self.json != self.json_orig
    }

    pub fn toml_dirty(&self) -> bool {
        self.toml.to_string() != self.toml_orig
    }

    pub fn dirty(&self) -> bool {
        self.json_dirty() || self.toml_dirty() || !self.ops.is_empty()
    }

    pub fn discard(&mut self) {
        self.json = self.json_orig.clone();
        self.toml = self.toml_orig.parse().unwrap_or_default();
        self.ops.clear();
    }

    // ------------------------------------------------------------ valores

    pub fn get(&self, bind: &Bind) -> Option<Value> {
        match bind {
            Bind::Json(p) => sj::get(&self.json, p).cloned(),
            Bind::Toml(s, k) => st::user_get(&self.toml, s, k),
        }
    }

    /// `Value::Null` borra la clave (vuelve al valor por defecto).
    pub fn set(&mut self, bind: &Bind, value: Value) {
        if value.is_null() {
            return self.unset(bind);
        }
        match bind {
            Bind::Json(p) => {
                sj::set(&mut self.json, p, value);
            }
            Bind::Toml(s, k) => st::user_set(&mut self.toml, s, k, &value),
        }
    }

    /// Valor tal como estaba al cargar.
    pub fn get_original(&self, bind: &Bind) -> Option<Value> {
        match bind {
            Bind::Json(p) => sj::get(&self.json_orig, p).cloned(),
            Bind::Toml(s, k) => self
                .toml_orig
                .parse::<DocumentMut>()
                .ok()
                .and_then(|d| st::user_get(&d, s, k)),
        }
    }

    /// Como `set`, pero si el valor es el predeterminado y la clave no existía
    /// al cargar, la elimina: volver al valor de fábrica no deja cambios.
    pub fn set_field(&mut self, bind: &Bind, value: Value, default: Option<&Value>) {
        if default == Some(&value) && self.get_original(bind).is_none() {
            return self.unset(bind);
        }
        self.set(bind, value);
    }

    pub fn unset(&mut self, bind: &Bind) {
        match bind {
            Bind::Json(p) => sj::remove(&mut self.json, p),
            Bind::Toml(s, k) => st::user_unset(&mut self.toml, s, k),
        }
    }

    // ------------------------------------------------------------ plugins

    pub fn queued(&self, id: &str) -> Option<&PluginOp> {
        self.ops.iter().find(|o| o.id() == id)
    }

    /// Estado visible (incluye cambios pendientes).
    pub fn plugin_enabled(&self, p: &Plugin) -> bool {
        match self.queued(&p.id) {
            Some(PluginOp::Enable(_)) => true,
            Some(PluginOp::Disable(_)) => false,
            None => p.enabled_in(&self.json),
        }
    }

    /// Activa/desactiva un plugin. Devuelve `Err(clave_i18n)` si no se puede.
    pub fn toggle_plugin(&mut self, id: &str) -> Result<(), &'static str> {
        let p = self.catalog.get(id).cloned().ok_or("msg.unknown_plugin")?;
        let enabled = self.plugin_enabled(&p);

        if p.is_bar_option() {
            if enabled {
                if p.id == "omarchy.bar" {
                    return Err("msg.builtin_bar_always");
                }
                if p.cloned_from.is_empty() || p.cloned_from == "omarchy.bar" {
                    sj::remove(&mut self.json, &[key("bar"), key("id")]);
                } else {
                    sj::set(
                        &mut self.json,
                        &[key("bar"), key("id")],
                        json!(p.cloned_from),
                    );
                }
            } else if p.id == "omarchy.bar" {
                sj::remove(&mut self.json, &[key("bar"), key("id")]);
            } else {
                sj::set(&mut self.json, &[key("bar"), key("id")], json!(p.id));
            }
            return Ok(());
        }

        if p.is_bar_widget() {
            if enabled {
                self.remove_widget_everywhere(&p);
            } else {
                self.add_widget(&p.id, None);
            }
            return Ok(());
        }

        if !p.cloned_from.is_empty() {
            // Reglas de clones: se delegan al shell.
            match self.queued(id) {
                Some(_) => self.ops.retain(|o| o.id() != id),
                None => self.ops.push(if enabled {
                    PluginOp::Disable(p.id.clone())
                } else {
                    PluginOp::Enable(p.id.clone())
                }),
            }
            return Ok(());
        }

        if p.first_party {
            sj::set_disabled(&mut self.json, &p.id, enabled);
        } else {
            sj::set_disabled(&mut self.json, &p.id, false);
            sj::set_in_plugins(&mut self.json, &p.id, !enabled);
        }
        Ok(())
    }

    fn remove_widget_everywhere(&mut self, p: &Plugin) {
        let mut locs = sj::find_in_bar(&self.json, &p.id);
        locs.reverse();
        for (s, i) in locs {
            if !p.cloned_from.is_empty() {
                // Quitar un clon devuelve su sitio (y ajustes) al original.
                sj::set(
                    &mut self.json,
                    &[sj::entry_path(s, i), vec![key("id")]].concat(),
                    json!(p.cloned_from),
                );
            } else {
                sj::remove_entry(&mut self.json, s, i);
            }
        }
    }

    /// Añade un widget a la barra. Sin posición: al final de su sección por
    /// defecto. Un clon ocupa el sitio del original si este está en la barra.
    /// Devuelve la posición final.
    pub fn add_widget(&mut self, id: &str, at: Option<(usize, usize)>) -> (usize, usize) {
        sj::set_disabled(&mut self.json, id, false);
        let p = self.catalog.get(id).cloned();
        if let Some(p) = &p
            && !p.cloned_from.is_empty()
            && at.is_none()
            && let Some(&(s, i)) = sj::find_in_bar(&self.json, &p.cloned_from).first()
        {
            sj::set(
                &mut self.json,
                &[sj::entry_path(s, i), vec![key("id")]].concat(),
                json!(id),
            );
            return (s, i);
        }
        let (s, i) = at.unwrap_or_else(|| {
            let s = p
                .as_ref()
                .and_then(|p| p.bar_widget.as_ref())
                .and_then(|b| b.default_section)
                .unwrap_or(1);
            (s, sj::section(&self.json, s).len())
        });
        let i = i.min(sj::section(&self.json, s).len());
        sj::insert_entry(&mut self.json, s, i, json!({ "id": id }));
        (s, i)
    }

    // ------------------------------------------------------------ cambios

    pub fn changes(&self) -> Vec<Change> {
        let mut out = vec![];
        diff_json("", &self.json_orig, &self.json, &mut out);
        let orig: DocumentMut = self.toml_orig.parse().unwrap_or_default();
        diff_toml(&orig, &self.toml, &mut out);
        out
    }

    /// Archivos modificados por fuera desde que se cargaron.
    pub fn external_changes(&self) -> Vec<&'static str> {
        let mut out = vec![];
        if read(&self.paths.shell_json()) != self.json_raw {
            out.push("shell.json");
        }
        if read(&self.paths.shell_toml()) != self.toml_raw {
            out.push("shell.toml");
        }
        out
    }

    pub fn apply(&mut self) -> Result<ApplyReport> {
        let mut report = ApplyReport::default();
        std::fs::create_dir_all(&self.paths.config_dir)
            .with_context(|| format!("{}", self.paths.config_dir.display()))?;

        let mut tx = Transaction::new(&self.paths.backup_dir);
        if self.json_dirty() {
            tx.write(&self.paths.shell_json(), &sj::to_pretty(&self.json))?;
        }
        if self.toml_dirty() {
            tx.write(&self.paths.shell_toml(), &self.toml.to_string())?;
        }
        report.backups = tx.commit(30);

        if !self.paths.sandbox && ipc::shell_running() {
            match ipc::reload_config() {
                Ok(()) => report.reloaded = true,
                Err(e) => report.errors.push(e),
            }
            for op in self.ops.clone() {
                let (verb, id) = match &op {
                    PluginOp::Enable(id) => ("enable", id),
                    PluginOp::Disable(id) => ("disable", id),
                };
                if let Err(e) = ipc::run("omarchy", &["plugin", verb, id]) {
                    report.errors.push(e);
                }
            }
        } else if !self.ops.is_empty() {
            report.errors.push("omarchy-shell".into());
        }
        self.reload();
        Ok(report)
    }
}

fn short(v: &Value) -> String {
    let s = match v {
        Value::String(s) => format!("\"{}\"", schema::escape_newlines(s)),
        Value::Array(a) if a.iter().all(|e| !sj::entry_id(e).is_empty()) && !a.is_empty() => {
            format!(
                "[{}]",
                a.iter().map(sj::entry_id).collect::<Vec<_>>().join(", ")
            )
        }
        other => other.to_string(),
    };
    if s.chars().count() > 120 {
        format!("{}…", s.chars().take(119).collect::<String>())
    } else {
        s
    }
}

fn diff_json(path: &str, a: &Value, b: &Value, out: &mut Vec<Change>) {
    if a == b {
        return;
    }
    let join = |k: &str| {
        if path.is_empty() {
            k.to_string()
        } else {
            format!("{path}.{k}")
        }
    };
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                match y.get(k) {
                    Some(vb) => diff_json(&join(k), va, vb, out),
                    None => out.push(Change {
                        file: "shell.json",
                        path: join(k),
                        old: Some(short(va)),
                        new: None,
                    }),
                }
            }
            for (k, vb) in y {
                if !x.contains_key(k) {
                    out.push(Change {
                        file: "shell.json",
                        path: join(k),
                        old: None,
                        new: Some(short(vb)),
                    });
                }
            }
        }
        (Value::Array(x), Value::Array(y))
            if x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|(p, q)| sj::entry_id(p) == sj::entry_id(q)) =>
        {
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                let id = sj::entry_id(p);
                let label = if id.is_empty() {
                    format!("{path}[{i}]")
                } else {
                    format!("{path}[{id}]")
                };
                diff_json(&label, p, q, out);
            }
        }
        _ => out.push(Change {
            file: "shell.json",
            path: path.to_string(),
            old: Some(short(a)),
            new: Some(short(b)),
        }),
    }
}

fn toml_pairs(doc: &DocumentMut) -> Vec<(String, String, Value)> {
    let mut out = vec![];
    for (section, item) in doc.iter() {
        if let Some(t) = item.as_table_like() {
            for (k, v) in t.iter() {
                if let Some(v) = v.as_value() {
                    out.push((section.to_string(), k.to_string(), st::toml_to_json(v)));
                }
            }
        }
    }
    out
}

fn diff_toml(a: &DocumentMut, b: &DocumentMut, out: &mut Vec<Change>) {
    let pa = toml_pairs(a);
    let pb = toml_pairs(b);
    let find = |list: &[(String, String, Value)], s: &str, k: &str| {
        list.iter()
            .find(|(x, y, _)| x == s && y == k)
            .map(|(_, _, v)| v.clone())
    };
    let mut seen = vec![];
    for (s, k, _) in pa.iter().chain(pb.iter()) {
        if seen.contains(&(s.clone(), k.clone())) {
            continue;
        }
        seen.push((s.clone(), k.clone()));
        let (va, vb) = (find(&pa, s, k), find(&pb, s, k));
        if va != vb {
            out.push(Change {
                file: "shell.toml",
                path: format!("{s}.{k}"),
                old: va.as_ref().map(short),
                new: vb.as_ref().map(short),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("shell.json"),
            r#"{"bar":{"position":"top","layout":{"left":[{"id":"omarchy.menu"}],"center":[{"id":"omarchy.clock","format":"HH:mm"}],"right":[]}},"idle":{"lock":300,"screensaver":150},"plugins":[],"version":1}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("shell.toml"),
            "# mío\n[font]\nbase-size = 12\n",
        )
        .unwrap();
        let store = Store::load(Paths::detect(Some(dir.path().to_path_buf())));
        (dir, store)
    }

    #[test]
    fn edits_produce_readable_changes() {
        let (_d, mut s) = sandbox();
        assert!(!s.dirty());
        let mut p = sj::entry_path(1, 0);
        p.push(key("format"));
        s.set(&Bind::Json(p), json!("dddd HH:mm"));
        s.set(&Bind::Json(vec![key("idle"), key("lock")]), json!(600));
        s.set(&Bind::Toml("font".into(), "base-size".into()), json!(14));
        let ch = s.changes();
        assert!(
            ch.iter()
                .any(|c| c.path == "bar.layout.center[omarchy.clock].format"
                    && c.new.as_deref() == Some("\"dddd HH:mm\""))
        );
        assert!(
            ch.iter()
                .any(|c| c.path == "idle.lock" && c.new.as_deref() == Some("600"))
        );
        assert!(
            ch.iter()
                .any(|c| c.file == "shell.toml" && c.path == "font.base-size")
        );
        s.discard();
        assert!(!s.dirty());
    }

    #[test]
    fn apply_writes_atomically_with_backup() {
        let (d, mut s) = sandbox();
        s.set(
            &Bind::Json(vec![key("bar"), key("position")]),
            json!("bottom"),
        );
        s.set(
            &Bind::Toml("bar".into(), "background-alpha".into()),
            json!(0.8),
        );
        let report = s.apply().unwrap();
        assert_eq!(report.backups.len(), 2);
        assert!(report.backups.iter().all(|b| b.exists()));
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(d.path().join("shell.json")).unwrap())
                .unwrap();
        assert_eq!(written["bar"]["position"], json!("bottom"));
        assert_eq!(
            written["bar"]["layout"]["center"][0]["format"],
            json!("HH:mm")
        );
        let toml = std::fs::read_to_string(d.path().join("shell.toml")).unwrap();
        assert!(toml.starts_with("# mío\n"));
        assert!(toml.contains("background-alpha = 0.8"));
        assert!(!s.dirty());
        assert!(!d.path().join(".shell.json.lizarbe.tmp").exists());
    }

    #[test]
    fn detects_external_changes() {
        let (d, s) = sandbox();
        assert!(s.external_changes().is_empty());
        std::fs::write(d.path().join("shell.json"), "{}").unwrap();
        assert_eq!(s.external_changes(), vec!["shell.json"]);
    }

    /// Valores de fábrica controlados para las pruebas de "Restaurar".
    fn sandbox_with_defaults() -> (tempfile::TempDir, Store) {
        let (dir, mut s) = sandbox();
        let omarchy = dir.path().join("omarchy");
        std::fs::create_dir_all(omarchy.join("config/omarchy")).unwrap();
        std::fs::write(
            omarchy.join("config/omarchy/shell.json"),
            r#"{"version":1,"idle":{"screensaver":150,"lock":300},"bar":{"position":"top","transparent":false,"centerAnchor":"omarchy.clock","layout":{"left":[{"id":"omarchy.menu"},{"id":"omarchy.workspaces"}],"center":[{"id":"omarchy.clock"}],"right":[{"id":"omarchy.audio"}]}},"plugins":[]}"#,
        )
        .unwrap();
        s.paths.omarchy_path = omarchy;
        (dir, s)
    }

    #[test]
    fn restore_sections() {
        let (_d, mut s) = sandbox_with_defaults();
        // Barra.
        s.set(
            &Bind::Json(vec![key("bar"), key("position")]),
            json!("left"),
        );
        s.set(&Bind::Json(vec![key("bar"), key("id")]), json!("acme.bar"));
        s.restore(&Scope::Bar);
        assert_eq!(s.json["bar"]["position"], json!("top"));
        assert_eq!(s.json["bar"]["centerAnchor"], json!("omarchy.clock"));
        assert!(s.json["bar"].get("id").is_none());
        // Widgets: el orden de fábrica.
        s.restore(&Scope::Layout);
        let ids: Vec<String> = sj::section(&s.json, 0).iter().map(sj::entry_id).collect();
        assert_eq!(ids, ["omarchy.menu", "omarchy.workspaces"]);
        assert_eq!(sj::section(&s.json, 1)[0], json!({"id": "omarchy.clock"}));
        // Inactividad.
        s.set(&Bind::Json(vec![key("idle"), key("lock")]), json!(900));
        s.restore(&Scope::Idle);
        assert_eq!(s.json["idle"], json!({"screensaver": 150, "lock": 300}));
    }

    #[test]
    fn restore_widget_keeps_identity() {
        let (_d, mut s) = sandbox_with_defaults();
        s.restore(&Scope::Widget(1, 0));
        assert_eq!(sj::section(&s.json, 1)[0], json!({"id": "omarchy.clock"}));
        sj::insert_entry(
            &mut s.json,
            2,
            0,
            json!({"id": "vpn", "type": "command", "exec": "x", "interval": 9}),
        );
        s.restore(&Scope::Widget(2, 0));
        assert_eq!(
            sj::section(&s.json, 2)[0],
            json!({"id": "vpn", "type": "command", "exec": "x"})
        );
    }

    #[test]
    fn restore_appearance_and_everything() {
        let (_d, mut s) = sandbox_with_defaults();
        s.set(
            &Bind::Toml("menu".into(), "background-alpha".into()),
            json!(0.5),
        );
        s.set(
            &Bind::Toml("tooltip".into(), "text".into()),
            json!("#ffffff"),
        );
        s.restore(&Scope::Appearance { all: false });
        assert!(
            s.get(&Bind::Toml("font".into(), "base-size".into()))
                .is_none()
        );
        assert!(
            s.get(&Bind::Toml("menu".into(), "background-alpha".into()))
                .is_none()
        );
        assert!(
            s.get(&Bind::Toml("tooltip".into(), "text".into()))
                .is_some()
        );
        s.restore(&Scope::Appearance { all: true });
        assert!(
            s.get(&Bind::Toml("tooltip".into(), "text".into()))
                .is_none()
        );
        // El comentario del usuario sobrevive.
        assert!(s.toml.to_string().contains("# mío"));

        s.set(
            &Bind::Json(vec![key("bar"), key("position")]),
            json!("left"),
        );
        s.set_disabled_for_test("omarchy.osd");
        s.restore(&Scope::All);
        assert_eq!(s.json["bar"]["position"], json!("top"));
        assert!(s.json.get("disabledPlugins").is_none());
        assert_eq!(sj::section(&s.json, 2)[0], json!({"id": "omarchy.audio"}));
    }

    impl Store {
        fn set_disabled_for_test(&mut self, id: &str) {
            sj::set_disabled(&mut self.json, id, true);
        }
    }

    #[test]
    fn widget_add_and_toggle() {
        let (_d, mut s) = sandbox();
        let at = s.add_widget("omarchy.spacer", Some((0, 1)));
        assert_eq!(at, (0, 1));
        assert_eq!(sj::entry_id(&sj::section(&s.json, 0)[1]), "omarchy.spacer");
        // Un widget fuera del catálogo se añade al centro por defecto.
        let at = s.add_widget("custom.x", None);
        assert_eq!(at, (1, 1));
    }
}
