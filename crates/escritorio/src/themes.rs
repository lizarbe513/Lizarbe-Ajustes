//! Temas de Omarchy del usuario: crear (copiando uno existente), duplicar,
//! borrar y editar sus colores (`colors.toml`). Solo se escribe en la carpeta
//! de temas del usuario; los temas de Omarchy (`/usr/share/omarchy/themes`)
//! solo se leen.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

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
    /// Temas propios, de más nuevo a más viejo no importa: por nombre.
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

    pub fn colors_file(&self, slug: &str) -> PathBuf {
        self.user.join(slug).join("colors.toml")
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
fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
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

/// Colores y modo de un `colors.toml`.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs() -> (tempfile::TempDir, Dirs) {
        let t = tempfile::tempdir().unwrap();
        let d = Dirs {
            user: t.path().join("user"),
            system: t.path().join("system"),
        };
        std::fs::create_dir_all(d.system.join("base/backgrounds")).unwrap();
        std::fs::write(
            d.system.join("base/colors.toml"),
            "mode = \"dark\"\n\naccent = \"#111111\"\nbackground = \"#000000\"\n",
        )
        .unwrap();
        std::fs::write(d.system.join("base/backgrounds/a.png"), "x").unwrap();
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
        let (_t, d) = dirs();
        d.create("mio", "base").unwrap();
        assert!(d.user.join("mio/backgrounds/a.png").is_file());
        assert!(d.create("mio", "base").is_err(), "no pisa uno existente");
        assert!(d.create("otro", "nope").is_err());
        assert_eq!(d.list_all(), vec!["base".to_string(), "mio".to_string()]);
        assert!(d.delete("base").is_err(), "los de Omarchy no se borran");
        assert!(d.delete("../system").is_err());
        d.delete("mio").unwrap();
        assert!(!d.user.join("mio").exists());
        assert!(d.system.join("base/colors.toml").is_file());
    }

    #[test]
    fn edits_colors_in_place() {
        let text =
            "mode = \"dark\"\n# comentario\naccent = \"#111111\"\nbackground = \"#000000\"\n";
        assert_eq!(parse_colors(text).len(), 3);
        let out = set_colors(
            text,
            &[
                ("accent".into(), "#e31b23".into()),
                ("brown".into(), "#75493d".into()),
            ],
        );
        assert!(out.contains("# comentario\naccent = \"#e31b23\"\n"));
        assert!(out.contains("background = \"#000000\""));
        assert!(out.ends_with("brown = \"#75493d\"\n"));
        assert_eq!(set_colors(text, &[]), text);
    }
}
