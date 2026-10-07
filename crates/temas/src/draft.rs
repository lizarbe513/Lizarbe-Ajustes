//! Borrador del tema que se está editando: el `ThemeSpec` con deshacer y los
//! enlaces (`Bind`) con los que los formularios del núcleo leen y cambian
//! cada valor.

use std::path::PathBuf;

use lizarbe_core::form::FieldStore;
use lizarbe_core::themes::{
    Border, Dirs, Surfaces, ThemeSpec, icon_themes, neovim_schemes, vscode_schemes,
};
use serde_json::{Value, json};

/// Dónde vive un valor del borrador (`name`, `mode`, `c:<color>`, `b:<lado>:<campo>`,
/// `icons`, `nvim`, `vscode`, `kb`, `sf:<campo>`).
#[derive(Debug, Clone, PartialEq)]
pub struct Bind(pub String);

/// Lo que ofrecen los catálogos (sacado de lo instalado).
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub icons: Vec<String>,
    pub nvim: Vec<(String, String)>,
    pub vscode: Vec<(String, String)>,
}

impl Catalog {
    pub fn load(dirs: &Dirs) -> Catalog {
        Catalog {
            icons: icon_themes(),
            nvim: neovim_schemes(dirs),
            vscode: vscode_schemes(dirs),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Draft {
    pub spec: ThemeSpec,
    pub orig: ThemeSpec,
    pub name: String,
    pub orig_name: String,
    /// Carpeta del tema del que se parte (se copia entera al guardar).
    pub base: Option<PathBuf>,
    /// Carpeta del tema propio que se edita; `None` si es nuevo.
    pub editing: Option<String>,
    pub catalog: Catalog,
    undo: Vec<(ThemeSpec, String)>,
}

fn border_of<'a>(spec: &'a ThemeSpec, side: &str) -> &'a Border {
    if side == "active" {
        &spec.active_border
    } else {
        &spec.inactive_border
    }
}

fn surf_field(s: &Surfaces, k: &str) -> Option<Value> {
    Some(match k {
        "bar_bg" => json!(s.bar_bg),
        "bar_alpha" => json!(s.bar_alpha),
        "bar_text" => json!(s.bar_text),
        "bar_active" => json!(s.bar_active),
        "card_bg" => json!(s.card_bg),
        "card_alpha" => json!(s.card_alpha),
        "card_text" => json!(s.card_text),
        "card_border" => json!(s.card_border),
        "card_selected" => json!(s.card_selected),
        "lock_bg" => json!(s.lock_bg),
        "lock_text" => json!(s.lock_text),
        "lock_border" => json!(s.lock_border),
        _ => return None,
    })
}

fn set_surf(s: &mut Surfaces, k: &str, v: &Value) {
    let text = || v.as_str().unwrap_or_default().to_string();
    let num = || v.as_f64().unwrap_or(1.0) as f32;
    match k {
        "bar_bg" => s.bar_bg = text(),
        "bar_alpha" => s.bar_alpha = num(),
        "bar_text" => s.bar_text = text(),
        "bar_active" => s.bar_active = text(),
        "card_bg" => s.card_bg = text(),
        "card_alpha" => s.card_alpha = num(),
        "card_text" => s.card_text = text(),
        "card_border" => s.card_border = text(),
        "card_selected" => s.card_selected = text(),
        "lock_bg" => s.lock_bg = text(),
        "lock_text" => s.lock_text = text(),
        "lock_border" => s.lock_border = text(),
        _ => {}
    }
}

impl Draft {
    pub fn new(
        spec: ThemeSpec,
        name: String,
        base: Option<PathBuf>,
        editing: Option<String>,
        catalog: Catalog,
    ) -> Draft {
        Draft {
            orig: spec.clone(),
            spec,
            orig_name: name.clone(),
            name,
            base,
            editing,
            catalog,
            undo: vec![],
        }
    }

    pub fn dirty(&self) -> bool {
        self.spec != self.orig || self.name != self.orig_name
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Aplica un valor sin guardar deshacer (para vistas previas en una copia).
    pub fn set_field_silent(&mut self, bind: &Bind, value: &Value) {
        self.write(&bind.0, value);
    }

    pub fn push_undo(&mut self) {
        self.undo.push((self.spec.clone(), self.name.clone()));
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some((spec, name)) => {
                self.spec = spec;
                self.name = name;
                true
            }
            None => false,
        }
    }

    /// Carpeta de destino (nombre) a partir del nombre escrito.
    pub fn slug(&self) -> String {
        match &self.editing {
            Some(s) => s.clone(),
            None => lizarbe_core::themes::slugify(&self.name),
        }
    }

    fn read(&self, spec: &ThemeSpec, name: &str, key: &str) -> Option<Value> {
        if let Some(c) = key.strip_prefix("c:") {
            return spec.colors.get(c).map(|v| json!(v));
        }
        if let Some(rest) = key.strip_prefix("b:") {
            let (side, field) = rest.split_once(':')?;
            let b = border_of(spec, side);
            return match field {
                "from" => Some(json!(b.from)),
                "grad" => Some(json!(b.to.is_some())),
                "to" => b.to.as_ref().map(|t| json!(t)),
                "angle" => Some(json!(b.angle.unwrap_or(45))),
                _ => None,
            };
        }
        if let Some(f) = key.strip_prefix("sf:") {
            if f == "on" {
                return Some(json!(spec.surfaces.is_some()));
            }
            return spec.surfaces.as_ref().and_then(|s| surf_field(s, f));
        }
        match key {
            "name" => Some(json!(name)),
            "mode" => Some(json!(spec.mode)),
            "icons" => Some(json!(spec.icons)),
            "nvim" => Some(json!(
                self.catalog
                    .nvim
                    .iter()
                    .find(|(_, c)| Some(c) == spec.neovim.as_ref())
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default()
            )),
            "vscode" => Some(json!(
                self.catalog
                    .vscode
                    .iter()
                    .find(|(_, c)| Some(c) == spec.vscode.as_ref())
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default()
            )),
            "kb" => spec.keyboard.as_ref().map(|k| json!(format!("#{k}"))),
            _ => None,
        }
    }

    fn write(&mut self, key: &str, v: &Value) {
        let text = v.as_str().unwrap_or_default().to_string();
        if let Some(c) = key.strip_prefix("c:") {
            self.spec.colors.insert(c.to_string(), text);
            return;
        }
        if let Some(rest) = key.strip_prefix("b:")
            && let Some((side, field)) = rest.split_once(':')
        {
            let b = if side == "active" {
                &mut self.spec.active_border
            } else {
                &mut self.spec.inactive_border
            };
            match field {
                "from" => b.from = text,
                "grad" => {
                    if v.as_bool().unwrap_or(false) {
                        if b.to.is_none() {
                            b.to = Some(b.from.clone());
                            b.angle.get_or_insert(45);
                        }
                    } else {
                        b.to = None;
                        b.angle = None;
                    }
                }
                "to" => b.to = Some(text),
                "angle" => b.angle = v.as_u64().map(|a| a as u32),
                _ => {}
            }
            return;
        }
        if let Some(f) = key.strip_prefix("sf:") {
            if f == "on" {
                self.spec.surfaces = v
                    .as_bool()
                    .unwrap_or(false)
                    .then(|| Surfaces::from_palette(&self.spec.colors));
            } else if let Some(s) = self.spec.surfaces.as_mut() {
                set_surf(s, f, v);
            }
            return;
        }
        match key {
            "name" => self.name = text,
            "mode" => self.spec.mode = text,
            "icons" => self.spec.icons = text,
            "nvim" => {
                self.spec.neovim = self
                    .catalog
                    .nvim
                    .iter()
                    .find(|(n, _)| *n == text)
                    .map(|(_, c)| c.clone());
            }
            "vscode" => {
                self.spec.vscode = self
                    .catalog
                    .vscode
                    .iter()
                    .find(|(n, _)| *n == text)
                    .map(|(_, c)| c.clone());
            }
            "kb" => {
                self.spec.keyboard =
                    Some(text.trim_start_matches('#').to_string()).filter(|s| !s.is_empty());
            }
            _ => {}
        }
    }
}

impl FieldStore<Bind> for Draft {
    fn get(&self, bind: &Bind) -> Option<Value> {
        self.read(&self.spec, &self.name, &bind.0)
    }

    fn get_original(&self, bind: &Bind) -> Option<Value> {
        self.read(&self.orig, &self.orig_name, &bind.0)
    }

    fn set_field(&mut self, bind: &Bind, value: Value, _default: Option<&Value>) {
        if self.get(bind).as_ref() == Some(&value) {
            return;
        }
        self.push_undo();
        self.write(&bind.0, &value);
    }

    fn unset(&mut self, bind: &Bind) {
        if let Some(orig) = self.get_original(bind)
            && self.get(bind).as_ref() != Some(&orig)
        {
            self.push_undo();
            self.write(&bind.0, &orig);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn draft() -> Draft {
        let mut colors = BTreeMap::new();
        colors.insert("accent".to_string(), "#7aa2f7".to_string());
        colors.insert("background".to_string(), "#1a1b26".to_string());
        let spec = ThemeSpec {
            mode: "dark".into(),
            colors,
            active_border: Border::solid("#7aa2f7"),
            inactive_border: Border::solid("#414868"),
            icons: "Yaru-blue".into(),
            neovim: None,
            vscode: None,
            keyboard: None,
            backgrounds: vec![],
            unlock: None,
            preview: None,
            surfaces: None,
        };
        let catalog = Catalog {
            icons: vec!["Yaru-blue".into(), "Papirus".into()],
            nvim: vec![("tokyonight-night".into(), "return {}".into())],
            vscode: vec![],
        };
        Draft::new(spec, "Mi tema".into(), None, None, catalog)
    }

    #[test]
    fn edits_track_changes_and_undo() {
        let mut d = draft();
        let b = Bind("c:accent".into());
        assert!(!d.dirty());
        d.set_field(&b, json!("#e31b23"), None);
        assert!(d.dirty());
        assert_eq!(d.get(&b), Some(json!("#e31b23")));
        assert_eq!(d.get_original(&b), Some(json!("#7aa2f7")));
        assert!(d.undo());
        assert!(!d.dirty());
        assert!(!d.undo());
    }

    #[test]
    fn same_value_is_not_a_change() {
        let mut d = draft();
        d.set_field(&Bind("mode".into()), json!("dark"), None);
        assert!(!d.dirty() && !d.can_undo());
    }

    #[test]
    fn gradient_borders_toggle() {
        let mut d = draft();
        d.set_field(&Bind("b:active:grad".into()), json!(true), None);
        assert!(d.spec.active_border.to.is_some());
        assert_eq!(d.spec.active_border.angle, Some(45));
        d.set_field(&Bind("b:active:angle".into()), json!(90), None);
        assert_eq!(d.spec.active_border.angle, Some(90));
        d.set_field(&Bind("b:active:grad".into()), json!(false), None);
        assert_eq!(d.spec.active_border, Border::solid("#7aa2f7"));
    }

    #[test]
    fn catalog_choices_write_files_content() {
        let mut d = draft();
        d.set_field(&Bind("nvim".into()), json!("tokyonight-night"), None);
        assert_eq!(d.spec.neovim.as_deref(), Some("return {}"));
        assert_eq!(d.get(&Bind("nvim".into())), Some(json!("tokyonight-night")));
        d.set_field(&Bind("nvim".into()), json!(""), None);
        assert_eq!(d.spec.neovim, None);
        d.set_field(&Bind("kb".into()), json!("#e31b23"), None);
        assert_eq!(d.spec.keyboard.as_deref(), Some("e31b23"));
    }

    #[test]
    fn surfaces_are_optional_and_follow_the_palette_when_enabled() {
        let mut d = draft();
        assert_eq!(d.get(&Bind("sf:on".into())), Some(json!(false)));
        d.set_field(&Bind("sf:on".into()), json!(true), None);
        assert_eq!(d.get(&Bind("sf:bar_bg".into())), Some(json!("#1a1b26")));
        d.set_field(&Bind("sf:bar_alpha".into()), json!(0.5), None);
        assert_eq!(d.spec.surfaces.as_ref().unwrap().bar_alpha, 0.5);
        d.unset(&Bind("sf:on".into()));
        assert!(d.spec.surfaces.is_none());
    }

    #[test]
    fn slug_is_fixed_when_editing_and_derived_when_new() {
        let mut d = draft();
        assert_eq!(d.slug(), "mi-tema");
        d.editing = Some("viejo".into());
        d.name = "Otro".into();
        assert_eq!(d.slug(), "viejo");
    }
}
