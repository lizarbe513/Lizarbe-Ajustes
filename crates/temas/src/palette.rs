//! Generación de paletas: a partir de un color de acento (y el modo) o de los
//! colores dominantes de una imagen. Siempre con contraste suficiente.

use std::collections::BTreeMap;

use lizarbe_core::color::{contrast, from_hsl, to_hex, to_hsl, with_contrast};

type Rgb = (u8, u8, u8);

fn put(m: &mut BTreeMap<String, String>, k: &str, c: Rgb) {
    m.insert(k.to_string(), to_hex(c));
}

/// Paleta completa desde un acento. `tint` (tono, saturación) tiñe los fondos;
/// si es `None` se usa el tono del acento.
pub fn from_accent(accent: Rgb, light: bool, tint: Option<(f32, f32)>) -> BTreeMap<String, String> {
    let (ah, as_, _al) = to_hsl(accent);
    let (th, ts) = tint.unwrap_or((ah, (as_ * 0.5).clamp(0.08, 0.28)));
    let mut m = BTreeMap::new();
    let bg = if light {
        from_hsl(th, ts.max(0.12), 0.93)
    } else {
        from_hsl(th, ts, 0.10)
    };
    let (dark_bg, darker_bg, lighter_bg) = if light {
        (
            from_hsl(th, ts.max(0.12), 0.89),
            from_hsl(th, ts.max(0.12), 0.84),
            from_hsl(th, ts.max(0.10), 0.97),
        )
    } else {
        (
            from_hsl(th, ts, 0.075),
            from_hsl(th, ts, 0.055),
            from_hsl(th, ts, 0.16),
        )
    };
    let fg = with_contrast(
        if light {
            from_hsl(th, 0.18, 0.18)
        } else {
            from_hsl(th, 0.14, 0.78)
        },
        bg,
        7.0,
    );
    let muted = with_contrast(
        if light {
            from_hsl(th, 0.14, 0.40)
        } else {
            from_hsl(th, 0.14, 0.50)
        },
        bg,
        4.0,
    );
    let light_fg = if light {
        from_hsl(th, 0.18, 0.12)
    } else {
        from_hsl(th, 0.14, 0.84)
    };
    let bright_fg = if light {
        from_hsl(th, 0.18, 0.05)
    } else {
        from_hsl(th, 0.14, 0.91)
    };
    let selection = if light {
        from_hsl(ah, 0.45, 0.86)
    } else {
        from_hsl(ah, (as_ * 0.45).clamp(0.1, 0.4), 0.22)
    };
    let acc = with_contrast(accent, bg, 3.0);

    put(&mut m, "background", bg);
    put(&mut m, "dark_background", dark_bg);
    put(&mut m, "darker_background", darker_bg);
    put(&mut m, "lighter_background", lighter_bg);
    put(&mut m, "foreground", fg);
    put(&mut m, "dark_foreground", muted);
    put(&mut m, "light_foreground", light_fg);
    put(&mut m, "bright_foreground", bright_fg);
    put(&mut m, "muted", muted);
    put(&mut m, "selection", selection);
    put(&mut m, "accent", acc);

    // Los 16 colores: tonos fijos con una pizca del acento para que armonicen.
    let nudge = |h: f32| h + (((ah - h + 540.0).rem_euclid(360.0)) - 180.0) * 0.06;
    let hues = [
        ("red", 355.0),
        ("orange", 22.0),
        ("yellow", 44.0),
        ("green", 135.0),
        ("cyan", 186.0),
        ("blue", 217.0),
        ("magenta", 292.0),
        ("brown", 18.0),
    ];
    for (name, hue) in hues {
        let (s, l, lb) = if light {
            (0.62, 0.36, 0.44)
        } else {
            (0.62, 0.66, 0.74)
        };
        let (s, l) = if name == "brown" {
            (0.42, if light { 0.30 } else { 0.38 })
        } else {
            (s, l)
        };
        let min = if name == "brown" { 3.0 } else { 4.5 };
        let c = with_contrast(from_hsl(nudge(hue), s, l), bg, min);
        put(&mut m, name, c);
        if !matches!(name, "orange" | "brown") {
            let b = with_contrast(from_hsl(nudge(hue), (s + 0.06).min(1.0), lb), bg, min);
            put(&mut m, &format!("bright_{name}"), b);
        }
    }
    m
}

/// Colores dominantes (k-means determinista) de una lista de píxeles:
/// (color, cuántos píxeles), de más a menos frecuente.
pub fn dominant(pixels: &[Rgb], k: usize) -> Vec<(Rgb, usize)> {
    if pixels.is_empty() || k == 0 {
        return vec![];
    }
    // Centros iniciales: el píxel más cercano a la media y después, cada vez, el
    // más lejano a los ya elegidos (sin azar → resultado estable).
    let k = k.min(pixels.len());
    let f = |p: &Rgb| [p.0 as f32, p.1 as f32, p.2 as f32];
    let n = pixels.len() as f32;
    let mean = pixels.iter().fold([0f32; 3], |a, p| {
        let v = f(p);
        [a[0] + v[0] / n, a[1] + v[1] / n, a[2] + v[2] / n]
    });
    let first = pixels
        .iter()
        .min_by(|a, b| dist(&f(a), &mean).total_cmp(&dist(&f(b), &mean)))
        .copied()
        .unwrap_or(pixels[0]);
    let mut centers: Vec<[f32; 3]> = vec![f(&first)];
    while centers.len() < k {
        let far = pixels
            .iter()
            .max_by(|a, b| {
                let da = centers
                    .iter()
                    .map(|c| dist(c, &f(a)))
                    .fold(f32::MAX, f32::min);
                let db = centers
                    .iter()
                    .map(|c| dist(c, &f(b)))
                    .fold(f32::MAX, f32::min);
                da.total_cmp(&db)
            })
            .copied()
            .unwrap_or(pixels[0]);
        centers.push(f(&far));
    }
    let mut counts = vec![0usize; k];
    for _ in 0..12 {
        let mut sums = vec![[0f32; 3]; k];
        counts.iter_mut().for_each(|c| *c = 0);
        for p in pixels {
            let v = [p.0 as f32, p.1 as f32, p.2 as f32];
            let best = (0..k)
                .min_by(|&a, &b| dist(&centers[a], &v).total_cmp(&dist(&centers[b], &v)))
                .unwrap_or(0);
            counts[best] += 1;
            for c in 0..3 {
                sums[best][c] += v[c];
            }
        }
        for i in 0..k {
            if counts[i] > 0 {
                for c in 0..3 {
                    centers[i][c] = sums[i][c] / counts[i] as f32;
                }
            }
        }
    }
    let mut out: Vec<(Rgb, usize)> = centers
        .iter()
        .zip(&counts)
        .filter(|(_, n)| **n > 0)
        .map(|(c, n)| {
            (
                (c[0].round() as u8, c[1].round() as u8, c[2].round() as u8),
                *n,
            )
        })
        .collect();
    out.sort_by_key(|b| std::cmp::Reverse(b.1));
    out
}

fn dist(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Paleta desde los píxeles de una imagen: el modo sale de su luminosidad
/// media, el acento del color más vivo y los fondos se tiñen del tono dominante.
pub fn from_pixels(pixels: &[Rgb]) -> Option<(BTreeMap<String, String>, bool)> {
    let centers = dominant(pixels, 6);
    if centers.is_empty() {
        return None;
    }
    let total: usize = centers.iter().map(|c| c.1).sum();
    let mean: f32 = centers
        .iter()
        .map(|(c, n)| lizarbe_core::color::luminance(*c) * *n as f32)
        .sum::<f32>()
        / total as f32;
    let light = mean > 0.45;
    // Acento: el más saturado entre los de luminosidad media, ponderado por presencia.
    let accent = centers
        .iter()
        .filter_map(|(c, n)| {
            let (_, s, l) = to_hsl(*c);
            (0.25..0.8)
                .contains(&l)
                .then_some((*c, s * (*n as f32 / total as f32).sqrt().max(0.2)))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|a| a.0)
        .unwrap_or(centers[0].0);
    // Tono de los fondos: el del color dominante de la imagen.
    let (bh, bs, _) = to_hsl(centers[0].0);
    let tint = (bh, bs.clamp(0.06, 0.3));
    let mut m = from_accent(accent, light, Some(tint));
    // el acento nunca debe quedarse sin contraste con el fondo generado
    let bg = lizarbe_core::color::parse_hex(&m["background"]).unwrap_or((0, 0, 0));
    let acc = lizarbe_core::color::parse_hex(&m["accent"]).unwrap_or(accent);
    if contrast(acc, bg) < 3.0 {
        m.insert("accent".into(), to_hex(with_contrast(acc, bg, 3.0)));
    }
    Some((m, light))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lizarbe_core::color::parse_hex;

    fn rgb(m: &BTreeMap<String, String>, k: &str) -> Rgb {
        parse_hex(&m[k]).unwrap_or_else(|| panic!("falta {k}"))
    }

    #[test]
    fn dark_palette_is_readable() {
        let m = from_accent((227, 27, 35), false, None);
        let bg = rgb(&m, "background");
        assert!(lizarbe_core::color::luminance(bg) < 0.05);
        for k in [
            "foreground",
            "red",
            "green",
            "yellow",
            "blue",
            "magenta",
            "cyan",
            "bright_red",
            "bright_cyan",
        ] {
            assert!(
                contrast(rgb(&m, k), bg) >= 4.4,
                "{k}: {}",
                contrast(rgb(&m, k), bg)
            );
        }
        assert!(contrast(rgb(&m, "muted"), bg) >= 3.9);
        assert_eq!(m.len(), 25, "{m:?}");
    }

    #[test]
    fn light_palette_has_light_backgrounds_and_dark_text() {
        let m = from_accent((179, 23, 30), true, None);
        let bg = rgb(&m, "background");
        assert!(lizarbe_core::color::luminance(bg) > 0.6);
        assert!(contrast(rgb(&m, "foreground"), bg) >= 7.0);
        assert!(contrast(rgb(&m, "green"), bg) >= 4.4);
    }

    #[test]
    fn dominant_colors_are_stable() {
        let mut px = vec![(10u8, 10, 30); 300];
        px.extend(vec![(200, 40, 40); 120]);
        px.extend(vec![(240, 240, 240); 40]);
        let a = dominant(&px, 3);
        let b = dominant(&px, 3);
        assert_eq!(a, b);
        assert_eq!(a[0].0, (10, 10, 30));
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn palette_from_pixels_picks_dark_mode_and_a_vivid_accent() {
        let mut px = vec![(12u8, 14, 28); 400];
        px.extend(vec![(30, 160, 220); 80]);
        px.extend(vec![(60, 60, 70); 100]);
        let (m, light) = from_pixels(&px).unwrap();
        assert!(!light);
        let (h, s, _) = to_hsl(rgb(&m, "accent"));
        assert!(s > 0.4 && (190.0..215.0).contains(&h), "{h} {s}");
        assert!(from_pixels(&[]).is_none());
    }
}
