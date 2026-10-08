//! Códigos QR dibujados con medios bloques (dos módulos por celda de texto).

use qrcode::{Color as Modulo, EcLevel, QrCode};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

/// Matriz de módulos (`true` = oscuro) del código QR de `url`.
pub fn matriz(url: &str) -> Option<Vec<Vec<bool>>> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::M).ok()?;
    let n = code.width();
    let colores = code.to_colors();
    Some(
        (0..n)
            .map(|y| (0..n).map(|x| colores[y * n + x] == Modulo::Dark).collect())
            .collect(),
    )
}

/// Líneas listas para pintar. Módulos oscuros sobre fondo claro (así lo leen
/// todas las cámaras), con 2 módulos de margen.
pub fn lineas(url: &str, oscuro: Color, claro: Color) -> Vec<Line<'static>> {
    let Some(m) = matriz(url) else {
        return vec![];
    };
    let margen = 2;
    let n = m.len() + margen * 2;
    let modulo = |x: usize, y: usize| -> bool {
        if x < margen || y < margen || x >= n - margen || y >= n - margen {
            false
        } else {
            m[y - margen][x - margen]
        }
    };
    let mut out = vec![];
    let mut y = 0;
    while y < n {
        let mut spans = vec![];
        for x in 0..n {
            let arriba = modulo(x, y);
            let abajo = y + 1 < n && modulo(x, y + 1);
            let c = |dark: bool| if dark { oscuro } else { claro };
            spans.push(Span::styled("▀", Style::new().fg(c(arriba)).bg(c(abajo))));
        }
        out.push(Line::from(spans));
        y += 2;
    }
    out
}

/// Ancho en celdas de un QR (con margen).
pub fn ancho(url: &str) -> usize {
    matriz(url).map_or(0, |m| m.len() + 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://kdeconnect.kde.org/download.html";

    #[test]
    fn matrix_is_square_with_finder_patterns() {
        let m = matriz(URL).unwrap();
        let n = m.len();
        assert!((25..=45).contains(&n), "{n}");
        assert!(m.iter().all(|f| f.len() == n));
        // Los tres cuadrados de posición: esquina de 7×7 con borde oscuro.
        for (ox, oy) in [(0, 0), (n - 7, 0), (0, n - 7)] {
            for i in 0..7 {
                assert!(m[oy][ox + i] && m[oy + 6][ox + i], "borde horizontal");
                assert!(m[oy + i][ox] && m[oy + i][ox + 6], "borde vertical");
            }
            assert!(m[oy + 3][ox + 3], "centro");
            assert!(!m[oy + 1][ox + 1], "anillo claro");
        }
    }

    #[test]
    fn lines_cover_the_matrix_with_margin() {
        let l = lineas(URL, Color::Black, Color::White);
        let n = matriz(URL).unwrap().len() + 4;
        assert_eq!(l.len(), n.div_ceil(2));
        assert!(l.iter().all(|x| x.width() == n));
        assert_eq!(ancho(URL), n);
    }

    #[test]
    fn longer_text_makes_a_bigger_code() {
        let corta = matriz("hola").unwrap().len();
        let larga = matriz(&"x".repeat(120)).unwrap().len();
        assert!(larga > corta);
    }
}
