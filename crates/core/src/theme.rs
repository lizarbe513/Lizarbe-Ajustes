//! Paleta de la interfaz tomada del tema activo de Omarchy (`colors.toml`).
//!
//! Además de los colores del tema se derivan `rule` (líneas finas) y `ok`; el
//! texto atenuado y el apagado se acercan al texto normal si no contrastan lo
//! bastante con el fondo, para que el diseño se lea con cualquier tema.

use std::collections::HashMap;
use std::path::Path;

use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Palette {
    /// Fondo del tema (para derivar tonos y comprobar el contraste).
    pub bg: Color,
    pub fg: Color,
    pub bright: Color,
    pub accent: Color,
    pub muted: Color,
    pub dim: Color,
    pub warn: Color,
    /// Verde del tema: estados correctos (activo, aplicado).
    pub ok: Color,
    /// Líneas finas de la rejilla: apenas más claras que el fondo.
    pub rule: Color,
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

/// Luminancia relativa (WCAG) de un color.
fn luminance(c: (u8, u8, u8)) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.0) + 0.7152 * lin(c.1) + 0.0722 * lin(c.2)
}

/// Relación de contraste (1 a 21) entre dos colores.
pub fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Acerca `c` a `toward` hasta que contraste al menos `min` con `bg`
/// (si ya contrasta lo suficiente, no lo toca).
pub fn ensure_contrast(
    c: (u8, u8, u8),
    bg: (u8, u8, u8),
    toward: (u8, u8, u8),
    min: f32,
) -> (u8, u8, u8) {
    let mut out = c;
    for step in 1..=20 {
        if contrast(out, bg) >= min {
            break;
        }
        let ratio = step as f32 / 20.0;
        out = (
            (c.0 as f32 * (1.0 - ratio) + toward.0 as f32 * ratio).round() as u8,
            (c.1 as f32 * (1.0 - ratio) + toward.1 as f32 * ratio).round() as u8,
            (c.2 as f32 * (1.0 - ratio) + toward.2 as f32 * ratio).round() as u8,
        );
    }
    out
}

fn blend_rgb(a: (u8, u8, u8), b: (u8, u8, u8), ratio: f32) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - ratio) + y as f32 * ratio).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
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
        let muted = map
            .get("muted")
            .or(map.get("dark_foreground"))
            .and_then(|v| rgb(v))
            .unwrap_or((0x58, 0x5b, 0x70));
        let dim = get("dark_foreground", "#505050");
        let warn = get("yellow", "#e0af68");
        let ok = get("green", "#9ece6a");

        // Texto atenuado y apagado siempre legibles sobre el fondo del tema.
        let toward = if light { (0, 0, 0) } else { (255, 255, 255) };
        let muted_t = ensure_contrast(muted, bg, toward, 4.0);
        let dim_t = ensure_contrast(dim, bg, toward, 2.4);
        let rule = ensure_contrast(blend_rgb(bg, muted_t, 0.4), bg, toward, 1.5);
        let c = |x: (u8, u8, u8)| Color::Rgb(x.0, x.1, x.2);
        Palette {
            bg: c(bg),
            fg: c(fg),
            bright: c(bright),
            accent: c(accent),
            muted: c(muted_t),
            dim: c(dim_t),
            warn: c(warn),
            ok: c(ok),
            rule: c(rule),
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
    fn keeps_quiet_text_legible_in_any_theme() {
        // Un atenuado casi igual al fondo se acerca al texto hasta ser legible.
        assert!(contrast((20, 20, 24), (8, 8, 11)) < 2.0);
        let c = ensure_contrast((20, 20, 24), (8, 8, 11), (255, 255, 255), 4.0);
        assert!(contrast(c, (8, 8, 11)) >= 4.0);
        // Lo que ya contrasta no se toca.
        assert_eq!(
            ensure_contrast((200, 200, 200), (0, 0, 0), (255, 255, 255), 4.0),
            (200, 200, 200)
        );
        // En un tema claro se oscurece.
        let c = ensure_contrast((230, 220, 200), (216, 199, 158), (0, 0, 0), 4.0);
        assert!(contrast(c, (216, 199, 158)) >= 4.0);
    }

    #[test]
    fn palette_always_has_visible_rules() {
        for (bg, mode) in [("#08080b", "dark"), ("#d8c79e", "light")] {
            let map: HashMap<String, String> = [("background", bg), ("mode", mode)]
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let p = Palette::from_map(&map);
            let (Color::Rgb(a, b, c), Color::Rgb(x, y, z)) = (p.bg, p.rule) else {
                panic!()
            };
            assert!(contrast((a, b, c), (x, y, z)) >= 1.4, "{bg}");
        }
    }
}
