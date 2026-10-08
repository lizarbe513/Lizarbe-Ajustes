//! Interpreta texto con secuencias ANSI de color (`ESC[…m`) y lo convierte
//! en líneas de ratatui: sirve para pintar el isotipo de Lizarbe (medios
//! bloques en color verdadero) y la salida de `fastfetch`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

fn apply_sgr(style: Style, params: &[u32]) -> Style {
    let mut st = style;
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => st = Style::default(),
            1 => st = st.add_modifier(Modifier::BOLD),
            2 => st = st.add_modifier(Modifier::DIM),
            3 => st = st.add_modifier(Modifier::ITALIC),
            4 => st = st.add_modifier(Modifier::UNDERLINED),
            22 => st = st.remove_modifier(Modifier::BOLD | Modifier::DIM),
            n @ 30..=37 => st = st.fg(Color::Indexed((n - 30) as u8)),
            n @ 90..=97 => st = st.fg(Color::Indexed((n - 90 + 8) as u8)),
            n @ 40..=47 => st = st.bg(Color::Indexed((n - 40) as u8)),
            n @ 100..=107 => st = st.bg(Color::Indexed((n - 100 + 8) as u8)),
            39 => st.fg = None,
            49 => st.bg = None,
            n @ (38 | 48) => {
                let color = match params.get(i + 1) {
                    Some(2) if i + 4 < params.len() => {
                        let c = Color::Rgb(
                            params[i + 2] as u8,
                            params[i + 3] as u8,
                            params[i + 4] as u8,
                        );
                        i += 4;
                        Some(c)
                    }
                    Some(5) if i + 2 < params.len() => {
                        let c = Color::Indexed(params[i + 2] as u8);
                        i += 2;
                        Some(c)
                    }
                    _ => None,
                };
                if let Some(c) = color {
                    st = if n == 38 { st.fg(c) } else { st.bg(c) };
                }
            }
            _ => {}
        }
        i += 1;
    }
    st
}

/// Convierte `text` en líneas con estilo. Las secuencias que no son de color
/// (mover el cursor, borrar…) se descartan.
pub fn parse(text: &str) -> Vec<Line<'static>> {
    let mut lines = vec![];
    for raw in text.lines() {
        let mut spans: Vec<Span<'static>> = vec![];
        let mut style = Style::default();
        let mut buf = String::new();
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\u{1b}' {
                buf.push(c);
                continue;
            }
            if chars.peek() != Some(&'[') {
                continue;
            }
            chars.next();
            let mut seq = String::new();
            let mut fin = ' ';
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    fin = n;
                    break;
                }
                seq.push(n);
            }
            if fin == 'm' {
                if !buf.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut buf), style));
                }
                let params: Vec<u32> = if seq.is_empty() {
                    vec![0]
                } else {
                    seq.split(';').map(|p| p.parse().unwrap_or(0)).collect()
                };
                style = apply_sgr(style, &params);
            }
        }
        if !buf.is_empty() {
            spans.push(Span::styled(buf, style));
        }
        lines.push(Line::from(spans));
    }
    lines
}

/// Ancho en celdas de la línea más larga.
pub fn width(lines: &[Line]) -> usize {
    lines.iter().map(Line::width).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truecolor_blocks_keep_their_colors() {
        let l = parse("\u{1b}[38;2;252;22;28m▀\u{1b}[48;2;18;18;18m▄\u{1b}[0m fin");
        assert_eq!(l.len(), 1);
        let s = &l[0].spans;
        assert_eq!(s[0].content, "▀");
        assert_eq!(s[0].style.fg, Some(Color::Rgb(252, 22, 28)));
        assert_eq!(s[1].style.bg, Some(Color::Rgb(18, 18, 18)));
        assert_eq!(s[1].style.fg, Some(Color::Rgb(252, 22, 28)));
        // «0» deja el estilo limpio.
        assert_eq!(s[2].content, " fin");
        assert_eq!(s[2].style, Style::default());
    }

    #[test]
    fn other_sequences_are_dropped_and_lines_split() {
        let l = parse("\u{1b}[?25l\u{1b}[1;32mok\u{1b}[K\u{1b}[0m\nsegunda");
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].spans[0].content, "ok");
        assert_eq!(l[0].spans[0].style.fg, Some(Color::Indexed(2)));
        assert!(l[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(l[1].spans[0].content, "segunda");
    }

    #[test]
    fn a_pixel_logo_keeps_its_shape_and_width() {
        let logo = "  \u{1b}[38;2;252;22;28m▀▄▀\u{1b}[0m\n \u{1b}[48;2;18;18;18m   \u{1b}[49m\n";
        let lines = parse(logo);
        assert_eq!(lines.len(), 2);
        assert_eq!(width(&lines), 5);
        assert!(
            lines[0]
                .spans
                .iter()
                .any(|s| s.style.fg == Some(Color::Rgb(252, 22, 28)))
        );
    }
}
