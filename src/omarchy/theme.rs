//! Paleta de la interfaz tomada del tema activo de Omarchy (`colors.toml`).
//!
//! Igual que en Meca, además de los colores del tema se derivan tonos suaves
//! (`soft_hover`, `soft_selection`, `soft_muted`) mezclando el fondo con el
//! acento, la selección y el color atenuado: así el destello al pasar el ratón
//! no deslumbra en temas oscuros y el texto sigue siendo legible.

use std::collections::HashMap;
use std::path::Path;

use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Palette {
    pub fg: Color,
    pub bright: Color,
    pub accent: Color,
    pub muted: Color,
    pub dim: Color,
    pub surface: Color,
    pub warn: Color,
    /// Fondo bajo el cursor del ratón.
    pub soft_hover: Color,
    /// Fondo del elemento seleccionado.
    pub soft_selection: Color,
    /// Fondo atenuado (sidebar al pasar el ratón, barras).
    pub soft_muted: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Palette::from_map(&HashMap::new())
    }
}

pub fn hex(s: &str) -> Option<Color> {
    let (r, g, b) = rgb(s)?;
    Some(Color::Rgb(r, g, b))
}

fn rgb(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    let h = if h.len() == 8 { &h[..6] } else { h };
    if h.len() == 3 {
        let p = |i: usize| u8::from_str_radix(&h[i..i + 1].repeat(2), 16).ok();
        return Some((p(0)?, p(1)?, p(2)?));
    }
    if h.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

/// Mezcla `a` con `b` en la proporción `ratio` (0 = a, 1 = b).
pub fn blend(a: (u8, u8, u8), b: (u8, u8, u8), ratio: f32) -> Color {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - ratio) + y as f32 * ratio).round() as u8;
    Color::Rgb(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

impl Palette {
    pub fn load(colors_toml: &Path) -> Palette {
        let map: HashMap<String, String> = std::fs::read_to_string(colors_toml)
            .ok()
            .and_then(|t| toml::from_str::<HashMap<String, toml::Value>>(&t).ok())
            .map(|m| {
                m.into_iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Palette::from_map(&map)
    }

    fn from_map(map: &HashMap<String, String>) -> Palette {
        let get = |k: &str, d: &str| {
            map.get(k)
                .and_then(|v| rgb(v))
                .unwrap_or_else(|| rgb(d).unwrap())
        };
        let light = map.get("mode").is_some_and(|m| m == "light");
        let bg = get("background", if light { "#f5f5f5" } else { "#08080b" });
        let fg = get("foreground", if light { "#202020" } else { "#d0d0d0" });
        let bright = map
            .get("bright_foreground")
            .and_then(|v| rgb(v))
            .unwrap_or(if light { fg } else { (255, 255, 255) });
        let accent = get("accent", "#e31b23");
        let selection = get("selection", "#45475a");
        let muted = map
            .get("muted")
            .or(map.get("dark_foreground"))
            .and_then(|v| rgb(v))
            .unwrap_or((0x58, 0x5b, 0x70));
        let dim = get("dark_foreground", "#505050");
        let surface = get("lighter_background", "#1a1a1a");
        let warn = get("yellow", "#e0af68");

        // Mismas proporciones que el motor de temas de Meca.
        let (soft_hover, soft_selection, soft_muted) = if light {
            (
                blend(bg, accent, 0.16),
                blend(bg, selection, 0.45),
                blend(bg, muted, 0.22),
            )
        } else {
            let lum = 0.299 * selection.0 as f32
                + 0.587 * selection.1 as f32
                + 0.114 * selection.2 as f32;
            let sel_ratio = if lum > 85.0 { 0.24 } else { 0.65 };
            (
                blend(bg, accent, 0.22),
                blend(bg, selection, sel_ratio),
                blend(bg, muted, 0.38),
            )
        };
        let c = |x: (u8, u8, u8)| Color::Rgb(x.0, x.1, x.2);
        Palette {
            fg: c(fg),
            bright: c(bright),
            accent: c(accent),
            muted: c(muted),
            dim: c(dim),
            surface: c(surface),
            warn: c(warn),
            soft_hover,
            soft_selection,
            soft_muted,
        }
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

    #[test]
    fn derives_soft_colors_like_meca() {
        let map: HashMap<String, String> = [
            ("background", "#000000"),
            ("accent", "#ff0000"),
            ("selection", "#ffffff"),
            ("muted", "#646464"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let p = Palette::from_map(&map);
        assert_eq!(p.soft_hover, Color::Rgb(56, 0, 0)); // 22 % de acento
        assert_eq!(p.soft_selection, Color::Rgb(61, 61, 61)); // selección clara: 24 %
        assert_eq!(p.soft_muted, Color::Rgb(38, 38, 38)); // 38 % de muted
    }
}
