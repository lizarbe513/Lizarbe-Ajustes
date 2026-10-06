//! Primitivas de dibujo con el estilo visual de Meca: texto recortado al
//! ancho, cajas en relieve de tres líneas, ventanas con marco de acento,
//! barra de título y barra de estado con atajos que se ocultan por prioridad.

use ratatui::Frame;
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

// ---------------------------------------------------------------- relieve 3D

/// Caja en relieve de 3 líneas, como los controles de Meca:
/// `┌───┐` / `┃ x │` / `┗━━━┙`. El borde izquierdo e inferior hacen de
/// sombra (acento al estar activo).
#[allow(clippy::too_many_arguments)]
pub fn bevel_box(
    f: &mut Frame,
    x: u16,
    y: u16,
    inner: Vec<Span<'static>>,
    inner_w: usize,
    border: Color,
    shadow: Color,
    bold: bool,
) {
    let b = if bold {
        Style::new().fg(border).add_modifier(Modifier::BOLD)
    } else {
        fg(border)
    };
    let sh = Style::new().fg(shadow).add_modifier(Modifier::BOLD);
    put(
        f,
        x,
        y,
        vec![Span::styled(format!("┌{}┐", "─".repeat(inner_w)), b)],
    );
    let mut mid = vec![Span::styled("┃", sh)];
    mid.extend(inner);
    mid.push(Span::styled("│", b));
    put(f, x, y + 1, mid);
    put(
        f,
        x,
        y + 2,
        vec![Span::styled(format!("┗{}┙", "━".repeat(inner_w)), sh)],
    );
}

/// Colores de un control según esté seleccionado o bajo el ratón.
pub struct Tone {
    pub border: Color,
    pub shadow: Color,
    pub bg: Option<Color>,
    pub bold: bool,
}

pub fn tone(pal: &Palette, selected: bool, hover: bool) -> Tone {
    if hover {
        Tone {
            border: pal.accent,
            shadow: pal.accent,
            bg: Some(pal.soft_hover),
            bold: true,
        }
    } else if selected {
        Tone {
            border: pal.bright,
            shadow: pal.accent,
            bg: Some(pal.soft_selection),
            bold: true,
        }
    } else {
        Tone {
            border: pal.fg,
            shadow: pal.muted,
            bg: None,
            bold: false,
        }
    }
}

pub fn inner_style(pal: &Palette, tn: &Tone) -> Style {
    let mut s = Style::new().fg(pal.bright);
    if let Some(bg) = tn.bg {
        s = s.bg(bg);
    }
    if tn.bold {
        s = s.add_modifier(Modifier::BOLD);
    }
    s
}

/// Aspecto de un botón en relieve: (borde, sombra, estilo del texto).
pub fn button_look(
    pal: &Palette,
    enabled: bool,
    hover: bool,
    selected: bool,
    primary: bool,
) -> (Color, Color, Style) {
    if !enabled {
        (pal.dim, pal.dim, fg(pal.dim))
    } else if hover {
        (
            pal.accent,
            pal.accent,
            Style::new()
                .fg(pal.bright)
                .bg(pal.soft_hover)
                .add_modifier(Modifier::BOLD),
        )
    } else if selected {
        (
            pal.bright,
            pal.accent,
            Style::new()
                .fg(pal.bright)
                .bg(pal.soft_selection)
                .add_modifier(Modifier::BOLD),
        )
    } else if primary {
        (
            pal.bright,
            pal.accent,
            fg(pal.bright).add_modifier(Modifier::BOLD),
        )
    } else {
        (pal.fg, pal.muted, fg(pal.fg))
    }
}

/// Estilo de una fila de lista: bajo el ratón, seleccionada o normal.
pub fn row_style(pal: &Palette, hover: bool, selected: bool, enabled: bool) -> Style {
    if !enabled {
        fg(pal.dim)
    } else if hover {
        Style::new()
            .fg(pal.bright)
            .bg(pal.soft_hover)
            .add_modifier(Modifier::BOLD)
    } else if selected {
        Style::new()
            .fg(pal.bright)
            .bg(pal.soft_selection)
            .add_modifier(Modifier::BOLD)
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

/// Ventana con marco de acento y título; devuelve el área interior.
pub fn frame(f: &mut Frame, pal: &Palette, r: Rect, title: &str) -> Rect {
    f.render_widget(Clear, r);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(fg(pal.accent))
        .title(Span::styled(
            format!(" {title} "),
            fg(pal.bright).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(r);
    f.render_widget(block, r);
    inner
}

// ---------------------------------------------------------------- barras

/// Barra de título en color de acento: `left` a la izquierda (o `left_short`
/// si no cabe) y `parts` separadas por `│` a la derecha.
pub fn header_bar(
    f: &mut Frame,
    pal: &Palette,
    area: Rect,
    left: &str,
    left_short: &str,
    parts: &[String],
) {
    let style = Style::new()
        .fg(pal.bright)
        .bg(pal.accent)
        .add_modifier(Modifier::BOLD);
    let mut right = format!(" {} ", parts.join(" │ "));
    let w = area.width as usize;
    let left = if left.width() + right.width() > w {
        left_short
    } else {
        left
    };
    if left.width() + right.width() > w {
        right = String::new();
    }
    let space = w.saturating_sub(left.width() + right.width());
    let line = format!("{left}{}{right}", " ".repeat(space));
    f.render_widget(Line::from(Span::styled(truncate(&line, w), style)), area);
}

/// Atajo de la barra de estado: (tecla, descripción, prioridad). Con poco
/// ancho se ocultan primero los de prioridad más baja.
pub type Hint = (String, String, u8);

/// Barra de estado: `status` a la izquierda y los atajos que quepan a la
/// derecha.
pub fn status_bar(
    f: &mut Frame,
    pal: &Palette,
    area: Rect,
    status: &str,
    bold: bool,
    hints: &[Hint],
) {
    let base = Style::new().fg(pal.bright).bg(pal.soft_muted);
    let status_style = if bold {
        base.add_modifier(Modifier::BOLD)
    } else {
        base
    };
    let render = |h: &[&Hint]| -> String {
        if h.is_empty() {
            return String::new();
        }
        let parts: Vec<String> = h.iter().map(|(k, d, _)| format!("{k}: {d}")).collect();
        format!(" {} ", parts.join(" │ "))
    };
    let mut keep: Vec<&Hint> = hints.iter().collect();
    let width = area.width as usize;
    let budget = width.saturating_sub(status.width().min(width / 2) + 1);
    for prio in 0..4u8 {
        while render(&keep).width() > budget {
            match keep.iter().position(|h| h.2 == prio) {
                Some(i) => {
                    keep.remove(i);
                }
                None => break,
            }
        }
    }
    let right = render(&keep);
    let left_w = width.saturating_sub(right.width());
    f.render_widget(
        Line::from(vec![
            Span::styled(pad(status, left_w), status_style),
            Span::styled(right, base),
        ]),
        area,
    );
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
