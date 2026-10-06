//! Paleta de la interfaz tomada del tema activo de Omarchy (`colors.toml`).

use std::collections::HashMap;
use std::path::Path;

use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Palette {
    pub fg: Color,
    pub accent: Color,
    pub muted: Color,
    pub dim: Color,
    pub selection: Color,
    pub surface: Color,
    pub warn: Color,
    pub ok: Color,
    pub error: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            fg: Color::Reset,
            accent: Color::Cyan,
            muted: Color::DarkGray,
            dim: Color::DarkGray,
            selection: Color::Rgb(40, 40, 40),
            surface: Color::Rgb(26, 26, 26),
            warn: Color::Yellow,
            ok: Color::Green,
            error: Color::Red,
        }
    }
}

pub fn hex(s: &str) -> Option<Color> {
    let h = s.trim().trim_start_matches('#');
    let h = if h.len() == 8 { &h[..6] } else { h };
    if h.len() == 3 {
        let p = |i: usize| u8::from_str_radix(&h[i..i + 1].repeat(2), 16).ok();
        return Some(Color::Rgb(p(0)?, p(1)?, p(2)?));
    }
    if h.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some(Color::Rgb(p(0)?, p(2)?, p(4)?))
}

impl Palette {
    pub fn load(colors_toml: &Path) -> Palette {
        let mut pal = Palette::default();
        let Ok(text) = std::fs::read_to_string(colors_toml) else {
            return pal;
        };
        let Ok(map) = toml::from_str::<HashMap<String, toml::Value>>(&text) else {
            return pal;
        };
        let c = |k: &str| map.get(k).and_then(|v| v.as_str()).and_then(hex);
        // El fondo se deja en Reset: la terminal ya usa el del tema y así se
        // respeta su transparencia.
        if let Some(x) = c("foreground") {
            pal.fg = x;
        }
        if let Some(x) = c("accent") {
            pal.accent = x;
        }
        if let Some(x) = c("muted").or(c("dark_foreground")) {
            pal.muted = x;
        }
        if let Some(x) = c("dark_foreground") {
            pal.dim = x;
        }
        if let Some(x) = c("selection").or(c("lighter_background")) {
            pal.selection = x;
        }
        if let Some(x) = c("lighter_background") {
            pal.surface = x;
        }
        if let Some(x) = c("yellow") {
            pal.warn = x;
        }
        if let Some(x) = c("green") {
            pal.ok = x;
        }
        if let Some(x) = c("red") {
            pal.error = x;
        }
        pal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex() {
        assert_eq!(hex("#8d8d8d"), Some(Color::Rgb(0x8d, 0x8d, 0x8d)));
        assert_eq!(hex("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(hex("#11223344"), Some(Color::Rgb(0x11, 0x22, 0x33)));
        assert_eq!(hex("nope"), None);
    }
}
