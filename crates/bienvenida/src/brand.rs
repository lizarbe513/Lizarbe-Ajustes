//! Identidad visual de Lizarbe para la Bienvenida: los colores salen del tema
//! activo de Omarchy (y cambian en vivo con él); si no hay tema legible se usa
//! la paleta de marca. Reglas: el acento solo para marca, cursor activo y
//! llamadas a la acción; ✦ como marca; tramas ░▒▓█ en vez de degradados;
//! esquinas rectas.

use std::sync::RwLock;

use lizarbe_core::ansi;
use lizarbe_core::color::mix;
use lizarbe_core::theme::Palette;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

#[derive(Clone, Copy)]
struct Colores {
    bg: Color,
    rojo: Color,
    acero: Color,
    pizarra: Color,
    plata: Color,
    fosforo: Color,
    verde: Color,
    cadmio: Color,
    cobalto: Color,
}

/// Paleta oficial de la marca (también la de las pruebas y el modo demo).
const MARCA: Colores = Colores {
    bg: Color::Rgb(0x0D, 0x0F, 0x12),
    rojo: Color::Rgb(0xF6, 0x24, 0x24),
    acero: Color::Rgb(0x32, 0x36, 0x3C),
    pizarra: Color::Rgb(0x55, 0x5A, 0x62),
    plata: Color::Rgb(0xAA, 0xB2, 0xBE),
    fosforo: Color::Rgb(0xF0, 0xF3, 0xF8),
    verde: Color::Rgb(0x1A, 0xB0, 0x54),
    cadmio: Color::Rgb(0xF8, 0xD8, 0x18),
    cobalto: Color::Rgb(0x4B, 0x8E, 0xFF),
};

static TEMA: RwLock<Colores> = RwLock::new(MARCA);

fn actual() -> Colores {
    TEMA.read().map(|t| *t).unwrap_or(MARCA)
}

macro_rules! color {
    ($($nombre:ident),*) => {
        $(pub fn $nombre() -> Color { actual().$nombre })*
    };
}
color!(
    bg, rojo, acero, pizarra, plata, fosforo, verde, cadmio, cobalto
);

/// Un QR se lee mejor con módulos oscuros sobre claro, sea cual sea el tema.
pub const QR_OSCURO: Color = Color::Rgb(0x12, 0x14, 0x18);
pub const QR_CLARO: Color = Color::Rgb(0xF0, 0xF3, 0xF8);

/// Usa los colores del tema (acento, texto, apagado…).
pub fn aplicar_paleta(p: &Palette) {
    let t = Colores {
        bg: p.bg,
        rojo: p.accent,
        acero: p.dim,
        pizarra: p.muted,
        plata: p.fg,
        fosforo: p.bright,
        verde: p.ok,
        cadmio: p.warn,
        cobalto: blend(p.accent, p.fg, 0.5),
    };
    if let Ok(mut w) = TEMA.write() {
        *w = t;
    }
}

/// Vuelve a la paleta de marca (las pruebas lo usan tras probar un tema).
#[cfg(test)]
pub fn paleta_de_marca() {
    if let Ok(mut w) = TEMA.write() {
        *w = MARCA;
    }
}

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
/// El rojo de la marca del dibujo se cambia por el acento del tema.
pub fn isotipo() -> Vec<Line<'static>> {
    const ROJO_DEL_DIBUJO: Color = Color::Rgb(252, 22, 28);
    let mut lineas = ansi::parse(include_str!("../assets/isotipo.ansi"));
    for linea in &mut lineas {
        for span in &mut linea.spans {
            if span.style.fg == Some(ROJO_DEL_DIBUJO) {
                span.style.fg = Some(rojo());
            }
            if span.style.bg == Some(ROJO_DEL_DIBUJO) {
                span.style.bg = Some(rojo());
            }
        }
    }
    lineas
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
                .any(|s| s.style.fg == Some(rojo()))
        );
    }

    #[test]
    fn brand_blend_moves_between_colors() {
        assert_eq!(blend(MARCA.bg, MARCA.fosforo, 0.0), MARCA.bg);
        assert_eq!(blend(MARCA.bg, MARCA.fosforo, 1.0), MARCA.fosforo);
    }
}
