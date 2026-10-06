//! Dibujo de la interfaz.

pub mod form;
pub mod popup;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use serde_json::Value;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Button, Focus, Hit, PluginRow, Section};
use crate::i18n::{self, t, tf};
use crate::omarchy::curated;
use crate::omarchy::schema::{Kind, value_label};
use crate::omarchy::shell_json as sj;
use crate::omarchy::theme::hex;
use form::{FieldRow, NoteKind, Row};
use popup::Popup;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = f.area();
    let [header, body, status, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, app, header);
    let side_w = if body.width < 100 { 19 } else { 24 };
    let [side, content] =
        Layout::horizontal([Constraint::Length(side_w), Constraint::Min(20)]).areas(body);
    draw_sidebar(f, app, side);
    draw_content(f, app, content);
    draw_status(f, app, status);
    draw_footer(f, app, footer);

    if app.popup.is_some() {
        draw_popup(f, app, area);
    }
}

fn panel<'a>(app: &App, title: String, focused: bool) -> Block<'a> {
    let color = if focused { app.pal.accent } else { app.pal.dim };
    Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::new().fg(color))
        .title(Span::styled(
            format!(" {title} "),
            Style::new()
                .fg(if focused {
                    app.pal.accent
                } else {
                    app.pal.muted
                })
                .bold(),
        ))
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let mut left = vec![
        Span::styled("  Quickshell", Style::new().fg(app.pal.accent).bold()),
        Span::styled(
            format!("  {}", t("app.subtitle")),
            Style::new().fg(app.pal.muted),
        ),
    ];
    if app.store.paths.sandbox {
        left.push(Span::styled(
            "  [sandbox]".to_string(),
            Style::new().fg(app.pal.warn),
        ));
    }
    let mut right = vec![];
    if app.store.dirty() {
        let n = app.store.changes().len() + app.store.ops.len();
        right.push(Span::styled(
            format!("● {} ", tf("hdr.pending", &[("n", &n.to_string())])),
            Style::new().fg(app.pal.warn).bold(),
        ));
    }
    right.push(Span::styled(
        format!(
            " {} ",
            if app.advanced {
                t("hdr.advanced")
            } else {
                t("hdr.simple")
            }
        ),
        Style::new().fg(app.pal.accent),
    ));
    right.push(Span::styled(
        format!(" {} ", i18n::lang().code().to_uppercase()),
        Style::new().fg(app.pal.muted),
    ));
    if !app.store.paths.sandbox && !app.shell_running {
        right.push(Span::styled(
            format!(" {} ", t("hdr.shell_off")),
            Style::new().fg(app.pal.warn),
        ));
    }
    let rw: u16 = right.iter().map(|s| s.content.width() as u16).sum();
    let lw: u16 = left.iter().map(|s| s.content.width() as u16).sum();
    if lw + rw > area.width {
        // Sin espacio: fuera el subtítulo.
        left.remove(1);
    }
    let [l, r] = Layout::horizontal([Constraint::Min(1), Constraint::Length(rw)]).areas(area);
    f.render_widget(Line::from(left), l);
    f.render_widget(Line::from(right), r);
}

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Sidebar && app.popup.is_none();
    let block = panel(app, t("side.title"), focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    for (i, s) in Section::ALL.iter().enumerate() {
        let y = inner.y + i as u16 * 2;
        if y >= inner.bottom() {
            break;
        }
        let r = Rect::new(inner.x, y, inner.width, 1);
        let selected = *s == app.section;
        let mut label = format!(" {}  {}", s.icon(), s.title());
        if *s == Section::Changes && app.store.dirty() {
            label.push_str(&format!(
                " ({})",
                app.store.changes().len() + app.store.ops.len()
            ));
        }
        let style = if selected {
            Style::new()
                .fg(app.pal.accent)
                .bg(if focused {
                    app.pal.selection
                } else {
                    Color::Reset
                })
                .bold()
        } else {
            Style::new().fg(app.pal.fg)
        };
        let marker = if selected { "▌" } else { " " };
        f.render_widget(
            Line::from(vec![
                Span::styled(marker, Style::new().fg(app.pal.accent)),
                Span::styled(pad(&label, inner.width.saturating_sub(1) as usize), style),
            ]),
            r,
        );
        app.hits.push((r, Hit::Sidebar(i)));
    }
    // Número de sección como atajo.
    let hint = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
    if inner.height > 13 {
        f.render_widget(
            Line::from(Span::styled(
                " 1-6 · Tab".to_string(),
                Style::new().fg(app.pal.dim),
            )),
            hint,
        );
    }
}

fn pad(s: &str, w: usize) -> String {
    let cur = s.width();
    if cur >= w {
        truncate(s, w)
    } else {
        format!("{s}{}", " ".repeat(w - cur))
    }
}

fn truncate(s: &str, w: usize) -> String {
    if s.width() <= w {
        return s.to_string();
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
fn wrap(text: &str, width: usize) -> Vec<String> {
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

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Content && app.popup.is_none();
    let mut title = app.section.title();
    if app.section == Section::Widgets
        && let Some((s, i)) = app.widgets.editing
    {
        let id = sj::section(&app.store.json, s)
            .get(i)
            .map(sj::entry_id)
            .unwrap_or_default();
        title = format!(
            "{title} › {}",
            curated::widget_name(
                &id,
                sj::section(&app.store.json, s).get(i),
                &app.store.catalog
            )
        );
    }
    let block = panel(app, title, focused || app.focus == Focus::Buttons);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [main, buttons] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).areas(inner);
    match app.section {
        Section::Widgets if app.widgets.editing.is_none() => draw_layout(f, app, main, focused),
        Section::Plugins => draw_plugins(f, app, main, focused),
        _ => draw_form(f, app, main, focused),
    }
    draw_buttons(f, app, buttons);
}

/// Fila de botones: Aplicar, Cancelar y Restaurar, clicables con el ratón.
fn draw_buttons(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Buttons && app.popup.is_none();
    let pending = app.store.changes().len() + app.store.ops.len();
    // Con poco ancho se quita la tecla de atajo de la etiqueta.
    let full: Vec<String> = Button::ALL.iter().map(|b| b.label()).collect();
    let short: Vec<String> = full
        .iter()
        .map(|l| {
            l.rsplit_once("  ")
                .map(|(a, _)| a.to_string())
                .unwrap_or(l.clone())
        })
        .collect();
    // Ancho de los botones + 1 de separación entre ellos + 1 de margen derecho.
    let width =
        |ls: &[String]| ls.iter().map(|l| l.width() as u16 + 4).sum::<u16>() + ls.len() as u16;
    let labels = if width(&full) + 12 <= area.width {
        full
    } else {
        short
    };
    let total = width(&labels);

    // Texto de estado a la izquierda.
    let (status, color) = if focused {
        let b = Button::ALL[app.button];
        let text = match b {
            Button::Apply => tf("btn.apply.desc", &[("n", &pending.to_string())]),
            Button::Cancel => t("btn.cancel.desc"),
            Button::Restore => t(app.restore_scope().1),
        };
        let color = if app.button_enabled(b) {
            app.pal.fg
        } else {
            app.pal.dim
        };
        (text, color)
    } else if pending > 0 {
        (
            format!("● {}", tf("btn.pending", &[("n", &pending.to_string())])),
            app.pal.warn,
        )
    } else {
        (format!("✓ {}", t("btn.clean")), app.pal.muted)
    };
    let status_w = area.width.saturating_sub(total);
    if status_w > 4 {
        let r = Rect::new(area.x, area.y + 1, status_w - 1, 1);
        f.render_widget(
            Line::from(Span::styled(
                truncate(&format!(" {status}"), status_w as usize),
                Style::new().fg(color),
            )),
            r,
        );
    }

    let mut x = area.right().saturating_sub(total);
    for (i, (b, label)) in Button::ALL.iter().zip(labels).enumerate() {
        let w = label.width() as u16 + 4;
        if x + w > area.right() {
            break;
        }
        let r = Rect::new(x, area.y, w, 3);
        let enabled = app.button_enabled(*b);
        let selected = focused && app.button == i;
        let (border, text) = match (enabled, selected) {
            (false, _) => (app.pal.dim, app.pal.dim),
            (true, true) => (app.pal.accent, app.pal.accent),
            (true, false) if *b == Button::Apply => (app.pal.accent, app.pal.fg),
            (true, false) => (app.pal.muted, app.pal.fg),
        };
        let mut style = Style::new().fg(text);
        if selected {
            style = style.bg(app.pal.selection).add_modifier(Modifier::BOLD);
        }
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(if selected {
                BorderType::Thick
            } else {
                BorderType::Rounded
            })
            .border_style(Style::new().fg(border));
        let inner = block.inner(r);
        f.render_widget(block, r);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(format!(" {label} "), style)))
                .alignment(ratatui::layout::HorizontalAlignment::Center),
            inner,
        );
        app.hits.push((r, Hit::Button(i)));
        x += w + 1;
    }
}

// ---------------------------------------------------------------- formularios

fn row_height(row: &Row, width: usize) -> usize {
    match row {
        Row::Note(text, _) => wrap(text, width.saturating_sub(4)).len(),
        Row::Header(_) => 2,
        _ => 1,
    }
}

fn draw_form(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let rows = app.rows();
    let st = if app.section == Section::Widgets {
        &mut app.widgets.form
    } else {
        &mut app.forms[app.section.index()]
    };
    st.clamp(&rows);
    let sel = st.sel;

    // Zona de descripción del campo seleccionado.
    let desc_lines = match rows.get(sel) {
        Some(Row::Field(fr)) => field_help(fr, app.advanced),
        _ => vec![],
    };
    let desc_wrapped: Vec<String> = desc_lines
        .iter()
        .flat_map(|l| wrap(l, area.width.saturating_sub(2) as usize))
        .collect();
    let desc_h = if desc_wrapped.is_empty() {
        0
    } else {
        (desc_wrapped.len() as u16 + 1).min(area.height / 3).max(2)
    };
    let [list, desc] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(desc_h)]).areas(area);

    let width = list.width as usize;
    let heights: Vec<usize> = rows.iter().map(|r| row_height(r, width)).collect();
    // Desplazamiento para que la selección sea visible.
    let st = if app.section == Section::Widgets {
        &mut app.widgets.form
    } else {
        &mut app.forms[app.section.index()]
    };
    if st.offset > sel {
        st.offset = sel;
    }
    while st.offset < sel && heights[st.offset..=sel].iter().sum::<usize>() > list.height as usize {
        st.offset += 1;
    }
    // Si hay cabecera justo encima, muéstrala también.
    if st.offset > 0 && st.offset == sel && matches!(rows.get(sel - 1), Some(Row::Header(_))) {
        let h: usize = heights[sel - 1..=sel].iter().sum();
        if h <= list.height as usize {
            st.offset -= 1;
        }
    }
    let offset = st.offset;

    let label_w = rows
        .iter()
        .filter_map(|r| match r {
            Row::Field(fr) => Some(fr.def.label.width()),
            _ => None,
        })
        .max()
        .unwrap_or(10)
        .clamp(10, (width / 2).max(10))
        + 4;

    let mut y = list.y;
    for (i, row) in rows.iter().enumerate().skip(offset) {
        let h = heights[i] as u16;
        if y + h > list.bottom() {
            break;
        }
        let r = Rect::new(list.x, y, list.width, h);
        let selected = i == sel && row.selectable();
        match row {
            Row::Header(text) => {
                let line = Rect::new(r.x, r.y + 1, r.width, 1);
                let label = format!(" {text} ");
                let rest = (r.width as usize).saturating_sub(label.width() + 1);
                f.render_widget(
                    Line::from(vec![
                        Span::styled("─", Style::new().fg(app.pal.dim)),
                        Span::styled(label, Style::new().fg(app.pal.accent).bold()),
                        Span::styled("─".repeat(rest), Style::new().fg(app.pal.dim)),
                    ]),
                    line,
                );
            }
            Row::Note(text, kind) => {
                let (icon, color) = match kind {
                    NoteKind::Info => ("", app.pal.muted),
                    NoteKind::Warn => ("", app.pal.warn),
                };
                let lines: Vec<Line> = wrap(text, width.saturating_sub(4))
                    .into_iter()
                    .enumerate()
                    .map(|(n, l)| {
                        Line::from(vec![
                            Span::styled(
                                if n == 0 {
                                    format!(" {icon} ")
                                } else {
                                    "   ".into()
                                },
                                Style::new().fg(color),
                            ),
                            Span::styled(l, Style::new().fg(color)),
                        ])
                    })
                    .collect();
                f.render_widget(Paragraph::new(lines), r);
            }
            Row::Field(fr) => draw_field(f, app, r, fr, selected && focused, selected, label_w),
            Row::Action(label, _) => {
                let style = if selected && focused {
                    Style::new().fg(app.pal.accent).bg(app.pal.selection).bold()
                } else {
                    Style::new().fg(app.pal.accent)
                };
                f.render_widget(
                    Line::from(vec![
                        Span::styled(
                            if selected { "▌" } else { " " },
                            Style::new().fg(app.pal.accent),
                        ),
                        Span::styled(pad(&format!(" ▸ {label}"), width.saturating_sub(1)), style),
                    ]),
                    r,
                );
            }
        }
        if row.selectable() {
            app.hits.push((r, Hit::Row(i)));
        }
        y += h;
    }

    if desc_h > 0 {
        let block = Block::new()
            .borders(Borders::TOP)
            .border_style(Style::new().fg(app.pal.dim));
        let inner = block.inner(desc);
        f.render_widget(block, desc);
        let lines: Vec<Line> = desc_wrapped
            .into_iter()
            .map(|l| {
                Line::from(Span::styled(
                    format!(" {l}"),
                    Style::new().fg(app.pal.muted),
                ))
            })
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }
}

fn field_help(fr: &FieldRow, advanced: bool) -> Vec<String> {
    let mut out = vec![];
    if !fr.def.desc.is_empty() {
        out.push(fr.def.desc.clone());
    }
    if fr.def.date_format
        && let Some(Value::String(s)) = fr.effective()
        && s.contains('\n')
    {
        let now = chrono::Local::now().naive_local();
        let prev =
            crate::omarchy::qt_format::format(s, &now, crate::omarchy::qt_format::time_lang());
        out.push(format!(
            "{}: {}",
            t("help.preview"),
            prev.replace('\n', " / ")
        ));
    }
    let mut keys = vec![];
    match &fr.def.kind {
        Kind::Bool => keys.push(t("help.k.toggle")),
        Kind::Enum(_) => keys.push(t("help.k.enum")),
        Kind::Multi(_) => keys.push(t("help.k.multi")),
        Kind::Int { .. } | Kind::Float { .. } if fr.def.presets.is_empty() => {
            keys.push(t("help.k.number"))
        }
        _ if !fr.def.presets.is_empty() => keys.push(t("help.k.presets")),
        _ => keys.push(t("help.k.edit")),
    }
    keys.push(t("help.k.reset"));
    let mut tail = keys.join(" · ");
    if advanced {
        let default = fr
            .def
            .default
            .as_ref()
            .map(|d| match d {
                Value::String(s) => format!("\"{}\"", crate::omarchy::schema::escape_newlines(s)),
                other => value_label(other),
            })
            .unwrap_or_else(|| "—".into());
        tail = format!(
            "{tail}   [{}: {} · {}: {default}]",
            t("help.key"),
            match &fr.bind {
                crate::store::Bind::Json(p) => sj::path_to_string(p),
                crate::store::Bind::Toml(s, k) => format!("{s}.{k}"),
            },
            t("help.default")
        );
    }
    out.push(tail);
    out
}

fn draw_field(
    f: &mut Frame,
    app: &App,
    r: Rect,
    fr: &FieldRow,
    focused_sel: bool,
    selected: bool,
    label_w: usize,
) {
    let width = r.width as usize;
    let label = pad(&format!("  {}", fr.def.label), label_w);
    let is_default = form::is_default(fr);
    let mut value = form::display_value(fr);
    let adjustable = matches!(
        fr.def.kind,
        Kind::Bool | Kind::Enum(_) | Kind::Int { .. } | Kind::Float { .. }
    ) || !fr.def.presets.is_empty();
    if focused_sel && adjustable {
        value = format!("‹ {value} ›");
    }
    let mut spans = vec![Span::styled(
        if selected { "▌" } else { " " },
        Style::new().fg(app.pal.accent),
    )];
    let bg = if focused_sel {
        app.pal.selection
    } else {
        Color::Reset
    };
    spans.push(Span::styled(
        label,
        Style::new()
            .fg(if focused_sel {
                app.pal.accent
            } else {
                app.pal.fg
            })
            .bg(bg),
    ));
    // Muestra de color.
    if fr.def.kind == Kind::Color
        && let Some(c) = fr.effective().and_then(Value::as_str).and_then(hex)
    {
        spans.push(Span::styled("██ ", Style::new().fg(c).bg(bg)));
    }
    let vstyle = if is_default {
        Style::new().fg(app.pal.muted).bg(bg)
    } else {
        Style::new().fg(app.pal.fg).bg(bg).bold()
    };
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    let suffix = if is_default && fr.def.default.is_some() {
        format!("  {}", t("val.default_tag"))
    } else {
        String::new()
    };
    let avail = width.saturating_sub(used + suffix.width());
    spans.push(Span::styled(pad(&value, avail), vstyle));
    spans.push(Span::styled(suffix, Style::new().fg(app.pal.dim).bg(bg)));
    f.render_widget(Line::from(spans), r);
}

// ---------------------------------------------------------------- layout

fn widget_label(app: &App, entry: &Value) -> String {
    let id = sj::entry_id(entry);
    let name = curated::widget_name(&id, Some(entry), &app.store.catalog);
    match entry.get("type").and_then(Value::as_str) {
        Some("command") => format!("󰆍 {name}"),
        Some("qml") => format!("󰅩 {name}"),
        _ => name,
    }
}

fn draw_layout(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let [preview, cols, detail] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(4),
        Constraint::Length(5),
    ])
    .areas(area);

    // Vista previa de la barra en una línea.
    let names = |s: usize| -> Vec<String> {
        sj::section(&app.store.json, s)
            .iter()
            .map(|e| widget_label(app, e))
            .collect()
    };
    let (l, c, r) = (
        names(0).join(" · "),
        names(1).join(" · "),
        names(2).join(" · "),
    );
    let pos = sj::get(&app.store.json, &[sj::key("bar"), sj::key("position")])
        .and_then(Value::as_str)
        .unwrap_or("top")
        .to_string();
    let w = preview.width.saturating_sub(4) as usize;
    let third = w / 3;
    let bar_line = format!(
        "{}{}{}",
        pad(&truncate(&l, third), third),
        pad(&center(&truncate(&c, third), third), third),
        right_align(&truncate(&r, w - 2 * third), w - 2 * third)
    );
    let pv = Paragraph::new(vec![
        Line::from(Span::styled(
            format!(" {} ({})", t("lay.preview"), t(&format!("pos.{pos}"))),
            Style::new().fg(app.pal.muted),
        )),
        Line::from(Span::styled(
            format!(" ▕{bar_line}▏"),
            Style::new().fg(app.pal.fg).bg(app.pal.surface),
        )),
    ]);
    f.render_widget(pv, preview);

    let areas = Layout::horizontal([Constraint::Ratio(1, 3); 3]).split(cols);
    for s in 0..3 {
        let a = areas[s];
        let active = focused && app.widgets.col == s;
        let entries = sj::section(&app.store.json, s).to_vec();
        let drop_target =
            app.widgets.drag.is_some() && app.widgets.drag_over == Some(Hit::Column(s));
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(if active || drop_target {
                BorderType::Thick
            } else {
                BorderType::Plain
            })
            .border_style(Style::new().fg(if active || drop_target {
                app.pal.accent
            } else {
                app.pal.dim
            }))
            .title(Span::styled(
                format!(
                    " {} ({}) ",
                    t(&format!("col.{}", sj::SECTIONS[s])),
                    entries.len()
                ),
                Style::new()
                    .fg(if active {
                        app.pal.accent
                    } else {
                        app.pal.muted
                    })
                    .bold(),
            ));
        let inner = block.inner(a);
        f.render_widget(block, a);
        app.hits.push((a, Hit::Column(s)));
        if entries.is_empty() {
            f.render_widget(
                Line::from(Span::styled(
                    format!(" {}", t("lay.empty")),
                    Style::new().fg(app.pal.dim),
                )),
                inner,
            );
            continue;
        }
        let sel = app.widgets.row[s].min(entries.len() - 1);
        let visible = inner.height as usize;
        let offset = if visible == 0 {
            0
        } else {
            sel.saturating_sub(visible.saturating_sub(1))
        };
        for (i, e) in entries.iter().enumerate().skip(offset).take(visible) {
            let y = inner.y + (i - offset) as u16;
            let r = Rect::new(inner.x, y, inner.width, 1);
            let is_sel = i == sel && app.widgets.col == s;
            let dragging = app.widgets.drag == Some((s, i));
            let over =
                app.widgets.drag.is_some() && app.widgets.drag_over == Some(Hit::Widget(s, i));
            let mut text = format!(" {}", widget_label(app, e));
            if app.advanced {
                let id = sj::entry_id(e);
                if !text.contains(&id) {
                    text = format!("{text}  {id}");
                }
            }
            let style = if over {
                Style::new()
                    .fg(app.pal.accent)
                    .add_modifier(Modifier::UNDERLINED)
            } else if is_sel && focused {
                Style::new().fg(app.pal.accent).bg(app.pal.selection).bold()
            } else if dragging {
                Style::new().fg(app.pal.muted)
            } else {
                Style::new().fg(app.pal.fg)
            };
            let marker = if is_sel { "▌" } else { " " };
            f.render_widget(
                Line::from(vec![
                    Span::styled(marker, Style::new().fg(app.pal.accent)),
                    Span::styled(pad(&text, inner.width.saturating_sub(1) as usize), style),
                ]),
                r,
            );
            app.hits.push((r, Hit::Widget(s, i)));
        }
    }

    // Detalle del widget seleccionado.
    let block = Block::new()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(app.pal.dim));
    let inner = block.inner(detail);
    f.render_widget(block, detail);
    let col = app.widgets.col;
    let entries = sj::section(&app.store.json, col);
    let mut lines = vec![];
    if let Some(e) = entries.get(app.widgets.row[col].min(entries.len().saturating_sub(1))) {
        let id = sj::entry_id(e);
        let desc = app
            .store
            .catalog
            .get(&id)
            .map(curated::widget_description)
            .unwrap_or_else(|| match e.get("type").and_then(Value::as_str) {
                Some("command") => t("lay.custom_command"),
                Some("qml") => t("lay.custom_qml"),
                _ => t("w.unknown"),
            });
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {}", widget_label(app, e)),
                Style::new().fg(app.pal.accent).bold(),
            ),
            Span::styled(format!("  {desc}"), Style::new().fg(app.pal.muted)),
        ]));
        let settings: Vec<String> = sj::entry_settings(e)
            .iter()
            .map(|(k, v)| {
                format!(
                    "{k}={}",
                    match v {
                        Value::String(s) => crate::omarchy::schema::escape_newlines(s),
                        other => other.to_string(),
                    }
                )
            })
            .collect();
        if !settings.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(
                    " {}",
                    truncate(&settings.join("  "), inner.width.saturating_sub(2) as usize)
                ),
                Style::new().fg(app.pal.muted),
            )));
        }
    }
    lines.push(Line::from(Span::styled(
        format!(" {}", t("lay.tip")),
        Style::new().fg(app.pal.dim),
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn center(s: &str, w: usize) -> String {
    let cw = s.width();
    if cw >= w {
        return s.to_string();
    }
    format!("{}{s}", " ".repeat((w - cw) / 2))
}

fn right_align(s: &str, w: usize) -> String {
    let cw = s.width();
    if cw >= w {
        return s.to_string();
    }
    format!("{}{s}", " ".repeat(w - cw))
}

// ---------------------------------------------------------------- plugins

fn draw_plugins(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    app.clamp_plugin_sel(true);
    let rows = app.plugin_rows();
    let [filter, list, detail] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(6),
    ])
    .areas(area);

    let ftext = if app.plugins.filtering || !app.plugins.filter.is_empty() {
        format!(
            " / {}{}",
            app.plugins.filter,
            if app.plugins.filtering { "▏" } else { "" }
        )
    } else {
        format!(" {}", t("pl.filter_hint"))
    };
    f.render_widget(
        Line::from(Span::styled(ftext, Style::new().fg(app.pal.muted))),
        filter,
    );

    let sel = app.plugins.sel.min(rows.len().saturating_sub(1));
    let h = list.height as usize;
    if app.plugins.offset > sel {
        app.plugins.offset = sel.saturating_sub(1);
    }
    if sel >= app.plugins.offset + h {
        app.plugins.offset = sel + 1 - h;
    }
    let offset = app.plugins.offset;
    for (i, row) in rows.iter().enumerate().skip(offset).take(h) {
        let r = Rect::new(list.x, list.y + (i - offset) as u16, list.width, 1);
        match row {
            PluginRow::Header(title) => {
                let rest = (r.width as usize).saturating_sub(title.width() + 3);
                f.render_widget(
                    Line::from(vec![
                        Span::styled("─ ", Style::new().fg(app.pal.dim)),
                        Span::styled(title.clone(), Style::new().fg(app.pal.accent).bold()),
                        Span::styled(
                            format!(" {}", "─".repeat(rest)),
                            Style::new().fg(app.pal.dim),
                        ),
                    ]),
                    r,
                );
            }
            PluginRow::Item(id) => {
                let Some(p) = app.store.catalog.get(id) else {
                    continue;
                };
                let on = app.store.plugin_enabled(p);
                let changed =
                    on != p.enabled_in(app.store.json_original()) || app.store.queued(id).is_some();
                let is_sel = i == sel;
                let name = curated::widget_name(id, None, &app.store.catalog);
                let origin = if !p.cloned_from.is_empty() {
                    tf("pl.origin.clone", &[("id", &p.cloned_from)])
                } else if p.first_party {
                    "Omarchy".to_string()
                } else if p.is_git {
                    t("pl.origin.git")
                } else {
                    t("pl.origin.user")
                };
                let state = if p.is_bar_widget() {
                    if on {
                        t("pl.state.in_bar")
                    } else {
                        t("pl.state.not_in_bar")
                    }
                } else if p.is_bar_option() {
                    if on {
                        t("pl.state.in_use")
                    } else {
                        t("pl.state.available")
                    }
                } else if on {
                    t("pl.state.on")
                } else {
                    t("pl.state.off")
                };
                let bg = if is_sel && focused {
                    app.pal.selection
                } else {
                    Color::Reset
                };
                let mut spans = vec![
                    Span::styled(
                        if is_sel { "▌" } else { " " },
                        Style::new().fg(app.pal.accent),
                    ),
                    Span::styled(
                        if on { " ● " } else { " ○ " },
                        Style::new()
                            .fg(if on { app.pal.accent } else { app.pal.dim })
                            .bg(bg),
                    ),
                ];
                let mut label = name;
                if app.advanced {
                    label = format!("{label}  {id}");
                }
                if changed {
                    label.push_str(" *");
                }
                let right = format!("{state} · {origin} ");
                let w = (r.width as usize).saturating_sub(4 + right.width());
                spans.push(Span::styled(
                    pad(&label, w),
                    Style::new()
                        .fg(if is_sel && focused {
                            app.pal.accent
                        } else {
                            app.pal.fg
                        })
                        .bg(bg),
                ));
                spans.push(Span::styled(right, Style::new().fg(app.pal.muted).bg(bg)));
                f.render_widget(Line::from(spans), r);
                app.hits.push((r, Hit::Plugin(i)));
            }
        }
    }

    let block = Block::new()
        .borders(Borders::TOP)
        .border_style(Style::new().fg(app.pal.dim));
    let inner = block.inner(detail);
    f.render_widget(block, detail);
    let mut lines = vec![];
    if let Some(p) = app.selected_plugin() {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {}", curated::widget_name(&p.id, None, &app.store.catalog)),
                Style::new().fg(app.pal.accent).bold(),
            ),
            Span::styled(
                if app.advanced {
                    format!(
                        "  {} · {}",
                        p.kinds.join(", "),
                        if p.version.is_empty() {
                            "-".into()
                        } else {
                            format!("v{}", p.version)
                        }
                    )
                } else {
                    String::new()
                },
                Style::new().fg(app.pal.muted),
            ),
        ]));
        let desc = curated::widget_description(&p);
        if !desc.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(" {desc}"),
                Style::new().fg(app.pal.fg),
            )));
        }
        if app.advanced {
            lines.push(Line::from(Span::styled(
                format!(
                    " {}  ·  {}{}",
                    p.id,
                    p.source_dir.display(),
                    if p.author.is_empty() {
                        String::new()
                    } else {
                        format!("  ·  {}", p.author)
                    }
                ),
                Style::new().fg(app.pal.dim),
            )));
        }
        if !p.first_party {
            lines.push(Line::from(Span::styled(
                format!(" {}", t("pl.warn_unsandboxed")),
                Style::new().fg(app.pal.warn),
            )));
        }
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

// ---------------------------------------------------------------- estado/pie

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    if let Some(toast) = &app.toast {
        let color = match toast.kind {
            NoteKind::Info => app.pal.accent,
            NoteKind::Warn => app.pal.warn,
        };
        f.render_widget(
            Line::from(Span::styled(
                format!(" {}", toast.text),
                Style::new().fg(color),
            )),
            area,
        );
    }
}

/// Atajo del pie: (tecla, descripción, prioridad). Con poco ancho se ocultan
/// primero los de prioridad más baja.
type Hint = (String, String, u8);

fn footer_hints(app: &App) -> Vec<Hint> {
    let k = |a: &str, b: &str, p: u8| (a.to_string(), t(b), p);
    if let Some(p) = &app.popup {
        return match p {
            Popup::Confirm { .. } => vec![k("Enter", "ft.yes", 3), k("Esc", "ft.no", 3)],
            Popup::Message { .. } => vec![k("Enter/Esc", "ft.close", 3)],
            Popup::Help { .. } => vec![k("↑↓", "ft.scroll", 2), k("Enter/Esc", "ft.close", 3)],
            Popup::Conflict { .. } => vec![
                k("o", "ft.overwrite", 3),
                k("r", "ft.reload", 3),
                k("Esc", "ft.cancel", 3),
            ],
            Popup::Input(_) => vec![
                k("Enter", "ft.save", 3),
                k("Esc", "ft.cancel", 3),
                k("Ctrl+U", "ft.clear", 1),
            ],
            Popup::Picker(_) => vec![
                k("↑↓", "ft.move", 1),
                k("Enter", "ft.choose", 3),
                k("abc", "ft.filter", 2),
                k("Esc", "ft.cancel", 3),
            ],
            Popup::Checklist(_) => vec![
                k(&t("key.space"), "ft.check", 3),
                k("a", "ft.all", 1),
                k("Enter", "ft.save", 3),
                k("Esc", "ft.cancel", 3),
            ],
        };
    }
    let mut v = vec![];
    match (app.focus, app.section) {
        (Focus::Sidebar, _) => {
            v.push(k("↑↓", "ft.section", 1));
            v.push(k("Enter", "ft.open", 2));
        }
        (Focus::Buttons, _) => {
            v.push(k("←→", "ft.choose_button", 2));
            v.push(k("Enter", "ft.press", 3));
            v.push(k("↑", "ft.back", 1));
        }
        (Focus::Content, Section::Widgets) if app.widgets.editing.is_none() => {
            v.push(k("←→↑↓", "ft.move", 0));
            v.push(k("⇧+←→↑↓", "ft.reorder", 1));
            v.push(k("Enter", "ft.settings", 2));
            v.push(k("n", "ft.add", 2));
            v.push(k("d", "ft.remove", 2));
        }
        (Focus::Content, Section::Plugins) => {
            v.push(k("Enter", "ft.toggle", 2));
            v.push(k("/", "ft.filter", 1));
            v.push(k("n", "ft.add_git", 0));
            v.push(k("p", "ft.clone", 0));
            v.push(k("u", "ft.update", 0));
            v.push(k("x", "ft.remove", 0));
        }
        _ => {
            v.push(k("↑↓", "ft.move", 0));
            v.push(k("←→", "ft.change", 2));
            v.push(k("Enter", "ft.edit", 2));
            v.push(k("r", "ft.reset", 1));
            v.push(k("Esc", "ft.back", 0));
        }
    }
    v.push(k("Tab", "ft.next_area", 1));
    v.push(k(
        "m",
        if app.advanced {
            "ft.simple"
        } else {
            "ft.advanced"
        },
        1,
    ));
    v.push(k("?", "ft.help", 3));
    v.push(k("q", "ft.quit", 3));
    v
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let hints = footer_hints(app);
    let width_of = |h: &Hint| h.0.width() + h.1.width() + 4;
    let mut keep = vec![true; hints.len()];
    let mut total: usize = 1 + hints.iter().map(width_of).sum::<usize>();
    for prio in 0..3u8 {
        for (i, h) in hints.iter().enumerate() {
            if total <= area.width as usize {
                break;
            }
            if h.2 == prio && keep[i] {
                keep[i] = false;
                total -= width_of(h);
            }
        }
    }
    let mut spans = vec![Span::raw(" ")];
    for ((key, desc, _), _) in hints.into_iter().zip(keep).filter(|(_, k)| *k) {
        spans.push(Span::styled(key, Style::new().fg(app.pal.accent).bold()));
        spans.push(Span::styled(
            format!(" {desc}   "),
            Style::new().fg(app.pal.muted),
        ));
    }
    f.render_widget(Line::from(spans), area);
}

// ---------------------------------------------------------------- popups

fn popup_area(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

fn popup_block<'a>(app: &App, title: &str) -> Block<'a> {
    Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::new().fg(app.pal.accent))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(app.pal.accent).bold(),
        ))
}

fn draw_popup(f: &mut Frame, app: &mut App, area: Rect) {
    let pal = app.pal.clone();
    let Some(popup) = app.popup.clone() else {
        return;
    };
    let w = (area.width * 7 / 10).clamp(40, 96);
    match popup {
        Popup::Confirm { title, lines, .. } | Popup::Message { title, lines } => {
            let text: Vec<String> = lines
                .iter()
                .flat_map(|l| wrap(l, (w - 4) as usize))
                .collect();
            let h = (text.len() as u16 + 4).min(area.height - 2);
            let r = popup_area(area, w, h);
            f.render_widget(Clear, r);
            let block = popup_block(app, &title);
            let inner = block.inner(r);
            f.render_widget(block, r);
            let mut out: Vec<Line> = text
                .into_iter()
                .map(|l| Line::from(Span::styled(format!(" {l}"), Style::new().fg(pal.fg))))
                .collect();
            out.push(Line::raw(""));
            f.render_widget(Paragraph::new(out), inner);
        }
        Popup::Conflict { files } => {
            let text = wrap(
                &tf("conflict.body", &[("files", &files.join(", "))]),
                (w - 4) as usize,
            );
            let r = popup_area(area, w, text.len() as u16 + 5);
            f.render_widget(Clear, r);
            let block = popup_block(app, &t("conflict.title"));
            let inner = block.inner(r);
            f.render_widget(block, r);
            let mut lines: Vec<Line> = text
                .into_iter()
                .map(|l| Line::from(Span::styled(format!(" {l}"), Style::new().fg(pal.fg))))
                .collect();
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                format!(" {}", t("conflict.keys")),
                Style::new().fg(pal.accent),
            )));
            f.render_widget(Paragraph::new(lines), inner);
        }
        Popup::Input(inp) => {
            let hint = wrap(&inp.hint, (w - 4) as usize);
            let h = hint.len() as u16 + 6 + u16::from(inp.error.is_some());
            let r = popup_area(area, w, h);
            f.render_widget(Clear, r);
            let block = popup_block(app, &inp.title);
            let inner = block.inner(r);
            f.render_widget(block, r);
            let mut lines: Vec<Line> = hint
                .iter()
                .map(|l| Line::from(Span::styled(format!(" {l}"), Style::new().fg(pal.muted))))
                .collect();
            lines.push(Line::raw(""));
            // Campo de texto con desplazamiento horizontal.
            let field_w = inner.width.saturating_sub(4) as usize;
            let before: String = inp.buf[..inp.cursor].iter().collect();
            let skip = before.width().saturating_sub(field_w.saturating_sub(1));
            let shown: String = {
                let mut acc = 0;
                let mut s = String::new();
                for c in inp.buf.iter() {
                    let cw = unicode_width::UnicodeWidthChar::width(*c).unwrap_or(0);
                    if acc >= skip {
                        s.push(*c);
                    }
                    acc += cw;
                }
                s
            };
            lines.push(Line::from(vec![
                Span::styled(" › ", Style::new().fg(pal.accent)),
                Span::styled(
                    pad(&shown, field_w),
                    Style::new().fg(pal.fg).bg(pal.surface),
                ),
            ]));
            if let Some(e) = &inp.error {
                lines.push(Line::from(Span::styled(
                    format!("   {e}"),
                    Style::new().fg(pal.warn),
                )));
            }
            let cursor_y = inner.y + hint.len() as u16 + 1;
            f.render_widget(Paragraph::new(lines), inner);
            let cx = inner.x + 3 + (before.width() - skip) as u16;
            f.set_cursor_position(Position::new(
                cx.min(inner.right().saturating_sub(1)),
                cursor_y,
            ));
        }
        Popup::Picker(p) => {
            let vis = p.visible();
            let mut list_rows: Vec<(Option<usize>, String)> = vec![];
            let mut last_group = String::new();
            for &i in &vis {
                let it = &p.items[i];
                if !it.group.is_empty() && it.group != last_group {
                    last_group = it.group.clone();
                    list_rows.push((None, it.group.clone()));
                }
                list_rows.push((Some(i), it.label.clone()));
            }
            let h = (list_rows.len() as u16 + 7).clamp(8, area.height.saturating_sub(2));
            let r = popup_area(area, w, h);
            f.render_widget(Clear, r);
            let block = popup_block(app, &p.title);
            let inner = block.inner(r);
            f.render_widget(block, r);
            let [flt, list, det] = Layout::vertical([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .areas(inner);
            f.render_widget(
                Line::from(vec![
                    Span::styled(" / ", Style::new().fg(pal.accent)),
                    Span::styled(
                        if p.filter.is_empty() {
                            t("pick.type_to_filter")
                        } else {
                            p.filter.clone()
                        },
                        Style::new().fg(if p.filter.is_empty() { pal.dim } else { pal.fg }),
                    ),
                ]),
                flt,
            );
            if list_rows.is_empty() {
                f.render_widget(
                    Line::from(Span::styled(
                        format!(" {}", t("pick.no_results")),
                        Style::new().fg(pal.dim),
                    )),
                    list,
                );
            }
            let sel_pos = list_rows
                .iter()
                .position(|(i, _)| *i == Some(p.sel))
                .unwrap_or(0);
            let lh = list.height as usize;
            let offset = sel_pos.saturating_sub(lh.saturating_sub(1));
            for (n, (idx, label)) in list_rows.iter().enumerate().skip(offset).take(lh) {
                let r = Rect::new(list.x, list.y + (n - offset) as u16, list.width, 1);
                match idx {
                    None => f.render_widget(
                        Line::from(Span::styled(
                            format!(" {label}"),
                            Style::new().fg(pal.muted).bold(),
                        )),
                        r,
                    ),
                    Some(i) => {
                        let it = &p.items[*i];
                        let is_sel = *i == p.sel;
                        let current = p.current.as_ref() == Some(&it.value);
                        let style = if !it.enabled {
                            Style::new().fg(pal.dim)
                        } else if is_sel {
                            Style::new().fg(pal.accent).bg(pal.selection).bold()
                        } else {
                            Style::new().fg(pal.fg)
                        };
                        let text = format!("{}   {label}", if current { "✓" } else { " " });
                        f.render_widget(
                            Line::from(vec![
                                Span::styled(
                                    if is_sel { "▌" } else { " " },
                                    Style::new().fg(pal.accent),
                                ),
                                Span::styled(
                                    pad(&text, list.width.saturating_sub(1) as usize),
                                    style,
                                ),
                            ]),
                            r,
                        );
                        app.hits.push((r, Hit::PopupItem(*i)));
                    }
                }
            }
            if let Some(it) = p.items.get(p.sel).filter(|_| vis.contains(&p.sel)) {
                let d = wrap(&it.detail, det.width.saturating_sub(2) as usize);
                let lines: Vec<Line> = d
                    .into_iter()
                    .take(3)
                    .map(|l| Line::from(Span::styled(format!(" {l}"), Style::new().fg(pal.muted))))
                    .collect();
                f.render_widget(Paragraph::new(lines), det);
            }
        }
        Popup::Checklist(c) => {
            let h = (c.opts.len() as u16 + 4).clamp(6, area.height.saturating_sub(2));
            let r = popup_area(area, w, h);
            f.render_widget(Clear, r);
            let block = popup_block(app, &c.title);
            let inner = block.inner(r);
            f.render_widget(block, r);
            let lh = inner.height.saturating_sub(1) as usize;
            let offset = c.sel.saturating_sub(lh.saturating_sub(1));
            for (n, o) in c.opts.iter().enumerate().skip(offset).take(lh) {
                let r = Rect::new(inner.x, inner.y + (n - offset) as u16, inner.width, 1);
                let on = c.checked.get(n).copied().unwrap_or(false);
                let is_sel = n == c.sel;
                let mut text = format!("{} {}", if on { "[x]" } else { "[ ]" }, o.label);
                if !o.desc.is_empty() {
                    text = format!("{text} — {}", o.desc);
                }
                let style = if is_sel {
                    Style::new().fg(pal.accent).bg(pal.selection)
                } else {
                    Style::new().fg(pal.fg)
                };
                f.render_widget(
                    Line::from(vec![
                        Span::styled(if is_sel { "▌" } else { " " }, Style::new().fg(pal.accent)),
                        Span::styled(pad(&text, inner.width.saturating_sub(1) as usize), style),
                    ]),
                    r,
                );
                app.hits.push((r, Hit::PopupItem(n)));
            }
            let hint = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
            f.render_widget(
                Line::from(Span::styled(
                    format!(" {}", t("check.empty_means")),
                    Style::new().fg(pal.dim),
                )),
                hint,
            );
        }
        Popup::Help { .. } => {
            let sections: Vec<(String, Vec<(String, String)>)> = vec![
                (
                    t("help.general"),
                    vec![
                        ("Tab / ⇧+Tab".into(), t("help.focus")),
                        ("Esc".into(), t("help.back")),
                        ("1-6".into(), t("help.jump")),
                        ("↑↓ / j k".into(), t("help.nav")),
                        ("← → / h l".into(), t("help.change")),
                        (format!("Enter / {}", t("key.space")), t("help.activate")),
                        (format!("r / {}", t("key.del")), t("help.reset")),
                        ("a".into(), t("help.apply")),
                        ("c".into(), t("help.discard")),
                        ("R".into(), t("help.restore")),
                        ("m".into(), t("help.mode")),
                        ("i".into(), t("help.lang")),
                        ("q".into(), t("help.quit")),
                    ],
                ),
                (
                    t("sec.widgets"),
                    vec![
                        (format!("⇧+{} / H J K L", t("key.arrows")), t("help.w.move")),
                        ("n".into(), t("help.w.add")),
                        (format!("d / {}", t("key.del")), t("help.w.remove")),
                        (t("help.mouse"), t("help.w.drag")),
                    ],
                ),
                (
                    t("sec.plugins"),
                    vec![
                        ("Enter".into(), t("help.p.toggle")),
                        ("/".into(), t("help.p.filter")),
                        ("n".into(), t("help.p.add")),
                        ("p".into(), t("help.p.clone")),
                        ("u / U".into(), t("help.p.update")),
                        ("x".into(), t("help.p.remove")),
                        ("e".into(), t("help.p.edit")),
                    ],
                ),
            ];
            let mut lines: Vec<Line> = vec![];
            let key_w = 22usize;
            let desc_w = (w as usize).saturating_sub(key_w + 6);
            for (title, keys) in sections {
                lines.push(Line::from(Span::styled(
                    format!(" {title}"),
                    Style::new().fg(pal.accent).bold(),
                )));
                for (k, d) in keys {
                    for (n, part) in wrap(&d, desc_w).into_iter().enumerate() {
                        let key = if n == 0 {
                            pad(&k, key_w)
                        } else {
                            " ".repeat(key_w)
                        };
                        lines.push(Line::from(vec![
                            Span::styled(format!("   {key}"), Style::new().fg(pal.fg).bold()),
                            Span::styled(part, Style::new().fg(pal.muted)),
                        ]));
                    }
                }
                lines.push(Line::raw(""));
            }
            for l in wrap(&t("help.modes"), (w - 4) as usize) {
                lines.push(Line::from(Span::styled(
                    format!(" {l}"),
                    Style::new().fg(pal.muted),
                )));
            }
            let total = lines.len() as u16;
            let r = popup_area(area, w, total + 2);
            let visible = r.height.saturating_sub(2);
            let max_scroll = total.saturating_sub(visible);
            if let Some(Popup::Help { scroll }) = app.popup.as_mut() {
                *scroll = (*scroll).min(max_scroll);
            }
            let scroll = match app.popup {
                Some(Popup::Help { scroll }) => scroll,
                _ => 0,
            };
            f.render_widget(Clear, r);
            let title = if max_scroll > 0 {
                format!(
                    "{} ({}/{})",
                    t("help.title"),
                    scroll + visible.min(total),
                    total
                )
            } else {
                t("help.title")
            };
            let block = popup_block(app, &title);
            let inner = block.inner(r);
            f.render_widget(block, r);
            f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
        }
    }
}
