//! Primitivas de dibujo con el estilo visual de Meca: texto recortado al
//! ancho, cajas en relieve de tres líneas, ventanas con marco de acento,
//! barra de título y barra de estado con atajos que se ocultan por prioridad.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear};
use unicode_width::UnicodeWidthStr;

use crate::theme::Palette;

pub fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

/// Escribe una línea de `spans` en (x, y), recortada al ancho disponible.
/// Devuelve el ancho ocupado.
pub fn put(f: &mut Frame, x: u16, y: u16, spans: Vec<Span>) -> u16 {
    let area = f.area();
    if y >= area.bottom() || x >= area.right() {
        return 0;
    }
    let w: u16 = spans.iter().map(|s| s.content.width() as u16).sum();
    let w = w.min(area.right() - x);
    f.render_widget(Line::from(spans), Rect::new(x, y, w, 1));
    w
}

pub fn pad(s: &str, w: usize) -> String {
    let cur = s.width();
    if cur >= w {
        truncate(s, w)
    } else {
        format!("{s}{}", " ".repeat(w - cur))
    }
}

pub fn truncate(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
    }
    if w == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + cw + 1 > w {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    out
}

/// Ajuste de línea sencillo por palabras.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut out = vec![];
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && line.width() + 1 + word.width() > width {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            while line.width() > width {
                let cut: String = line.chars().take(width).collect();
                line = line.chars().skip(width).collect();
                out.push(cut);
            }
        }
        out.push(line);
    }
    out
}

pub fn center(s: &str, w: usize) -> String {
    let cw = s.width();
    if cw >= w {
        return truncate(s, w);
    }
    let left = (w - cw) / 2;
    format!("{}{s}{}", " ".repeat(left), " ".repeat(w - cw - left))
}

pub fn right_align(s: &str, w: usize) -> String {
    let cw = s.width();
    if cw >= w {
        return truncate(s, w);
    }
    format!("{}{s}", " ".repeat(w - cw))
}

/// Minúsculas y sin acentos, para buscar "micro" y encontrar "Micrófono".
pub fn fold(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

// ---------------------------------------------------------------- piezas planas

/// Línea horizontal fina de `w` celdas.
pub fn rule_h(f: &mut Frame, x: u16, y: u16, w: u16, color: Color) {
    if w > 0 {
        put(
            f,
            x,
            y,
            vec![Span::styled("─".repeat(w as usize), fg(color))],
        );
    }
}

/// Línea vertical fina de `h` celdas.
pub fn rule_v(f: &mut Frame, x: u16, y: u16, h: u16, color: Color) {
    for dy in 0..h {
        put(f, x, y + dy, vec![Span::styled("│", fg(color))]);
    }
}

/// "ESCRITORIO" → "E S C R I T O R I O" (las palabras se separan con tres espacios).
pub fn spaced(s: &str) -> String {
    s.to_uppercase()
        .split_whitespace()
        .map(|w| w.chars().map(String::from).collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("   ")
}

/// Texto de una tecla para mostrar en un botón: "A", "⏎", "Esc"…
pub fn key_text(code: KeyCode) -> String {
    match code {
        KeyCode::Enter => "⏎".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::Delete => "Supr".into(),
        KeyCode::Char(' ') => "Espacio".into(),
        KeyCode::Char(c) => c.to_uppercase().to_string(),
        other => format!("{other:?}"),
    }
}

/// Ancho de un botón de texto `[K] etiqueta`.
pub fn button_width(key: &str, label: &str) -> usize {
    key.width() + 3 + label.width()
}

/// Botón de texto `[K] etiqueta`: la tecla en acento, la etiqueta en el tono
/// normal; el principal en negrita y el que tiene el ratón o el foco subrayado.
pub fn button_spans(
    pal: &Palette,
    key: &str,
    label: &str,
    enabled: bool,
    primary: bool,
    lit: bool,
) -> Vec<Span<'static>> {
    if !enabled {
        return vec![Span::styled(format!("[{key}] {label}"), fg(pal.dim))];
    }
    let mut text = if primary || lit {
        fg(pal.bright).add_modifier(Modifier::BOLD)
    } else {
        fg(pal.fg)
    };
    if lit {
        text = text.add_modifier(Modifier::UNDERLINED);
    }
    vec![
        Span::styled("[", fg(pal.muted)),
        Span::styled(key.to_string(), fg(pal.accent).add_modifier(Modifier::BOLD)),
        Span::styled("] ", fg(pal.muted)),
        Span::styled(label.to_string(), text),
    ]
}

/// Fila de una lista: marca `▍` en acento si es la elegida; texto en negrita
/// si está elegida o bajo el ratón. `width` incluye la marca.
pub fn list_row(
    pal: &Palette,
    text: &str,
    width: usize,
    selected: bool,
    hover: bool,
    enabled: bool,
) -> Vec<Span<'static>> {
    let style = row_style(pal, hover, selected, enabled);
    vec![
        Span::styled(
            if selected && enabled { "▍" } else { " " },
            fg(pal.accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(pad(text, width.saturating_sub(1)), style),
    ]
}

/// Estilo de una fila de lista: sin fondos; el tono y el subrayado marcan el estado.
pub fn row_style(pal: &Palette, hover: bool, selected: bool, enabled: bool) -> Style {
    if !enabled {
        fg(pal.dim)
    } else if hover {
        Style::new()
            .fg(pal.bright)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else if selected {
        Style::new().fg(pal.bright).add_modifier(Modifier::BOLD)
    } else {
        fg(pal.fg)
    }
}

// ---------------------------------------------------------------- ventanas

/// Rectángulo de `w`×`h` centrado en `area`, con margen de una celda.
pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// Ventana con marco fino redondeado y título en mayúsculas; devuelve el área interior.
pub fn frame(f: &mut Frame, pal: &Palette, r: Rect, title: &str) -> Rect {
    f.render_widget(Clear, r);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(fg(pal.rule))
        .title(Span::styled(
            format!(" {} ", title.to_uppercase()),
            fg(pal.bright).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(r);
    f.render_widget(block, r);
    inner
}

// ---------------------------------------------------------------- barras

/// Atajo de la barra de estado: (tecla, descripción, prioridad). Con poco
/// ancho se ocultan primero los de prioridad más baja.
pub type Hint = (String, String, u8);

/// Barra de estado sin relleno: `status` a la izquierda y los atajos que
/// quepan a la derecha (`tecla descripción · tecla descripción`).
pub fn status_bar(
    f: &mut Frame,
    pal: &Palette,
    area: Rect,
    status: &str,
    bold: bool,
    hints: &[Hint],
) {
    let render = |h: &[&Hint]| -> usize {
        h.iter()
            .map(|(k, d, _)| k.width() + 1 + d.width())
            .sum::<usize>()
            + 5 * h.len().saturating_sub(1)
    };
    let mut keep: Vec<&Hint> = hints.iter().collect();
    let width = area.width as usize;
    let status_w = status.width().min(width / 2);
    let budget = width.saturating_sub(status_w + 6);
    for prio in 0..4u8 {
        while render(&keep) > budget {
            match keep.iter().position(|h| h.2 == prio) {
                Some(i) => {
                    keep.remove(i);
                }
                None => break,
            }
        }
    }
    let status_style = if bold {
        fg(pal.bright).add_modifier(Modifier::BOLD)
    } else {
        fg(pal.muted)
    };
    put(
        f,
        area.x + 2,
        area.y,
        vec![Span::styled(truncate(status, status_w), status_style)],
    );
    let mut spans: Vec<Span> = vec![];
    for (n, (k, d, _)) in keep.iter().enumerate() {
        if n > 0 {
            spans.push(Span::styled("  ·  ", fg(pal.rule)));
        }
        spans.push(Span::styled(k.clone(), fg(pal.fg)));
        spans.push(Span::styled(format!(" {d}"), fg(pal.muted)));
    }
    let w = render(&keep) as u16;
    if w > 0 {
        put(f, area.right().saturating_sub(w + 2), area.y, spans);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_helpers_respect_width() {
        assert_eq!(pad("ab", 4), "ab  ");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(center("ab", 6), "  ab  ");
        assert_eq!(right_align("ab", 4), "  ab");
        assert_eq!(
            wrap("uno dos tres cuatro", 9),
            vec!["uno dos", "tres", "cuatro"]
        );
    }

    #[test]
    fn folds_accents() {
        assert_eq!(fold("Micrófono Año"), "microfono ano");
    }
}
