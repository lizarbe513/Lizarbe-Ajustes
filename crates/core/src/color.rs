//! Conversiones de color (hex, RGB, HSL) y mezclas para editar paletas.

/// "#rrggbb" / "#rgb" / "rrggbb" → (r, g, b).
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    let h = if h.len() == 8 { &h[..6] } else { h };
    if h.len() == 3 {
        let p = |i: usize| u8::from_str_radix(&h[i..i + 1].repeat(2), 16).ok();
        return Some((p(0)?, p(1)?, p(2)?));
    }
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

pub fn to_hex(c: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// (r, g, b) → (h 0..360, s 0..1, l 0..1).
pub fn to_hsl(c: (u8, u8, u8)) -> (f32, f32, f32) {
    let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    (h, s.clamp(0.0, 1.0), l)
}

/// (h 0..360, s 0..1, l 0..1) → (r, g, b).
pub fn from_hsl(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let q = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (q(r1), q(g1), q(b1))
}

pub fn mix(a: (u8, u8, u8), b: (u8, u8, u8), ratio: f32) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| (x as f32 * (1.0 - ratio) + y as f32 * ratio).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Luminancia relativa (WCAG).
pub fn luminance(c: (u8, u8, u8)) -> f32 {
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

pub fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Mueve la luminosidad de `c` hasta contrastar al menos `min` con `bg`,
/// conservando el tono y la saturación.
pub fn with_contrast(c: (u8, u8, u8), bg: (u8, u8, u8), min: f32) -> (u8, u8, u8) {
    if contrast(c, bg) >= min {
        return c;
    }
    let (h, s, l) = to_hsl(c);
    let lighter = luminance(bg) < 0.5;
    let mut best = c;
    for step in 1..=100 {
        let nl = if lighter {
            l + step as f32 / 100.0
        } else {
            l - step as f32 / 100.0
        };
        let cand = from_hsl(h, s, nl.clamp(0.0, 1.0));
        best = cand;
        if contrast(cand, bg) >= min {
            break;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(parse_hex("#E31B23"), Some((227, 27, 35)));
        assert_eq!(parse_hex("fff"), Some((255, 255, 255)));
        assert_eq!(parse_hex("#11223344"), Some((0x11, 0x22, 0x33)));
        assert_eq!(parse_hex("zz"), None);
        assert_eq!(to_hex((227, 27, 35)), "#e31b23");
    }

    #[test]
    fn hsl_roundtrip_is_close() {
        for c in [
            (227, 27, 35),
            (10, 200, 120),
            (30, 30, 40),
            (255, 255, 255),
            (0, 0, 0),
        ] {
            let (h, s, l) = to_hsl(c);
            let back = from_hsl(h, s, l);
            for (a, b) in [(c.0, back.0), (c.1, back.1), (c.2, back.2)] {
                assert!((a as i32 - b as i32).abs() <= 1, "{c:?} → {back:?}");
            }
        }
    }

    #[test]
    fn with_contrast_reaches_the_minimum() {
        let bg = (26, 27, 38);
        let c = with_contrast((60, 62, 80), bg, 4.5);
        assert!(contrast(c, bg) >= 4.5);
        let bg = (216, 199, 158);
        let c = with_contrast((190, 170, 120), bg, 4.5);
        assert!(contrast(c, bg) >= 4.5);
        // lo que ya contrasta no cambia
        assert_eq!(with_contrast((0, 0, 0), bg, 4.5), (0, 0, 0));
    }
}
