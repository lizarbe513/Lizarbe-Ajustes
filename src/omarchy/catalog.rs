//! Catálogo de plugins: recorre los `manifest.json` igual que
//! `omarchy-plugin-catalog` (primero los de Omarchy, luego los del usuario).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::paths::Paths;
use super::schema::{self, FieldDef};
use super::shell_json as sj;

#[derive(Debug, Clone, Default)]
pub struct BarWidgetMeta {
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub allow_multiple: bool,
    pub default_section: Option<usize>,
    pub schema: Vec<FieldDef>,
}

#[derive(Debug, Clone, Default)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub kinds: Vec<String>,
    pub first_party: bool,
    pub source_dir: PathBuf,
    pub cloned_from: String,
    pub is_git: bool,
    pub bar_widget: Option<BarWidgetMeta>,
}

impl Plugin {
    pub fn has_kind(&self, k: &str) -> bool {
        self.kinds.iter().any(|x| x == k)
    }

    pub fn is_bar_widget(&self) -> bool {
        self.has_kind("bar-widget")
    }

    pub fn is_bar_option(&self) -> bool {
        self.has_kind("bar")
    }

    /// Nombre a mostrar: el del widget de barra si existe.
    pub fn display_name(&self) -> String {
        self.bar_widget
            .as_ref()
            .map(|b| b.display_name.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if self.name.is_empty() {
                    self.id.clone()
                } else {
                    self.name.clone()
                }
            })
    }

    pub fn display_description(&self) -> String {
        self.bar_widget
            .as_ref()
            .map(|b| b.description.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.description.clone())
    }

    /// Estado "activo" según las reglas de `PluginRegistry.isEnabled`.
    pub fn enabled_in(&self, cfg: &Value) -> bool {
        if self.is_bar_option() {
            return sj::bar_option(cfg) == self.id;
        }
        if sj::disabled_list(cfg).contains(&self.id) {
            return false;
        }
        if self.is_bar_widget() {
            // Para un widget lo relevante para el usuario es si está en la barra.
            return !sj::find_in_bar(cfg, &self.id).is_empty();
        }
        if self.first_party {
            return true;
        }
        sj::in_plugins(cfg, &self.id)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub plugins: Vec<Plugin>,
}

impl Catalog {
    pub fn load(paths: &Paths) -> Catalog {
        let mut manifests = vec![];
        collect_first_party(&paths.first_party_plugins(), &mut manifests);
        let first_count = manifests.len();
        collect_user(&paths.user_plugins(), &mut manifests);

        let mut plugins: Vec<Plugin> = vec![];
        for (i, path) in manifests.iter().enumerate() {
            let Some(p) = parse_manifest(path, i < first_count) else {
                continue;
            };
            // Un plugin del usuario con el mismo id reemplaza al de Omarchy.
            if let Some(existing) = plugins.iter_mut().find(|x| x.id == p.id) {
                *existing = p;
            } else {
                plugins.push(p);
            }
        }
        plugins.sort_by_key(|p| p.display_name().to_lowercase());
        Catalog { plugins }
    }

    pub fn get(&self, id: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.id == id)
    }

    pub fn bar_widgets(&self) -> impl Iterator<Item = &Plugin> {
        self.plugins.iter().filter(|p| p.is_bar_widget())
    }

    pub fn bar_options(&self) -> impl Iterator<Item = &Plugin> {
        self.plugins.iter().filter(|p| p.is_bar_option())
    }

    /// Clon activo de un plugin de Omarchy (si existe).
    pub fn clone_of(&self, source: &str) -> Option<&Plugin> {
        self.plugins.iter().find(|p| p.cloned_from == source)
    }
}

fn is_manifest_name(name: &str) -> bool {
    name == "manifest.json" || name.ends_with(".manifest.json")
}

fn collect_first_party(dir: &Path, out: &mut Vec<PathBuf>) {
    // find -mindepth 2 -maxdepth 4 (manifest.json | *.manifest.json)
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                if depth < 4 {
                    walk(&p, depth + 1, out);
                }
            } else if depth >= 2
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(is_manifest_name)
            {
                out.push(p);
            }
        }
    }
    walk(dir, 1, out);
}

fn collect_user(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut dirs: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
        })
        .collect();
    dirs.sort();
    for d in dirs {
        let m = d.join("manifest.json");
        if m.is_file() {
            out.push(m);
        }
    }
}

pub fn parse_manifest(path: &Path, first_party: bool) -> Option<Plugin> {
    let text = std::fs::read_to_string(path).ok()?;
    let m: Value = serde_json::from_str(&text).ok()?;
    let s = |k: &str| {
        m.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let id = s("id");
    if id.is_empty() {
        return None;
    }
    let kinds = m
        .get("kinds")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|k| k.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let source_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let cloned_from = m
        .get("omarchy")
        .and_then(|o| o.get("clonedFrom"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let bar_widget = m.get("barWidget").map(|b| {
        let bs = |k: &str| {
            b.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let defaults = b.get("defaults").cloned().unwrap_or(Value::Null);
        let schema = b
            .get("schema")
            .and_then(Value::as_array)
            .map(|fields| {
                fields
                    .iter()
                    .filter_map(|f| schema::from_manifest(f, Some(&defaults)))
                    .collect()
            })
            .unwrap_or_default();
        BarWidgetMeta {
            display_name: bs("displayName"),
            description: bs("description"),
            category: bs("category"),
            allow_multiple: b
                .get("allowMultiple")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            default_section: sj::SECTIONS.iter().position(|x| *x == bs("defaultSection")),
            schema,
        }
    });
    Some(Plugin {
        name: s("name"),
        description: s("description"),
        author: s("author"),
        version: s("version"),
        kinds,
        first_party,
        is_git: !first_party && source_dir.join(".git").exists(),
        source_dir,
        cloned_from,
        bar_widget,
        id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Todos los manifests reales de Omarchy (si está instalado) deben poder
    /// leerse sin errores.
    #[test]
    fn parses_installed_manifests() {
        let paths = Paths::detect(None);
        let mut files = vec![];
        collect_first_party(&paths.first_party_plugins(), &mut files);
        if files.is_empty() {
            return; // Omarchy no instalado en esta máquina.
        }
        for f in &files {
            assert!(
                parse_manifest(f, true).is_some(),
                "manifest ilegible: {f:?}"
            );
        }
        let cat = Catalog::load(&paths);
        let clock = cat.get("omarchy.clock").expect("omarchy.clock");
        assert!(clock.is_bar_widget());
        let ind = cat.get("omarchy.indicators").unwrap();
        assert!(ind.bar_widget.as_ref().unwrap().allow_multiple);
        assert!(!ind.bar_widget.as_ref().unwrap().schema.is_empty());
    }

    #[test]
    fn enabled_rules() {
        let cfg = json!({
            "bar": {"layout": {"left": [{"id": "omarchy.clock"}], "center": [], "right": []}},
            "plugins": [{"id": "acme.svc"}],
            "disabledPlugins": ["omarchy.osd"],
            "version": 1
        });
        let p = |id: &str, kinds: &[&str], fp: bool| Plugin {
            id: id.into(),
            kinds: kinds.iter().map(|s| s.to_string()).collect(),
            first_party: fp,
            ..Default::default()
        };
        assert!(p("omarchy.clock", &["bar-widget"], true).enabled_in(&cfg));
        assert!(!p("omarchy.audio", &["bar-widget"], true).enabled_in(&cfg));
        assert!(!p("omarchy.osd", &["panel"], true).enabled_in(&cfg));
        assert!(p("omarchy.idle", &["service"], true).enabled_in(&cfg));
        assert!(p("acme.svc", &["service"], false).enabled_in(&cfg));
        assert!(!p("acme.other", &["service"], false).enabled_in(&cfg));
        assert!(p("omarchy.bar", &["bar"], true).enabled_in(&cfg));
    }
}
