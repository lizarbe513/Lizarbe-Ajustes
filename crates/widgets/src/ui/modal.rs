//! Ventanas emergentes con el estilo de Meca: ventanas con botones en relieve
//! clicables, desplegables pegados al control que los abre y menú contextual.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Clear};
use unicode_width::UnicodeWidthStr;

use super::popup::{Menu, MenuAct, Picker, Popup};
use super::{fg, pad, put, truncate, wrap};
use lizarbe_core::ui::{bevel_box, button_look, centered, frame, row_style};
use crate::app::{App, Hit};
use crate::i18n::{t, tf};

pub(super) fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(popup) = app.popup.clone() else {
        return;
    };
    match &popup {
        Popup::Menu(m) => draw_menu(f, app, area, m),
        Popup::Picker(p) if p.anchor.is_some() => draw_dropdown(f, app, area, p),
        _ => draw_window(f, app, area, &popup),
    }
}

fn hover_is(app: &App, hit: Hit) -> bool {
    app.hover == Some(hit)
}

/// Botones de la ventana, alineados a la derecha (como en Meca).
fn draw_modal_buttons(f: &mut Frame, app: &mut App, popup: &Popup, area: Rect) {
    let buttons = popup.buttons();
    let labels: Vec<String> = buttons.iter().map(|(k, _)| format!(" {} ", t(k))).collect();
    let gap = 2u16;
    let total: u16 = labels.iter().map(|l| l.width() as u16 + 2).sum::<u16>()
        + gap * labels.len().saturating_sub(1) as u16;
    let mut x = area.right().saturating_sub(total + 1);
    for (i, ((_, key), label)) in buttons.iter().zip(labels).enumerate() {
        let hit = Hit::ModalButton(i);
        let hover = hover_is(app, hit);
        let primary = *key == KeyCode::Enter;
        let w = label.width();
        let (border, shadow, text) = button_look(&app.pal, true, hover, false, primary);
        bevel_box(
            f,
            x,
            area.y,
            vec![Span::styled(label, text)],
            w,
            border,
            shadow,
            hover || primary,
        );
        app.hits.push((Rect::new(x, area.y, w as u16 + 2, 3), hit));
        x += w as u16 + 2 + gap;
    }
}

// ---------------------------------------------------------------- ventanas

fn draw_window(f: &mut Frame, app: &mut App, area: Rect, popup: &Popup) {
    let w = (area.width * 7 / 10)
        .clamp(44, 96)
        .min(area.width.saturating_sub(2));
    let tw = (w as usize).saturating_sub(4);
    let has_buttons = !popup.buttons().is_empty();
    let buttons_h: u16 = if has_buttons { 4 } else { 0 };
    let max_body = area.height.saturating_sub(4 + buttons_h).max(3);

    match popup {
        Popup::Confirm { title, lines, .. } | Popup::Message { title, lines } => {
            let text: Vec<String> = lines.iter().flat_map(|l| wrap(l, tw)).collect();
            let body_h = (text.len() as u16).min(max_body);
            let r = centered(area, w, body_h + 2 + buttons_h);
            let inner = frame(f, &app.pal, r, title);
            for (n, l) in text.iter().take(body_h as usize).enumerate() {
                put(
                    f,
                    inner.x,
                    inner.y + n as u16,
                    vec![Span::styled(format!(" {l}"), fg(app.pal.fg))],
                );
            }
            buttons_at(f, app, popup, inner);
        }
        Popup::Conflict { files } => {
            let text = wrap(&tf("conflict.body", &[("files", &files.join(", "))]), tw);
            let r = centered(area, w, text.len() as u16 + 2 + buttons_h);
            let inner = frame(f, &app.pal, r, &t("conflict.title"));
            for (n, l) in text.iter().enumerate() {
                put(
                    f,
                    inner.x,
                    inner.y + n as u16,
                    vec![Span::styled(format!(" {l}"), fg(app.pal.fg))],
                );
            }
            buttons_at(f, app, popup, inner);
        }
        Popup::Input(inp) => {
            let hint = wrap(&inp.hint, tw);
            let body_h = hint.len() as u16 + 2 + u16::from(inp.error.is_some());
            let r = centered(area, w, body_h + 2 + buttons_h);
            let inner = frame(f, &app.pal, r, &inp.title);
            for (n, l) in hint.iter().enumerate() {
                put(
                    f,
                    inner.x,
                    inner.y + n as u16,
                    vec![Span::styled(format!(" {l}"), fg(app.pal.muted))],
                );
            }
            // Campo de texto con desplazamiento horizontal.
            let fy = inner.y + hint.len() as u16 + 1;
            let field_w = inner.width.saturating_sub(5) as usize;
            let before: String = inp.buf[..inp.cursor].iter().collect();
            let skip = before.width().saturating_sub(field_w.saturating_sub(1));
            let mut acc = 0;
            let mut shown = String::new();
            for c in &inp.buf {
                if acc >= skip {
                    shown.push(*c);
                }
                acc += unicode_width::UnicodeWidthChar::width(*c).unwrap_or(0);
            }
            put(
                f,
                inner.x,
                fy,
                vec![
                    Span::styled(" ┃ ", fg(app.pal.accent).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        pad(&shown, field_w),
                        Style::new().fg(app.pal.bright).bg(app.pal.soft_selection),
                    ),
                ],
            );
            if let Some(e) = &inp.error {
                put(
                    f,
                    inner.x,
                    fy + 1,
                    vec![Span::styled(format!("    {e}"), fg(app.pal.warn))],
                );
            }
            let cx = inner.x + 3 + (before.width() - skip) as u16;
            f.set_cursor_position(Position::new(cx.min(inner.right().saturating_sub(1)), fy));
            buttons_at(f, app, popup, inner);
        }
        Popup::Picker(p) => draw_big_picker(f, app, area, w, popup, p),
        Popup::Checklist(c) => {
            let body_h = (c.opts.len() as u16 + 2).min(max_body);
            let r = centered(area, w, body_h + 2 + buttons_h);
            let inner = frame(f, &app.pal, r, &c.title);
            let lh = body_h.saturating_sub(2) as usize;
            let offset = c.sel.saturating_sub(lh.saturating_sub(1));
            for (n, o) in c.opts.iter().enumerate().skip(offset).take(lh) {
                let y = inner.y + (n - offset) as u16;
                let hit = Hit::PopupItem(n);
                let on = c.checked.get(n).copied().unwrap_or(false);
                let style = row_style(&app.pal, hover_is(app, hit), n == c.sel, true);
                let mut text = format!(" {} {}", if on { "■" } else { "□" }, o.label);
                if !o.desc.is_empty() {
                    text = format!("{text}  — {}", o.desc);
                }
                put(
                    f,
                    inner.x,
                    y,
                    vec![
                        Span::styled(if n == c.sel { "▌" } else { " " }, fg(app.pal.accent)),
                        Span::styled(pad(&text, inner.width as usize - 1), style),
                    ],
                );
                app.hits.push((Rect::new(inner.x, y, inner.width, 1), hit));
            }
            put(
                f,
                inner.x,
                inner.y + lh as u16 + 1,
                vec![Span::styled(
                    format!(" {}", t("check.empty_means")),
                    fg(app.pal.dim),
                )],
            );
            buttons_at(f, app, popup, inner);
        }
        Popup::Help { scroll } => draw_help(f, app, area, w, popup, *scroll),
        Popup::Menu(_) => {}
    }
}

fn buttons_at(f: &mut Frame, app: &mut App, popup: &Popup, inner: Rect) {
    if popup.buttons().is_empty() || inner.height < 4 {
        return;
    }
    let y = inner.bottom() - 3;
    draw_modal_buttons(f, app, popup, Rect::new(inner.x, y, inner.width, 3));
}

/// Selector grande (añadir widget): buscador, lista agrupada y detalle.
fn draw_big_picker(f: &mut Frame, app: &mut App, area: Rect, w: u16, popup: &Popup, p: &Picker) {
    let vis = p.visible();
    let mut rows: Vec<(Option<usize>, String)> = vec![];
    let mut last_group = String::new();
    for &i in &vis {
        let it = &p.items[i];
        if !it.group.is_empty() && it.group != last_group {
            last_group = it.group.clone();
            rows.push((None, it.group.clone()));
        }
        rows.push((Some(i), it.label.clone()));
    }
    let h = (rows.len() as u16 + 10).clamp(14, area.height.saturating_sub(2));
    let r = centered(area, w, h);
    let inner = frame(f, &app.pal, r, &p.title);
    let iw = inner.width as usize;

    put(
        f,
        inner.x,
        inner.y,
        vec![
            Span::styled(" 󰍉 ", fg(app.pal.accent)),
            Span::styled(
                if p.filter.is_empty() {
                    t("pick.type_to_filter")
                } else {
                    format!("{}▏", p.filter)
                },
                fg(if p.filter.is_empty() {
                    app.pal.dim
                } else {
                    app.pal.bright
                }),
            ),
        ],
    );
    let list = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2 + 2 + 4),
    );
    if rows.is_empty() {
        put(
            f,
            list.x,
            list.y,
            vec![Span::styled(
                format!(" {}", t("pick.no_results")),
                fg(app.pal.dim),
            )],
        );
    }
    let sel_pos = rows
        .iter()
        .position(|(i, _)| *i == Some(p.sel))
        .unwrap_or(0);
    let lh = list.height as usize;
    let offset = sel_pos.saturating_sub(lh.saturating_sub(1));
    for (n, (idx, label)) in rows.iter().enumerate().skip(offset).take(lh) {
        let y = list.y + (n - offset) as u16;
        match idx {
            None => {
                let head = format!(" ━━ {label} ");
                put(
                    f,
                    list.x,
                    y,
                    vec![Span::styled(
                        format!("{head}{}", "━".repeat(iw.saturating_sub(head.width() + 1))),
                        fg(app.pal.accent).add_modifier(Modifier::BOLD),
                    )],
                );
            }
            Some(i) => {
                let it = &p.items[*i];
                let hit = Hit::PopupItem(*i);
                let style = row_style(&app.pal, hover_is(app, hit), *i == p.sel, it.enabled);
                let current = p.current.as_ref() == Some(&it.value);
                put(
                    f,
                    list.x,
                    y,
                    vec![
                        Span::styled(if *i == p.sel { "▌" } else { " " }, fg(app.pal.accent)),
                        Span::styled(
                            pad(
                                &format!(" {} {label}", if current { "✓" } else { " " }),
                                iw.saturating_sub(1),
                            ),
                            style,
                        ),
                    ],
                );
                app.hits.push((Rect::new(list.x, y, list.width, 1), hit));
            }
        }
    }
    // Descripción del elemento seleccionado.
    if let Some(it) = p.items.get(p.sel).filter(|_| vis.contains(&p.sel)) {
        let dy = list.bottom();
        for (n, l) in wrap(&it.detail, iw.saturating_sub(2))
            .into_iter()
            .take(2)
            .enumerate()
        {
            put(
                f,
                inner.x,
                dy + n as u16,
                vec![Span::styled(format!(" {l}"), fg(app.pal.muted))],
            );
        }
    }
    buttons_at(f, app, popup, inner);
}

fn draw_help(f: &mut Frame, app: &mut App, area: Rect, w: u16, popup: &Popup, scroll: u16) {
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
                ("o".into(), t("help.menu")),
                ("a".into(), t("help.apply")),
                ("c".into(), t("help.discard")),
                ("R".into(), t("help.restore")),
                ("m".into(), t("help.mode")),
                ("i".into(), t("help.lang")),
                ("q".into(), t("help.quit")),
            ],
        ),
        (
            t("help.mouse"),
            vec![
                (t("help.m.hover"), t("help.m.hover.desc")),
                (t("help.m.click"), t("help.m.click.desc")),
                (t("help.m.right"), t("help.m.right.desc")),
                (t("help.m.wheel"), t("help.m.wheel.desc")),
                (t("help.m.drag"), t("help.w.drag")),
            ],
        ),
        (
            t("sec.widgets"),
            vec![
                (format!("⇧+{} / H J K L", t("key.arrows")), t("help.w.move")),
                ("n".into(), t("help.w.add")),
                (format!("d / {}", t("key.del")), t("help.w.remove")),
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
    let key_w = 22usize;
    let desc_w = (w as usize).saturating_sub(key_w + 7);
    let mut lines: Vec<Vec<Span>> = vec![];
    for (title, keys) in sections {
        let head = format!(" ━━ {title} ");
        lines.push(vec![Span::styled(
            head,
            fg(app.pal.accent).add_modifier(Modifier::BOLD),
        )]);
        for (k, d) in keys {
            for (n, part) in wrap(&d, desc_w).into_iter().enumerate() {
                let key = if n == 0 {
                    pad(&k, key_w)
                } else {
                    " ".repeat(key_w)
                };
                lines.push(vec![
                    Span::styled(
                        format!("   {key}"),
                        fg(app.pal.bright).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(part, fg(app.pal.muted)),
                ]);
            }
        }
        lines.push(vec![]);
    }
    for l in wrap(&t("help.modes"), (w as usize).saturating_sub(4)) {
        lines.push(vec![Span::styled(format!(" {l}"), fg(app.pal.muted))]);
    }
    let total = lines.len() as u16;
    let r = centered(area, w, total + 2 + 4);
    let visible = r.height.saturating_sub(6);
    let max_scroll = total.saturating_sub(visible);
    let scroll = scroll.min(max_scroll);
    if let Some(Popup::Help { scroll: s }) = app.popup.as_mut() {
        *s = scroll;
    }
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
    let inner = frame(f, &app.pal, r, &title);
    for (n, l) in lines
        .into_iter()
        .skip(scroll as usize)
        .take(visible as usize)
        .enumerate()
    {
        put(f, inner.x, inner.y + n as u16, l);
    }
    buttons_at(f, app, popup, inner);
}

// ---------------------------------------------------------------- desplegables

/// Lista pegada al control que la abrió, como los `select` de Meca.
fn draw_dropdown(f: &mut Frame, app: &mut App, area: Rect, p: &Picker) {
    let Some(anchor) = p.anchor else { return };
    let vis = p.visible();
    let label_w = p.items.iter().map(|i| i.label.width()).max().unwrap_or(10);
    let w = (label_w as u16 + 7)
        .max(anchor.width)
        .max(20)
        .min(area.width.saturating_sub(2));
    let filter_h = u16::from(!p.filter.is_empty());
    let want = vis.len().max(1) as u16 + 2 + filter_h;
    let below = area.bottom().saturating_sub(anchor.bottom());
    let above = anchor.y.saturating_sub(area.y);
    let (y, h) = if want <= below || below >= above {
        (anchor.bottom(), want.min(below))
    } else {
        let h = want.min(above);
        (anchor.y - h, h)
    };
    let x = anchor.x.min(area.right().saturating_sub(w));
    let r = Rect::new(x, y, w, h.max(3));
    f.render_widget(Clear, r);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(fg(app.pal.accent).add_modifier(Modifier::BOLD));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let iw = inner.width as usize;
    let mut ly = inner.y;
    if filter_h == 1 {
        put(
            f,
            inner.x,
            ly,
            vec![Span::styled(
                pad(&format!(" 󰍉 {}▏", p.filter), iw),
                fg(app.pal.bright),
            )],
        );
        ly += 1;
    }
    let lh = inner.bottom().saturating_sub(ly) as usize;
    if vis.is_empty() {
        put(
            f,
            inner.x,
            ly,
            vec![Span::styled(
                format!(" {}", t("pick.no_results")),
                fg(app.pal.dim),
            )],
        );
    }
    let sel_pos = vis.iter().position(|&i| i == p.sel).unwrap_or(0);
    let offset = sel_pos.saturating_sub(lh.saturating_sub(1));
    for (n, &i) in vis.iter().enumerate().skip(offset).take(lh) {
        let it = &p.items[i];
        let hit = Hit::PopupItem(i);
        let y = ly + (n - offset) as u16;
        let current = p.current.as_ref() == Some(&it.value);
        let style = row_style(&app.pal, hover_is(app, hit), i == p.sel, it.enabled);
        put(
            f,
            inner.x,
            y,
            vec![Span::styled(
                pad(
                    &format!(" {} {}", if current { "✓" } else { " " }, it.label),
                    iw,
                ),
                style,
            )],
        );
        app.hits.push((Rect::new(inner.x, y, inner.width, 1), hit));
    }
}

// ---------------------------------------------------------------- menú

fn draw_menu(f: &mut Frame, app: &mut App, area: Rect, m: &Menu) {
    // Antes de las acciones generales va una línea separadora.
    let split = m
        .items
        .iter()
        .position(|i| i.act == MenuAct::Apply)
        .filter(|&p| p > 0);
    let label_w = m.items.iter().map(|i| i.label.width()).max().unwrap_or(10);
    let w = (label_w as u16 + 8).min(area.width.saturating_sub(2));
    let h = (m.items.len() as u16 + 2 + u16::from(split.is_some())).min(area.height);
    let x = m.at.0.min(area.right().saturating_sub(w));
    let y = if m.at.1 + h <= area.bottom() {
        m.at.1
    } else {
        area.bottom().saturating_sub(h)
    };
    let r = Rect::new(x, y, w, h);
    f.render_widget(Clear, r);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(fg(app.pal.accent).add_modifier(Modifier::BOLD));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let iw = inner.width as usize;
    let mut ly = inner.y;
    for (i, it) in m.items.iter().enumerate() {
        if ly >= inner.bottom() {
            break;
        }
        if Some(i) == split {
            put(
                f,
                inner.x,
                ly,
                vec![Span::styled("─".repeat(iw), fg(app.pal.dim))],
            );
            ly += 1;
        }
        let hit = Hit::MenuItem(i);
        let style = row_style(&app.pal, hover_is(app, hit), i == m.sel, it.enabled);
        put(
            f,
            inner.x,
            ly,
            vec![Span::styled(
                pad(&truncate(&format!(" {}  {}", it.icon, it.label), iw), iw),
                style,
            )],
        );
        if it.enabled {
            app.hits.push((Rect::new(inner.x, ly, inner.width, 1), hit));
        }
        ly += 1;
    }
}
