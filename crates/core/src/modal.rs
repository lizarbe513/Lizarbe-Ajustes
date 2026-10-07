//! Ventanas emergentes con el estilo de Meca: ventanas con botones en relieve
//! clicables, desplegables pegados al control que los abre y menú contextual.

use ratatui::Frame;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Clear};
use unicode_width::UnicodeWidthStr;

use crate::i18n::{t, tf};
use crate::popup::{Menu, Picker, Popup, PopupTypes};
use crate::ui::{
    button_spans, button_width, centered, fg, frame, key_text, list_row, pad, put, truncate, wrap,
};
use crate::view::{CoreHit, Ctx};

/// Dibuja la ventana emergente encima de todo lo demás.
pub fn draw<T: PopupTypes, H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    popup: &mut Popup<T>,
) {
    let snapshot = popup.clone();
    match &snapshot {
        Popup::Menu(m) => draw_menu(f, ctx, area, m),
        Popup::Picker(p) if p.anchor.is_some() => draw_dropdown(f, ctx, area, p),
        Popup::Help { scroll, .. } => {
            let w = window_width(area);
            let clamped = draw_help(f, ctx, area, w, &snapshot, *scroll);
            if let Popup::Help { scroll, .. } = popup {
                *scroll = clamped;
            }
        }
        _ => draw_window(f, ctx, area, &snapshot),
    }
}

fn window_width(area: Rect) -> u16 {
    (area.width * 7 / 10)
        .clamp(44, 96)
        .min(area.width.saturating_sub(2))
}

/// Botones de texto de la ventana, alineados a la derecha: `[⏎] Aceptar   [Esc] Cancelar`.
fn draw_modal_buttons<T: PopupTypes, H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    popup: &Popup<T>,
    area: Rect,
) {
    let buttons: Vec<(String, String, bool)> = popup
        .buttons()
        .iter()
        .map(|(k, code)| (key_text(*code), t(k), *code == KeyCode::Enter))
        .collect();
    let gap = 3u16;
    let total: u16 = buttons
        .iter()
        .map(|(k, l, _)| button_width(k, l) as u16)
        .sum::<u16>()
        + gap * buttons.len().saturating_sub(1) as u16;
    let mut x = area.right().saturating_sub(total + 2);
    for (i, (key, label, primary)) in buttons.iter().enumerate() {
        let hit = H::modal_button(i);
        let w = button_width(key, label) as u16;
        put(
            f,
            x,
            area.y,
            button_spans(ctx.pal, key, label, true, *primary, ctx.hover_is(hit)),
        );
        ctx.hit(Rect::new(x, area.y, w, 1), hit);
        x += w + gap;
    }
}

// ---------------------------------------------------------------- ventanas

fn draw_window<T: PopupTypes, H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    popup: &Popup<T>,
) {
    let w = window_width(area);
    let tw = (w as usize).saturating_sub(4);
    let has_buttons = !popup.buttons().is_empty();
    let buttons_h: u16 = if has_buttons { 2 } else { 0 };
    let max_body = area.height.saturating_sub(4 + buttons_h).max(3);

    match popup {
        Popup::Confirm { title, lines, .. } | Popup::Message { title, lines } => {
            let text: Vec<String> = lines.iter().flat_map(|l| wrap(l, tw)).collect();
            let body_h = (text.len() as u16).min(max_body);
            let r = centered(area, w, body_h + 2 + buttons_h);
            let inner = frame(f, ctx.pal, r, title);
            for (n, l) in text.iter().take(body_h as usize).enumerate() {
                put(
                    f,
                    inner.x,
                    inner.y + n as u16,
                    vec![Span::styled(format!("  {l}"), fg(ctx.pal.fg))],
                );
            }
            buttons_at(f, ctx, popup, inner);
        }
        Popup::Conflict { files } => {
            let text = wrap(&tf("conflict.body", &[("files", &files.join(", "))]), tw);
            let r = centered(area, w, text.len() as u16 + 2 + buttons_h);
            let inner = frame(f, ctx.pal, r, &t("conflict.title"));
            for (n, l) in text.iter().enumerate() {
                put(
                    f,
                    inner.x,
                    inner.y + n as u16,
                    vec![Span::styled(format!("  {l}"), fg(ctx.pal.fg))],
                );
            }
            buttons_at(f, ctx, popup, inner);
        }
        Popup::Input(inp) => {
            let hint = wrap(&inp.hint, tw);
            let body_h = hint.len() as u16 + 2 + u16::from(inp.error.is_some());
            let r = centered(area, w, body_h + 2 + buttons_h);
            let inner = frame(f, ctx.pal, r, &inp.title);
            for (n, l) in hint.iter().enumerate() {
                put(
                    f,
                    inner.x,
                    inner.y + n as u16,
                    vec![Span::styled(format!("  {l}"), fg(ctx.pal.muted))],
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
                    Span::styled("  ▍", fg(ctx.pal.accent).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        pad(&shown, field_w),
                        Style::new()
                            .fg(ctx.pal.bright)
                            .add_modifier(Modifier::UNDERLINED),
                    ),
                ],
            );
            if let Some(e) = &inp.error {
                put(
                    f,
                    inner.x,
                    fy + 1,
                    vec![Span::styled(format!("    {e}"), fg(ctx.pal.warn))],
                );
            }
            let cx = inner.x + 3 + (before.width() - skip) as u16;
            f.set_cursor_position(Position::new(cx.min(inner.right().saturating_sub(1)), fy));
            buttons_at(f, ctx, popup, inner);
        }
        Popup::Picker(p) => draw_big_picker(f, ctx, area, w, popup, p),
        Popup::Color(c) => draw_color(f, ctx, area, popup, c),
        Popup::Checklist(c) => {
            let body_h = (c.opts.len() as u16 + 2).min(max_body);
            let r = centered(area, w, body_h + 2 + buttons_h);
            let inner = frame(f, ctx.pal, r, &c.title);
            let lh = body_h.saturating_sub(2) as usize;
            let offset = c.sel.saturating_sub(lh.saturating_sub(1));
            for (n, o) in c.opts.iter().enumerate().skip(offset).take(lh) {
                let y = inner.y + (n - offset) as u16;
                let hit = H::popup_item(n);
                let on = c.checked.get(n).copied().unwrap_or(false);
                let mut text = format!(" {} {}", if on { "●" } else { "○" }, o.label);
                if !o.desc.is_empty() {
                    text = format!("{text}  — {}", o.desc);
                }
                put(
                    f,
                    inner.x,
                    y,
                    list_row(
                        ctx.pal,
                        &text,
                        inner.width as usize,
                        n == c.sel,
                        ctx.hover_is(hit),
                        true,
                    ),
                );
                ctx.hit(Rect::new(inner.x, y, inner.width, 1), hit);
            }
            put(
                f,
                inner.x,
                inner.y + lh as u16 + 1,
                vec![Span::styled(
                    format!(" {}", t("check.empty_means")),
                    fg(ctx.pal.dim),
                )],
            );
            buttons_at(f, ctx, popup, inner);
        }
        Popup::Help { .. } => {}
        Popup::Menu(_) => {}
    }
}

fn buttons_at<T: PopupTypes, H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    popup: &Popup<T>,
    inner: Rect,
) {
    if popup.buttons().is_empty() || inner.height < 2 {
        return;
    }
    let y = inner.bottom() - 1;
    draw_modal_buttons(f, ctx, popup, Rect::new(inner.x, y, inner.width, 1));
}

/// Selector de color: barras de tono, saturación y luminosidad, hex y muestras.
fn draw_color<T: PopupTypes, H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    popup: &Popup<T>,
    c: &crate::popup::ColorPick<T>,
) {
    use crate::color::{from_hsl, parse_hex};
    use ratatui::style::Color;
    let pal = ctx.pal;
    let w = 62u16.min(area.width.saturating_sub(2));
    let r = centered(area, w, 15);
    let inner = frame(f, pal, r, &c.title);
    let bar_w = (inner.width as usize).saturating_sub(24).clamp(12, 40);
    let rgb = |c: (u8, u8, u8)| Color::Rgb(c.0, c.1, c.2);
    let names = [
        ("H", c.h / 360.0, format!("{:>3.0}°", c.h)),
        ("S", c.s, format!("{:>3.0}%", c.s * 100.0)),
        ("L", c.l, format!("{:>3.0}%", c.l * 100.0)),
    ];
    for (i, (name, frac, label)) in names.iter().enumerate() {
        let y = inner.y + 1 + i as u16 * 2;
        let active = c.chan == i;
        let pos = ((frac.clamp(0.0, 1.0)) * (bar_w as f32 - 1.0)).round() as usize;
        let mut spans = vec![
            Span::styled(
                if active { "  ▍" } else { "   " },
                fg(pal.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{name} "),
                if active {
                    fg(pal.bright).add_modifier(Modifier::BOLD)
                } else {
                    fg(pal.muted)
                },
            ),
        ];
        for k in 0..bar_w {
            let t = k as f32 / (bar_w as f32 - 1.0);
            let col = match i {
                0 => from_hsl(t * 360.0, c.s.max(0.75), c.l.clamp(0.35, 0.65)),
                1 => from_hsl(c.h, t, c.l),
                _ => from_hsl(c.h, c.s, t),
            };
            if k == pos {
                spans.push(Span::styled(
                    "●",
                    Style::new()
                        .fg(pal.bright)
                        .bg(rgb(col))
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled("█", fg(rgb(col))));
            }
        }
        spans.push(Span::styled(format!("  {label}"), fg(pal.fg)));
        put(f, inner.x, y, spans);
    }
    // Hex, antes y ahora.
    let y = inner.y + 7;
    let text: String = c.text.iter().collect();
    let shown = if text.starts_with('#') {
        text.clone()
    } else {
        format!("#{text}")
    };
    let active = c.chan == 3;
    put(
        f,
        inner.x,
        y,
        vec![
            Span::styled(
                if active { "  ▍" } else { "   " },
                fg(pal.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "# ",
                if active {
                    fg(pal.bright)
                } else {
                    fg(pal.muted)
                },
            ),
            Span::styled(
                format!("{:<8}", shown.trim_start_matches('#')),
                if active {
                    fg(pal.bright).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    fg(pal.fg)
                },
            ),
            Span::styled("   antes ", fg(pal.muted)),
            Span::styled("██", fg(rgb(parse_hex(&c.original).unwrap_or((0, 0, 0))))),
            Span::styled("   ahora ", fg(pal.muted)),
            Span::styled("██", fg(rgb(c.rgb()))),
        ],
    );
    if active {
        let cx = inner.x + 5 + shown.trim_start_matches('#').chars().count() as u16;
        f.set_cursor_position(Position::new(cx.min(inner.right().saturating_sub(1)), y));
    }
    // Muestras del tema (1-9).
    if !c.swatches.is_empty() {
        let mut spans = vec![Span::raw("   ")];
        for (i, hex) in c.swatches.iter().take(9).enumerate() {
            let col = rgb(parse_hex(hex).unwrap_or((0, 0, 0)));
            spans.push(Span::styled(format!("{}", i + 1), fg(pal.muted)));
            spans.push(Span::styled("██ ", fg(col)));
        }
        put(f, inner.x, inner.y + 9, spans);
    }
    put(
        f,
        inner.x + 3,
        inner.y + 10,
        vec![Span::styled(
            "←→ ajustar · ⇧ más rápido · ↑↓ canal · r original",
            fg(pal.dim),
        )],
    );
    buttons_at(f, ctx, popup, inner);
}

/// Selector grande (añadir widget): buscador, lista agrupada y detalle.
fn draw_big_picker<T: PopupTypes, H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    w: u16,
    popup: &Popup<T>,
    p: &Picker<T>,
) {
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
    let inner = frame(f, ctx.pal, r, &p.title);
    let iw = inner.width as usize;

    put(
        f,
        inner.x,
        inner.y,
        vec![
            Span::styled(" 󰍉 ", fg(ctx.pal.accent)),
            Span::styled(
                if p.filter.is_empty() {
                    t("pick.type_to_filter")
                } else {
                    format!("{}▏", p.filter)
                },
                fg(if p.filter.is_empty() {
                    ctx.pal.dim
                } else {
                    ctx.pal.bright
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
                fg(ctx.pal.dim),
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
                put(
                    f,
                    list.x + 2,
                    y,
                    vec![
                        Span::styled(
                            label.to_uppercase(),
                            fg(ctx.pal.muted).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" ───", fg(ctx.pal.rule)),
                    ],
                );
            }
            Some(i) => {
                let it = &p.items[*i];
                let hit = H::popup_item(*i);
                let current = p.current.as_ref() == Some(&it.value);
                put(
                    f,
                    list.x,
                    y,
                    list_row(
                        ctx.pal,
                        &format!(" {} {label}", if current { "✓" } else { " " }),
                        iw,
                        *i == p.sel,
                        ctx.hover_is(hit),
                        it.enabled,
                    ),
                );
                ctx.hit(Rect::new(list.x, y, list.width, 1), hit);
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
                vec![Span::styled(format!("  {l}"), fg(ctx.pal.muted))],
            );
        }
    }
    buttons_at(f, ctx, popup, inner);
}

/// Ayuda con desplazamiento; devuelve el desplazamiento ajustado al contenido.
fn draw_help<T: PopupTypes, H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    w: u16,
    popup: &Popup<T>,
    scroll: u16,
) -> u16 {
    let Popup::Help { content, .. } = popup else {
        return scroll;
    };
    let pal = ctx.pal;
    let key_w = 22usize;
    let desc_w = (w as usize).saturating_sub(key_w + 7);
    let mut lines: Vec<Vec<Span>> = vec![];
    for (title, keys) in &content.sections {
        lines.push(vec![
            Span::styled(
                format!("  {}", title.to_uppercase()),
                fg(pal.muted).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ───", fg(pal.rule)),
        ]);
        for (k, d) in keys {
            for (n, part) in wrap(d, desc_w).into_iter().enumerate() {
                let key = if n == 0 {
                    pad(k, key_w)
                } else {
                    " ".repeat(key_w)
                };
                lines.push(vec![
                    Span::styled(
                        format!("   {key}"),
                        fg(pal.bright).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(part, fg(pal.muted)),
                ]);
            }
        }
        lines.push(vec![]);
    }
    if !content.footer.is_empty() {
        for l in wrap(&content.footer, (w as usize).saturating_sub(4)) {
            lines.push(vec![Span::styled(format!(" {l}"), fg(pal.muted))]);
        }
    }
    let total = lines.len() as u16;
    let r = centered(area, w, total + 2 + 4);
    let visible = r.height.saturating_sub(6);
    let max_scroll = total.saturating_sub(visible);
    let scroll = scroll.min(max_scroll);
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
    let inner = frame(f, ctx.pal, r, &title);
    for (n, l) in lines
        .into_iter()
        .skip(scroll as usize)
        .take(visible as usize)
        .enumerate()
    {
        put(f, inner.x, inner.y + n as u16, l);
    }
    buttons_at(f, ctx, popup, inner);
    scroll
}

// ---------------------------------------------------------------- desplegables

/// Lista pegada al control que la abrió, como los `select` de Meca.
fn draw_dropdown<T: PopupTypes, H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    p: &Picker<T>,
) {
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
        .border_type(BorderType::Rounded)
        .border_style(fg(ctx.pal.rule));
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
                fg(ctx.pal.bright),
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
                fg(ctx.pal.dim),
            )],
        );
    }
    let sel_pos = vis.iter().position(|&i| i == p.sel).unwrap_or(0);
    let offset = sel_pos.saturating_sub(lh.saturating_sub(1));
    for (n, &i) in vis.iter().enumerate().skip(offset).take(lh) {
        let it = &p.items[i];
        let hit = H::popup_item(i);
        let y = ly + (n - offset) as u16;
        let current = p.current.as_ref() == Some(&it.value);
        put(
            f,
            inner.x,
            y,
            list_row(
                ctx.pal,
                &format!(" {} {}", if current { "✓" } else { " " }, it.label),
                iw,
                i == p.sel,
                ctx.hover_is(hit),
                it.enabled,
            ),
        );
        ctx.hit(Rect::new(inner.x, y, inner.width, 1), hit);
    }
}

// ---------------------------------------------------------------- menú

fn draw_menu<T: PopupTypes, H: CoreHit>(f: &mut Frame, ctx: &mut Ctx<H>, area: Rect, m: &Menu<T>) {
    let seps = m.items.iter().skip(1).filter(|i| i.separator).count() as u16;
    let label_w = m.items.iter().map(|i| i.label.width()).max().unwrap_or(10);
    let w = (label_w as u16 + 8).min(area.width.saturating_sub(2));
    let h = (m.items.len() as u16 + 2 + seps).min(area.height);
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
        .border_type(BorderType::Rounded)
        .border_style(fg(ctx.pal.rule));
    let inner = block.inner(r);
    f.render_widget(block, r);
    let iw = inner.width as usize;
    let mut ly = inner.y;
    for (i, it) in m.items.iter().enumerate() {
        if ly >= inner.bottom() {
            break;
        }
        if i > 0 && it.separator {
            put(
                f,
                inner.x,
                ly,
                vec![Span::styled("─".repeat(iw), fg(ctx.pal.rule))],
            );
            ly += 1;
        }
        let hit = H::menu_item(i);
        put(
            f,
            inner.x,
            ly,
            list_row(
                ctx.pal,
                &truncate(&format!(" {}  {}", it.icon, it.label), iw),
                iw,
                i == m.sel,
                ctx.hover_is(hit),
                it.enabled,
            ),
        );
        if it.enabled {
            ctx.hit(Rect::new(inner.x, ly, inner.width, 1), hit);
        }
        ly += 1;
    }
}
