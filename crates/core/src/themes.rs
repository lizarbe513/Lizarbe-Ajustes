//! Temas de Omarchy: lectura y escritura de un tema completo (colores, bordes,
//! fondos, iconos, Neovim, VS Code, teclado, superficies del shell, imágenes
//! de desbloqueo y vista previa).
//!
//! Solo se escribe en la carpeta de temas del usuario; los temas de Omarchy
//! (`/usr/share/omarchy/themes`) solo se leen.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::color::{parse_hex, to_hex};
use crate::fsutil::write_atomic;

/// Colores que tiene `colors.toml`, en el orden en que se muestran.
pub const COLOR_KEYS: [&str; 22] = [
    "accent",
    "background",
    "foreground",
    "selection",
    "muted",
    "dark_background",
    "darker_background",
    "lighter_background",
    "dark_foreground",
    "light_foreground",
    "bright_foreground",
    "red",
    "yellow",
    "orange",
    "green",
    "cyan",
    "blue",
    "magenta",
    "brown",
    "bright_red",
    "bright_yellow",
    "bright_green",
];

/// Los 16 colores del terminal: (normal, brillante).
pub const ANSI_PAIRS: [(&str, &str); 6] = [
    ("red", "bright_red"),
    ("green", "bright_green"),
    ("yellow", "bright_yellow"),
    ("blue", "bright_blue"),
    ("magenta", "bright_magenta"),
    ("cyan", "bright_cyan"),
];

/// Claves de colores.toml que no están en `COLOR_KEYS` pero Omarchy usa.
pub const EXTRA_COLOR_KEYS: [&str; 3] = ["bright_cyan", "bright_blue", "bright_magenta"];

pub const IMAGE_EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];

/// Los que se muestran en modo simple.
pub const SIMPLE_KEYS: [&str; 5] = ["accent", "background", "foreground", "selection", "muted"];

/// Nombre de carpeta a partir de un nombre escrito por el usuario.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if (c == ' ' || c == '-' || c == '_') && !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

#[derive(Debug, Clone)]
pub struct Dirs {
    pub user: PathBuf,
    pub system: PathBuf,
}

impl Dirs {
    /// Temas propios, por nombre.
    pub fn list_user(&self) -> Vec<String> {
        list(&self.user)
    }

    /// Todos los temas que se pueden usar como base.
    pub fn list_all(&self) -> Vec<String> {
        let mut all = list(&self.user);
        for t in list(&self.system) {
            if !all.contains(&t) {
                all.push(t);
            }
        }
        all.sort();
        all
    }

    pub fn find(&self, slug: &str) -> Option<PathBuf> {
        [&self.user, &self.system]
            .iter()
            .map(|d| d.join(slug))
            .find(|p| p.is_dir())
    }

    pub fn is_user(&self, slug: &str) -> bool {
        self.user.join(slug).is_dir()
    }

    /// Crea `slug` como copia de `base`.
    pub fn create(&self, slug: &str, base: &str) -> Result<()> {
        if slug.is_empty() {
            bail!("nombre vacío");
        }
        let dest = self.user.join(slug);
        if dest.exists() {
            bail!("ya existe un tema con ese nombre");
        }
        let src = self.find(base).context("tema base no encontrado")?;
        copy_dir(&src, &dest)?;
        Ok(())
    }

    /// Borra un tema propio. Nunca borra uno de Omarchy.
    pub fn delete(&self, slug: &str) -> Result<()> {
        if slug.is_empty() || slug.contains('/') || slug == "." || slug == ".." {
            bail!("nombre no válido");
        }
        let dir = self.user.join(slug);
        if !dir.is_dir() {
            bail!("no es un tema propio");
        }
        std::fs::remove_dir_all(&dir).with_context(|| dir.display().to_string())
    }
}

fn list(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(String::from))
        .collect();
    out.sort();
    out
}

/// Copia una carpeta (siguiendo enlaces) dejando todo escribible.
pub fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for e in std::fs::read_dir(src)?.flatten() {
        let from = e.path();
        let to = dest.join(e.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).with_context(|| from.display().to_string())?;
            let mut perms = std::fs::metadata(&to)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            std::fs::set_permissions(&to, perms)?;
        }
    }
    Ok(())
}

/// Claves y valores de un `colors.toml` (solo texto entre comillas).
pub fn parse_colors(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('#') {
                return None;
            }
            let (k, v) = l.split_once('=')?;
            let v = v.trim().trim_matches('"');
            (!v.is_empty()).then(|| (k.trim().to_string(), v.to_string()))
        })
        .collect()
}

/// `colors.toml` con `changes` aplicados; las claves nuevas van al final y
/// el resto del archivo (comentarios, orden) no cambia.
pub fn set_colors(text: &str, changes: &[(String, String)]) -> String {
    let mut done = vec![false; changes.len()];
    let mut out = String::new();
    for line in text.lines() {
        let key = line
            .split_once('=')
            .map(|(k, _)| k.trim())
            .filter(|_| !line.trim().starts_with('#'));
        if let Some(i) = key.and_then(|k| changes.iter().position(|(ck, _)| ck == k)) {
            out.push_str(&format!("{} = \"{}\"\n", changes[i].0, changes[i].1));
            done[i] = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for (i, (k, v)) in changes.iter().enumerate() {
        if !done[i] {
            out.push_str(&format!("{k} = \"{v}\"\n"));
        }
    }
    out
}

// ---------------------------------------------------------------- bordes

/// Borde de ventana: un color, o un degradado de dos con ángulo opcional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Border {
    pub from: String,
    pub to: Option<String>,
    pub angle: Option<u32>,
}

impl Border {
    pub fn solid(hex: &str) -> Border {
        Border {
            from: hex.to_string(),
            to: None,
            angle: None,
        }
    }

    /// Lee `#rrggbb`, `rgb(rrggbb)`, `rgba(rrggbbaa)` o un degradado
    /// `rgba(…) rgba(…) 45deg`.
    pub fn parse(raw: &str) -> Option<Border> {
        let mut colors = vec![];
        let mut angle = None;
        for tok in raw.split_whitespace() {
            if let Some(a) = tok.strip_suffix("deg") {
                angle = a.parse().ok();
                continue;
            }
            let inner = tok
                .strip_prefix("rgba(")
                .or_else(|| tok.strip_prefix("rgb("))
                .map(|t| t.trim_end_matches(')'))
                .unwrap_or(tok);
            let c = parse_hex(inner)?;
            colors.push(to_hex(c));
        }
        let mut it = colors.into_iter();
        Some(Border {
            from: it.next()?,
            to: it.next(),
            angle,
        })
    }

    /// Texto para `colors.toml` / Hyprland.
    pub fn render(&self) -> String {
        match &self.to {
            None => self.from.to_uppercase(),
            Some(to) => {
                let hex = |c: &str| format!("rgba({}ff)", c.trim_start_matches('#'));
                let mut s = format!("{} {}", hex(&self.from), hex(to));
                if let Some(a) = self.angle {
                    s.push_str(&format!(" {a}deg"));
                }
                s
            }
        }
    }
}

// ---------------------------------------------------------------- superficies del shell

/// Colores de la barra, el menú/lanzador y el bloqueo del shell.
#[derive(Debug, Clone, PartialEq)]
pub struct Surfaces {
    pub bar_bg: String,
    pub bar_alpha: f32,
    pub bar_text: String,
    pub bar_active: String,
    pub card_bg: String,
    pub card_alpha: f32,
    pub card_text: String,
    pub card_border: String,
    pub card_selected: String,
    pub lock_bg: String,
    pub lock_text: String,
    pub lock_border: String,
}

impl Surfaces {
    /// Valores derivados de la paleta (lo que haría la plantilla de Omarchy).
    pub fn from_palette(c: &BTreeMap<String, String>) -> Surfaces {
        let g = |k: &str, d: &str| c.get(k).cloned().unwrap_or_else(|| d.to_string());
        Surfaces {
            bar_bg: g("background", "#1a1b26"),
            bar_alpha: 1.0,
            bar_text: g("foreground", "#a9b1d6"),
            bar_active: g("red", "#f7768e"),
            card_bg: g("lighter_background", "#24283b"),
            card_alpha: 0.98,
            card_text: g("foreground", "#a9b1d6"),
            card_border: g("accent", "#7aa2f7"),
            card_selected: g("accent", "#7aa2f7"),
            lock_bg: g("background", "#1a1b26"),
            lock_text: g("foreground", "#a9b1d6"),
            lock_border: g("accent", "#7aa2f7"),
        }
    }

    /// Lee `shell.bar/menu/lock.toml` de un tema; `None` si no trae ninguno.
    pub fn load(dir: &Path) -> Option<Surfaces> {
        let read = |name: &str, section: &str| -> Option<toml::Table> {
            let t: toml::Table =
                toml::from_str(&std::fs::read_to_string(dir.join(name)).ok()?).ok()?;
            t.get(section)?.as_table().cloned()
        };
        let bar = read("shell.bar.toml", "bar");
        let menu = read("shell.menu.toml", "menu");
        let lock = read("shell.lock.toml", "lock");
        if bar.is_none() && menu.is_none() {
            return None;
        }
        let s = |t: &Option<toml::Table>, k: &str| -> Option<String> {
            t.as_ref()?.get(k)?.as_str().map(String::from)
        };
        let f = |t: &Option<toml::Table>, k: &str| -> Option<f32> {
            let v = t.as_ref()?.get(k)?;
            v.as_float()
                .map(|x| x as f32)
                .or_else(|| v.as_integer().map(|x| x as f32))
        };
        let mut base = Surfaces::from_palette(&BTreeMap::new());
        macro_rules! set {
            ($field:ident, $v:expr) => {
                if let Some(v) = $v {
                    base.$field = v;
                }
            };
        }
        set!(bar_bg, s(&bar, "background"));
        set!(bar_alpha, f(&bar, "background-alpha"));
        set!(bar_text, s(&bar, "text"));
        set!(bar_active, s(&bar, "active"));
        set!(card_bg, s(&menu, "background"));
        set!(card_alpha, f(&menu, "background-alpha"));
        set!(card_text, s(&menu, "text"));
        set!(card_border, s(&menu, "border"));
        set!(card_selected, s(&menu, "selected-background"));
        set!(lock_bg, s(&lock, "background"));
        set!(lock_text, s(&lock, "text"));
        set!(lock_border, s(&lock, "border"));
        Some(base)
    }

    fn bar(&self) -> String {
        format!(
            "[bar]\nbackground       = \"{}\"\nbackground-alpha = {:.2}\ntext             = \"{}\"\nactive           = \"{}\"\nscale-with-font  = true\nsize-horizontal  = 26\nsize-vertical    = 28\n",
            self.bar_bg, self.bar_alpha, self.bar_text, self.bar_active
        )
    }

    fn card(&self, section: &str, title: &str) -> String {
        format!(
            "[{section}]\n# {title}\nbackground                = \"{}\"\nbackground-alpha          = {:.2}\ntext                      = \"{}\"\nborder                    = \"{}\"\nborder-alpha              = 0.85\nscrim                     = \"#000000\"\nscrim-alpha               = 0.25\nselected-background       = \"{}\"\nselected-background-alpha = 0.12\nselected-text             = \"{}\"\nselected-border           = \"{}\"\nselected-border-alpha     = 0.40\n",
            self.card_bg,
            self.card_alpha,
            self.card_text,
            self.card_border,
            self.card_selected,
            self.card_selected,
            self.card_border
        )
    }

    fn lock(&self) -> String {
        format!(
            "[lock]\nbackground       = \"{}\"\nbackground-alpha = 0.90\ntext             = \"{}\"\nplaceholder      = \"{}\"\ntext-error       = \"{}\"\nborder           = \"{}\"\nborder-active    = \"{}\"\nborder-error     = \"{}\"\nborder-alpha     = 1.0\nselection        = \"{}\"\nselection-alpha  = 0.25\n",
            self.lock_bg,
            self.lock_text,
            self.lock_text,
            self.lock_text,
            self.lock_border,
            self.lock_border,
            self.lock_border,
            self.lock_border
        )
    }
}

// ---------------------------------------------------------------- tema completo

/// Todo lo que se puede personalizar de un tema.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSpec {
    /// "dark" o "light".
    pub mode: String,
    /// Las claves de `COLOR_KEYS` (y las brillantes extra).
    pub colors: BTreeMap<String, String>,
    pub active_border: Border,
    pub inactive_border: Border,
    /// Tema de iconos GTK (contenido de `icons.theme`).
    pub icons: String,
    /// Contenido de `neovim.lua`.
    pub neovim: Option<String>,
    /// Contenido de `vscode.json`.
    pub vscode: Option<String>,
    /// Color del teclado RGB (sin `#`).
    pub keyboard: Option<String>,
    /// Imágenes de fondo, en orden (la primera es la que se ve al aplicar).
    pub backgrounds: Vec<PathBuf>,
    pub unlock: Option<PathBuf>,
    pub preview: Option<PathBuf>,
    pub surfaces: Option<Surfaces>,
}

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTS.contains(&e.to_lowercase().as_str()))
}

/// Imágenes de una carpeta, ordenadas por nombre.
pub fn images_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_image(p))
        .collect();
    v.sort();
    v
}

impl ThemeSpec {
    /// Lee un tema (de Omarchy o propio).
    pub fn load(dir: &Path) -> Result<ThemeSpec> {
        let text = std::fs::read_to_string(dir.join("colors.toml"))
            .with_context(|| format!("{}/colors.toml", dir.display()))?;
        let all: BTreeMap<String, String> = parse_colors(&text).into_iter().collect();
        let mode = all.get("mode").cloned().unwrap_or_else(|| "dark".into());
        let mut colors = BTreeMap::new();
        for k in COLOR_KEYS.iter().chain(EXTRA_COLOR_KEYS.iter()) {
            if let Some(v) = all.get(*k) {
                colors.insert(k.to_string(), v.clone());
            }
        }
        let accent = colors
            .get("accent")
            .cloned()
            .unwrap_or_else(|| "#7aa2f7".into());
        let active_border = all
            .get("hyprland_active_border")
            .and_then(|v| Border::parse(v))
            .unwrap_or_else(|| Border::solid(&accent));
        let inactive_border = all
            .get("hyprland_inactive_border")
            .and_then(|v| Border::parse(v))
            .unwrap_or_else(|| {
                Border::solid(colors.get("muted").map(String::as_str).unwrap_or("#595959"))
            });
        let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
        let existing = |name: &str| {
            let p = dir.join(name);
            p.is_file().then_some(p)
        };
        Ok(ThemeSpec {
            mode,
            colors,
            active_border,
            inactive_border,
            icons: read("icons.theme")
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            neovim: read("neovim.lua"),
            vscode: read("vscode.json"),
            keyboard: read("keyboard.rgb")
                .map(|s| s.trim().trim_start_matches('#').to_string())
                .filter(|s| !s.is_empty()),
            backgrounds: images_in(&dir.join("backgrounds")),
            unlock: existing("unlock.png"),
            preview: existing("preview.png"),
            surfaces: Surfaces::load(dir),
        })
    }

    /// `colors.toml` completo.
    pub fn render_colors(&self) -> String {
        let mut out = format!("mode = \"{}\"\n\n", self.mode);
        let line = |out: &mut String, k: &str| {
            if let Some(v) = self.colors.get(k) {
                out.push_str(&format!("{k} = \"{v}\"\n"));
            }
        };
        for k in ["accent", "selection", "muted"] {
            line(&mut out, k);
        }
        out.push('\n');
        for k in [
            "background",
            "dark_background",
            "darker_background",
            "lighter_background",
        ] {
            line(&mut out, k);
        }
        out.push('\n');
        for k in [
            "foreground",
            "dark_foreground",
            "light_foreground",
            "bright_foreground",
        ] {
            line(&mut out, k);
        }
        out.push_str(&format!(
            "\nhyprland_active_border = \"{}\"\nhyprland_inactive_border = \"{}\"\n\n",
            self.active_border.render(),
            self.inactive_border.render()
        ));
        for k in [
            "red", "yellow", "orange", "green", "cyan", "blue", "magenta", "brown",
        ] {
            line(&mut out, k);
        }
        out.push('\n');
        for k in [
            "bright_red",
            "bright_yellow",
            "bright_green",
            "bright_cyan",
            "bright_blue",
            "bright_magenta",
        ] {
            line(&mut out, k);
        }
        out
    }

    /// Escribe el tema en `dest` (carpeta del usuario). Si `base` existe y es
    /// otra carpeta, primero se copia entera para conservar lo que el estudio
    /// no edita (plantillas, `chromium.theme`…). Devuelve los archivos escritos.
    pub fn save(&self, dest: &Path, base: Option<&Path>) -> Result<Vec<PathBuf>> {
        let mut written = vec![];
        if let Some(base) = base
            && base.is_dir()
            && base.canonicalize().ok() != dest.canonicalize().ok()
        {
            copy_dir(base, dest)?;
        }
        std::fs::create_dir_all(dest)?;
        let mut put = |name: &str, text: &str| -> Result<()> {
            let p = dest.join(name);
            write_atomic(&p, text)?;
            written.push(p);
            Ok(())
        };
        put("colors.toml", &self.render_colors())?;
        if !self.icons.is_empty() {
            put("icons.theme", &format!("{}\n", self.icons))?;
        }
        if let Some(n) = &self.neovim {
            put("neovim.lua", n)?;
        }
        if let Some(v) = &self.vscode {
            put("vscode.json", v)?;
        }
        if let Some(k) = &self.keyboard {
            put("keyboard.rgb", &format!("{k}\n"))?;
        }
        if let Some(s) = &self.surfaces {
            put("shell.bar.toml", &s.bar())?;
            put(
                "shell.menu.toml",
                &s.card("menu", "Tarjeta del menú y estados de selección"),
            )?;
            put(
                "shell.launcher.toml",
                &s.card("launcher", "Tarjeta del lanzador rápido (Super + Espacio)"),
            )?;
            put("shell.lock.toml", &s.lock())?;
        }
        // Un hyprland.lua propio del tema lleva los colores de borde: se actualizan.
        let hl = dest.join("hyprland.lua");
        if let Ok(text) = std::fs::read_to_string(&hl) {
            let fixed = rewrite_border_lines(&text, &self.active_border, &self.inactive_border);
            if fixed != text {
                write_atomic(&hl, &fixed)?;
                written.push(hl);
            }
        }
        written.extend(self.save_images(dest)?);
        Ok(written)
    }

    fn save_images(&self, dest: &Path) -> Result<Vec<PathBuf>> {
        let mut written = vec![];
        let bg_dir = dest.join("backgrounds");
        std::fs::create_dir_all(&bg_dir)?;
        let keep: Vec<String> = self
            .backgrounds
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
            .collect();
        // Fondos que ya no están en la lista (solo imágenes dentro de backgrounds/).
        for old in images_in(&bg_dir) {
            let name = old.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !keep.iter().any(|k| k == name) {
                let _ = std::fs::remove_file(&old);
            }
        }
        for (i, src) in self.backgrounds.iter().enumerate() {
            let name = src
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("fondo.png");
            let target = bg_dir.join(name);
            if src.canonicalize().ok() != target.canonicalize().ok() {
                std::fs::copy(src, &target).with_context(|| src.display().to_string())?;
            }
            // El orden de aplicación de Omarchy es alfabético: el prefijo numérico lo fija.
            let _ = i;
            written.push(target);
        }
        for (src, name) in [(&self.unlock, "unlock.png"), (&self.preview, "preview.png")] {
            if let Some(src) = src {
                let target = dest.join(name);
                if src.canonicalize().ok() != target.canonicalize().ok() {
                    std::fs::copy(src, &target)?;
                }
                written.push(target);
            }
        }
        if let Some(u) = &self.unlock {
            let target = dest.join("preview-unlock.png");
            if u.canonicalize().ok() != target.canonicalize().ok() {
                std::fs::copy(u, &target)?;
            }
            written.push(target);
        }
        Ok(written)
    }
}

/// Reemplaza `local active_border_color = …` / `local inactive_border_color = …`.
fn rewrite_border_lines(text: &str, active: &Border, inactive: &Border) -> String {
    let lua = |b: &Border| match &b.to {
        None => format!("\"{}\"", b.from.to_uppercase()),
        Some(_) => format!("\"{}\"", b.render()),
    };
    let mut out = String::new();
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("local active_border_color") {
            out.push_str(&format!("local active_border_color = {}\n", lua(active)));
        } else if t.starts_with("local inactive_border_color") {
            out.push_str(&format!(
                "local inactive_border_color = {}\n",
                lua(inactive)
            ));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

// ---------------------------------------------------------------- catálogos (de lo instalado)

/// Temas de iconos instalados (carpetas con `index.theme` y sin cursores).
pub fn icon_themes() -> Vec<String> {
    let home = dirs::home_dir().unwrap_or_default();
    let roots = [
        PathBuf::from("/usr/share/icons"),
        home.join(".local/share/icons"),
        home.join(".icons"),
    ];
    let mut out: Vec<String> = vec![];
    for r in roots {
        for name in list(&r) {
            let d = r.join(&name);
            if d.join("index.theme").is_file()
                && !d.join("cursors").is_dir()
                && !["hicolor", "default", "locolor"].contains(&name.as_str())
                && !out.contains(&name)
            {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

/// Esquemas de Neovim que traen los temas instalados: (nombre, contenido de neovim.lua).
pub fn neovim_schemes(dirs: &Dirs) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![];
    for slug in dirs.list_all() {
        let Some(dir) = dirs.find(&slug) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(dir.join("neovim.lua")) else {
            continue;
        };
        let name = text
            .split("colorscheme = \"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .unwrap_or(&slug)
            .to_string();
        if !out.iter().any(|(n, _)| *n == name) {
            out.push((name, text));
        }
    }
    out
}

/// Temas de VS Code que traen los temas instalados: (nombre, contenido de vscode.json).
pub fn vscode_schemes(dirs: &Dirs) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![];
    for slug in dirs.list_all() {
        let Some(dir) = dirs.find(&slug) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(dir.join("vscode.json")) else {
            continue;
        };
        let name = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("name")?.as_str().map(String::from))
            .unwrap_or_else(|| slug.clone());
        if !out.iter().any(|(n, _)| *n == name) {
            out.push((name, text));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (tempfile::TempDir, Dirs) {
        let t = tempfile::tempdir().unwrap();
        let d = Dirs {
            user: t.path().join("user"),
            system: t.path().join("system"),
        };
        std::fs::create_dir_all(d.system.join("base/backgrounds")).unwrap();
        std::fs::write(
            d.system.join("base/colors.toml"),
            "mode = \"dark\"\n\naccent = \"#111111\"\nbackground = \"#000000\"\nhyprland_active_border = \"rgba(26a269ee) rgba(2ec27eee) 45deg\"\nhyprland_inactive_border = \"rgb(1e1e1e)\"\n",
        )
        .unwrap();
        std::fs::write(d.system.join("base/backgrounds/a.png"), "x").unwrap();
        std::fs::write(d.system.join("base/icons.theme"), "Yaru-blue\n").unwrap();
        std::fs::write(
            d.system.join("base/hyprland.lua"),
            "local active_border_color = \"#E31B23\"\nlocal inactive_border_color = \"#D5D8DC\"\nhl.config({})\n",
        )
        .unwrap();
        (t, d)
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Mi Tema Ñandú!"), "mi-tema-nandu");
        assert_eq!(slugify("  --a__b  "), "a-b");
        assert_eq!(slugify("***"), "");
    }

    #[test]
    fn creates_copies_and_deletes_only_user_themes() {
        let (_t, d) = sample();
        d.create("mio", "base").unwrap();
        assert!(d.user.join("mio/backgrounds/a.png").is_file());
        assert!(d.create("mio", "base").is_err());
        assert!(d.create("otro", "nope").is_err());
        assert!(d.delete("base").is_err(), "los de Omarchy no se borran");
        assert!(d.delete("../system").is_err());
        d.delete("mio").unwrap();
        assert!(d.system.join("base/colors.toml").is_file());
    }

    #[test]
    fn edits_colors_in_place() {
        let text = "mode = \"dark\"\n# c\naccent = \"#111111\"\n";
        let out = set_colors(
            text,
            &[
                ("accent".into(), "#e31b23".into()),
                ("brown".into(), "#75".into()),
            ],
        );
        assert!(out.contains("# c\naccent = \"#e31b23\"\n"));
        assert!(out.ends_with("brown = \"#75\"\n"));
        assert_eq!(set_colors(text, &[]), text);
    }

    #[test]
    fn parses_and_renders_borders() {
        let b = Border::parse("rgba(26a269ee) rgba(2ec27eee) 45deg").unwrap();
        assert_eq!(b.from, "#26a269");
        assert_eq!(b.to.as_deref(), Some("#2ec27e"));
        assert_eq!(b.angle, Some(45));
        assert_eq!(b.render(), "rgba(26a269ff) rgba(2ec27eff) 45deg");
        assert_eq!(Border::parse("#B3171E").unwrap().render(), "#B3171E");
        assert_eq!(Border::parse("rgb(1e1e1e)").unwrap().from, "#1e1e1e");
        assert!(Border::parse("nope").is_none());
    }

    #[test]
    fn spec_roundtrips_a_theme() {
        let (t, d) = sample();
        let mut spec = ThemeSpec::load(&d.system.join("base")).unwrap();
        assert_eq!(spec.icons, "Yaru-blue");
        assert_eq!(spec.active_border.angle, Some(45));
        spec.colors.insert("accent".into(), "#e31b23".into());
        spec.icons = "Papirus".into();
        spec.neovim = Some("return {}\n".into());
        spec.keyboard = Some("e31b23".into());
        spec.active_border = Border::solid("#e31b23");
        let dest = t.path().join("user/nuevo");
        spec.save(&dest, Some(&d.system.join("base"))).unwrap();

        let back = ThemeSpec::load(&dest).unwrap();
        assert_eq!(back.colors.get("accent").unwrap(), "#e31b23");
        assert_eq!(back.icons, "Papirus");
        assert_eq!(back.keyboard.as_deref(), Some("e31b23"));
        assert_eq!(back.neovim.as_deref(), Some("return {}\n"));
        assert_eq!(back.active_border, Border::solid("#e31b23").tap_upper());
        assert_eq!(back.backgrounds.len(), 1);
        // el hyprland.lua propio sigue los bordes nuevos
        let lua = std::fs::read_to_string(dest.join("hyprland.lua")).unwrap();
        assert!(
            lua.contains("local active_border_color = \"#E31B23\""),
            "{lua}"
        );
        assert!(lua.contains("hl.config({})"));
        // y el de Omarchy no se tocó
        assert_eq!(
            std::fs::read_to_string(d.system.join("base/icons.theme"))
                .unwrap()
                .trim(),
            "Yaru-blue"
        );
    }

    #[test]
    fn saving_twice_is_stable_and_replaces_backgrounds() {
        let (t, d) = sample();
        let spec = ThemeSpec::load(&d.system.join("base")).unwrap();
        let dest = t.path().join("user/x");
        spec.save(&dest, Some(&d.system.join("base"))).unwrap();
        let first = std::fs::read_to_string(dest.join("colors.toml")).unwrap();
        let mut again = ThemeSpec::load(&dest).unwrap();
        again.save(&dest, None).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("colors.toml")).unwrap(),
            first
        );
        // quitar el fondo borra la copia del tema propio, no la de Omarchy
        again.backgrounds.clear();
        again.save(&dest, None).unwrap();
        assert!(!dest.join("backgrounds/a.png").exists());
        assert!(d.system.join("base/backgrounds/a.png").exists());
    }

    #[test]
    fn surfaces_are_written_and_read() {
        let (t, _d) = sample();
        let mut colors = BTreeMap::new();
        colors.insert("background".to_string(), "#D8C79E".to_string());
        colors.insert("accent".to_string(), "#B3171E".to_string());
        let spec = ThemeSpec {
            mode: "light".into(),
            colors: colors.clone(),
            active_border: Border::solid("#B3171E"),
            inactive_border: Border::solid("#B8A57A"),
            icons: String::new(),
            neovim: None,
            vscode: None,
            keyboard: None,
            backgrounds: vec![],
            unlock: None,
            preview: None,
            surfaces: Some(Surfaces::from_palette(&colors)),
        };
        let dest = t.path().join("t");
        spec.save(&dest, None).unwrap();
        let s = Surfaces::load(&dest).unwrap();
        assert_eq!(s.bar_bg, "#D8C79E");
        assert_eq!(s.card_border, "#B3171E");
    }

    #[test]
    fn loads_every_installed_omarchy_theme() {
        // Si Omarchy está instalado, todos sus temas se leen y se guardan sin perder nada.
        let sys = Path::new("/usr/share/omarchy/themes");
        let Ok(rd) = std::fs::read_dir(sys) else {
            return;
        };
        let t = tempfile::tempdir().unwrap();
        for e in rd.flatten().filter(|e| e.path().is_dir()) {
            let spec =
                ThemeSpec::load(&e.path()).unwrap_or_else(|er| panic!("{:?}: {er}", e.path()));
            let dest = t.path().join(e.file_name());
            spec.save(&dest, Some(&e.path())).unwrap();
            let back = ThemeSpec::load(&dest).unwrap();
            assert_eq!(back.colors, spec.colors, "{:?}", e.file_name());
            assert_eq!(back.mode, spec.mode);
            assert_eq!(back.icons, spec.icons);
        }
    }

    trait TapUpper {
        fn tap_upper(self) -> Self;
    }
    impl TapUpper for Border {
        fn tap_upper(mut self) -> Self {
            self.from = self.from.to_lowercase();
            self
        }
    }
}
