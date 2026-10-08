//! Identidad visual de Lizarbe para la Bienvenida: paleta oficial (fija, no
//! depende del tema de Omarchy), isotipo y piezas tipográficas.
//! Reglas: rojo solo para marca, cursor activo y llamadas a la acción; ✦ como
//! marca; tramas ░▒▓█ en vez de degradados; esquinas rectas.

use lizarbe_core::ansi;
use lizarbe_core::color::mix;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

pub const BG: Color = Color::Rgb(0x0D, 0x0F, 0x12);
pub const INK: Color = Color::Rgb(0x12, 0x14, 0x18);
pub const RED: Color = Color::Rgb(0xF6, 0x24, 0x24);
pub const STEEL: Color = Color::Rgb(0x32, 0x36, 0x3C);
pub const SLATE: Color = Color::Rgb(0x55, 0x5A, 0x62);
pub const SILVER: Color = Color::Rgb(0xAA, 0xB2, 0xBE);
pub const PHOSPHOR: Color = Color::Rgb(0xF0, 0xF3, 0xF8);
pub const GREEN: Color = Color::Rgb(0x1A, 0xB0, 0x54);
pub const CADMIUM: Color = Color::Rgb(0xF8, 0xD8, 0x18);
pub const COBALT: Color = Color::Rgb(0x4B, 0x8E, 0xFF);

/// La marca de Lizarbe: el destello.
pub const SPARK: &str = "✦";

pub fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

pub fn bold(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

pub fn rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (0, 0, 0),
    }
}

/// Mezcla `a` hacia `b` (0 = a, 1 = b).
pub fn blend(a: Color, b: Color, t: f32) -> Color {
    let (r, g, bl) = mix(rgb(a), rgb(b), t.clamp(0.0, 1.0));
    Color::Rgb(r, g, bl)
}

/// El isotipo (cubo con destello) en medios bloques de color verdadero.
pub fn isotipo() -> Vec<Line<'static>> {
    ansi::parse(include_str!("../assets/isotipo.ansi"))
}

/// Letras del logotipo en una rejilla de 5×5 puntos.
fn letra(c: char) -> [&'static str; 5] {
    match c {
        'L' => ["█....", "█....", "█....", "█....", "█████"],
        'I' => ["█████", "..█..", "..█..", "..█..", "█████"],
        'Z' => ["█████", "...█.", "..█..", ".█...", "█████"],
        'A' => [".███.", "█...█", "█████", "█...█", "█...█"],
        'R' => ["████.", "█...█", "████.", "█..█.", "█...█"],
        'B' => ["████.", "█...█", "████.", "█...█", "████."],
        'E' => ["█████", "█....", "████.", "█....", "█████"],
        _ => [".....", ".....", ".....", ".....", "....."],
    }
}

/// «LIZARBE» en tres filas de texto (medios bloques). `ancho` = columnas por punto (1 o 2).
pub fn logotipo(ancho: usize) -> Vec<String> {
    let texto = "LIZARBE";
    // 5 filas de puntos → 3 filas de texto (dos puntos por celda).
    let mut filas = vec![String::new(); 3];
    for (n, c) in texto.chars().enumerate() {
        let g = letra(c);
        for (r, fila) in filas.iter_mut().enumerate() {
            let arriba: Vec<char> = g[r * 2].chars().collect();
            let abajo: Vec<char> = g
                .get(r * 2 + 1)
                .map_or(vec!['.'; 5], |s| s.chars().collect());
            for k in 0..5 {
                let ch = match (arriba[k] == '█', abajo[k] == '█') {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    _ => ' ',
                };
                for _ in 0..ancho {
                    fila.push(ch);
                }
            }
            if n + 1 < texto.len() {
                for _ in 0..ancho {
                    fila.push(' ');
                }
            }
        }
    }
    filas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordmark_has_three_rows_of_equal_width() {
        for ancho in [1, 2] {
            let l = logotipo(ancho);
            assert_eq!(l.len(), 3);
            let w = l[0].chars().count();
            assert_eq!(w, (7 * 5 + 6) * ancho);
            assert!(l.iter().all(|r| r.chars().count() == w));
            assert!(l.iter().any(|r| r.contains('█')));
        }
    }

    #[test]
    fn isotipo_is_the_real_logo() {
        let l = isotipo();
        assert!(l.len() >= 12);
        assert!(
            l.iter()
                .flat_map(|x| &x.spans)
                .any(|s| s.style.fg == Some(Color::Rgb(252, 22, 28)))
        );
    }

    #[test]
    fn brand_blend_moves_between_colors() {
        assert_eq!(blend(BG, PHOSPHOR, 0.0), BG);
        assert_eq!(blend(BG, PHOSPHOR, 1.0), PHOSPHOR);
    }
}
