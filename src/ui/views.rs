//! Panel de contenido: encabezado de sección, formularios con filas de tres
//! líneas y controles en relieve (estilo Meca), columnas de widgets, lista de
//! plugins y botones 3D Restaurar · Cancelar · Aplicar.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders};
use serde_json::Value;
use unicode_width::UnicodeWidthStr;

use super::form::{self, FieldRow, NoteKind, Row};
use super::{center, fg, hovered, pad, put, right_align, truncate, wrap};
use crate::app::{App, Button, Focus, Hit, PluginRow, Section, Sub};
use crate::i18n::{t, tf};
use crate::omarchy::curated;
use crate::omarchy::qt_format;
use crate::omarchy::schema::{self, Kind, value_label};
use crate::omarchy::shell_json as sj;
use crate::omarchy::theme::hex;
use crate::store::Bind;

/// Alto de la zona de botones: separador + botones de 3 líneas.
const BUTTONS_H: u16 = 4;

pub(super) fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Content && app.popup.is_none();
    let x = area.x + 1;
    let w = area.width.saturating_sub(2);
    if area.height < 10 || w < 30 {
        return;
    }

    // Encabezado: título en mayúsculas, descripción y separador.
    let (title, desc) = section_header(app);
    put(
        f,
        x,
        area.y,
        vec![
            Span::styled(
                if focused { "▍" } else { " " },
                fg(app.pal.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                truncate(&title.to_uppercase(), w as usize - 1),
                fg(app.pal.bright).add_modifier(Modifier::BOLD),
            ),
        ],
    );
    put(
        f,
        x,
        area.y + 1,
        vec![Span::styled(
            truncate(&format!(" {desc}"), w as usize),
            fg(app.pal.muted),
        )],
    );
    put(
        f,
        x,
        area.y + 2,
        vec![Span::styled("─".repeat(w as usize), fg(app.pal.muted))],
    );

    let body = Rect::new(x, area.y + 3, w, area.height.saturating_sub(3 + BUTTONS_H));
    match app.section {
        Section::Widgets if app.widgets.editing.is_none() => draw_layout(f, app, body, focused),
        Section::Plugins => draw_plugins(f, app, body, focused),
        _ => draw_form(f, app, body, focused),
    }

    let sep_y = area.bottom() - BUTTONS_H;
    put(
        f,
        x,
        sep_y,
        vec![Span::styled("─".repeat(w as usize), fg(app.pal.muted))],
    );
    draw_buttons(f, app, Rect::new(x, sep_y + 1, w, 3));
}

fn section_header(app: &App) -> (String, String) {
    if app.section == Section::Widgets
        && let Some((s, i)) = app.widgets.editing
    {
        let entry = sj::section(&app.store.json, s).get(i);
        let id = entry.map(sj::entry_id).unwrap_or_default();
        let name = curated::widget_name(&id, entry, &app.store.catalog);
        let desc = app
            .store
            .catalog
            .get(&id)
            .map(curated::widget_description)
            .unwrap_or_else(|| t("w.custom_desc"));
        return (format!("{} › {name}", app.section.title()), desc);
    }
    (app.section.title(), app.section.description())
}

// ---------------------------------------------------------------- relieve 3D

/// Caja en relieve de 3 líneas, como los controles de Meca:
/// `┌───┐` / `┃ x │` / `┗━━━┙`. El borde izquierdo e inferior hacen de
/// sombra (acento al estar activo).
#[allow(clippy::too_many_arguments)]
pub(super) fn bevel_box(
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
struct Tone {
    border: Color,
    shadow: Color,
    bg: Option<Color>,
    bold: bool,
}

fn tone(app: &App, selected: bool, hover: bool) -> Tone {
    if hover {
        Tone {
            border: app.pal.accent,
            shadow: app.pal.accent,
            bg: Some(app.pal.soft_hover),
            bold: true,
        }
    } else if selected {
        Tone {
            border: app.pal.bright,
            shadow: app.pal.accent,
            bg: Some(app.pal.soft_selection),
            bold: true,
        }
    } else {
        Tone {
            border: app.pal.fg,
            shadow: app.pal.muted,
            bg: None,
            bold: false,
        }
    }
}

fn inner_style(app: &App, tn: &Tone) -> Style {
    let mut s = Style::new().fg(app.pal.bright);
    if let Some(bg) = tn.bg {
        s = s.bg(bg);
    }
    if tn.bold {
        s = s.add_modifier(Modifier::BOLD);
    }
    s
}

// ---------------------------------------------------------------- botones

fn draw_buttons(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Buttons && app.popup.is_none();
    let labels: Vec<String> = Button::ALL.iter().map(|b| b.label()).collect();
    let gap = 2u16;
    let total: u16 =
        labels.iter().map(|l| l.width() as u16 + 2).sum::<u16>() + gap * (labels.len() as u16 - 1);
    let start = area.right().saturating_sub(total + 1);

    // A la izquierda: estado de los cambios pendientes.
    let pending = app.store.changes().len() + app.store.ops.len();
    let (status, color) = if pending > 0 {
        (
            format!("● {}", tf("btn.pending", &[("n", &pending.to_string())])),
            app.pal.warn,
        )
    } else {
        (format!("✓ {}", t("btn.clean")), app.pal.muted)
    };
    let status_w = start.saturating_sub(area.x + 1) as usize;
    if status_w > 6 {
        put(
            f,
            area.x,
            area.y + 1,
            vec![Span::styled(
                truncate(&format!(" {status}"), status_w),
                fg(color),
            )],
        );
    }

    let mut x = start;
    for (i, (b, label)) in Button::ALL.iter().zip(labels).enumerate() {
        let inner_w = label.width();
        let hit = Hit::Button(i);
        let hover = hovered(app, hit);
        let selected = focused && app.button == i;
        let enabled = app.button_enabled(*b);
        let primary = *b == Button::Apply && enabled;
        let (border, shadow, text) = if !enabled {
            (app.pal.dim, app.pal.dim, fg(app.pal.dim))
        } else if hover {
            (
                app.pal.accent,
                app.pal.accent,
                Style::new()
                    .fg(app.pal.bright)
                    .bg(app.pal.soft_hover)
                    .add_modifier(Modifier::BOLD),
            )
        } else if selected {
            (
                app.pal.bright,
                app.pal.accent,
                Style::new()
                    .fg(app.pal.bright)
                    .bg(app.pal.soft_selection)
                    .add_modifier(Modifier::BOLD),
            )
        } else if primary {
            (
                app.pal.bright,
                app.pal.accent,
                fg(app.pal.bright).add_modifier(Modifier::BOLD),
            )
        } else {
            (app.pal.fg, app.pal.muted, fg(app.pal.fg))
        };
        bevel_box(
            f,
            x,
            area.y,
            vec![Span::styled(label, text)],
            inner_w,
            border,
            shadow,
            selected || primary || hover,
        );
        app.hits
            .push((Rect::new(x, area.y, inner_w as u16 + 2, 3), hit));
        x += inner_w as u16 + 2 + gap;
    }
}

// ---------------------------------------------------------------- formularios

/// Tipo de control que se dibuja a la derecha de una fila.
enum Ctl {
    Toggle(bool),
    Stepper(String),
    Slider { frac: f64, text: String },
    Select(String),
    Edit(String),
    Color(Option<Color>, String),
    Run(String),
}

fn control_for(row: &Row) -> Option<Ctl> {
    match row {
        Row::Action(..) => Some(Ctl::Run(t("ctl.run"))),
        Row::Field(fr) => Some(field_control(fr)),
        _ => None,
    }
}

fn field_control(fr: &FieldRow) -> Ctl {
    let def = &fr.def;
    let v = fr.effective();
    match &def.kind {
        Kind::Bool => Ctl::Toggle(v.and_then(Value::as_bool).unwrap_or(false)),
        Kind::Enum(_) | Kind::Multi(_) => Ctl::Select(form::display_value(fr)),
        _ if !def.presets.is_empty() => {
            let text = match v {
                Some(Value::String(s)) if def.date_format => qt_format::preview(s),
                _ => form::display_value(fr),
            };
            Ctl::Select(text)
        }
        Kind::Float {
            min: Some(min),
            max: Some(max),
            ..
        } => {
            let n = v.or(def.example.as_ref()).and_then(Value::as_f64);
            let frac = n
                .map(|n| ((n - min) / (max - min).max(1e-9)).clamp(0.0, 1.0))
                .unwrap_or(0.0);
            Ctl::Slider {
                frac,
                text: n.map(|n| format!("{n:.2}")).unwrap_or_else(|| "—".into()),
            }
        }
        Kind::Int { .. } | Kind::Float { .. } => Ctl::Stepper(match v {
            Some(n) => value_label(n),
            None => def
                .example
                .as_ref()
                .map(|e| format!("({})", value_label(e)))
                .unwrap_or_else(|| "—".into()),
        }),
        Kind::Color => {
            let s = v.and_then(Value::as_str).unwrap_or("").to_string();
            Ctl::Color(hex(&s), if s.is_empty() { "—".into() } else { s })
        }
        _ => Ctl::Edit(match v {
            None => def
                .example
                .as_ref()
                .map(|e| format!("({})", value_label(e)))
                .unwrap_or_else(|| "—".into()),
            Some(Value::String(s)) if s.is_empty() => format!("({})", t("val.empty")),
            Some(Value::String(s)) => schema::escape_newlines(s),
            Some(other) => other.to_string(),
        }),
    }
}

/// Ancho que ocupa el control (sin contar el margen).
fn ctl_width(c: &Ctl, cap: usize) -> usize {
    let boxed = |text: &str, extra: usize| text.width().min(cap) + extra + 2;
    match c {
        Ctl::Toggle(_) => 5 + 1 + t("val.on").width().max(t("val.off").width()) + 1,
        Ctl::Stepper(_) => 17,
        Ctl::Slider { .. } => 17,
        Ctl::Select(s) | Ctl::Edit(s) => boxed(s, 4),
        Ctl::Color(_, s) => boxed(s, 7),
        Ctl::Run(s) => boxed(s, 4),
    }
}

fn row_height(row: &Row, width: usize) -> usize {
    match row {
        Row::Note(text, _) => wrap(text, width.saturating_sub(4)).len(),
        Row::Header(_) => 2,
        _ => 3,
    }
}

fn row_hovered(app: &App, i: usize) -> bool {
    app.popup.is_none() && matches!(app.hover, Some(Hit::Row(r)) | Some(Hit::Ctrl(r, _)) if r == i)
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
    let width = area.width as usize;
    let heights: Vec<usize> = rows.iter().map(|r| row_height(r, width)).collect();
    if st.offset > sel {
        st.offset = sel;
    }
    while st.offset < sel && heights[st.offset..=sel].iter().sum::<usize>() > area.height as usize {
        st.offset += 1;
    }
    // Si justo encima hay un encabezado, muéstralo también.
    if st.offset > 0
        && st.offset == sel
        && matches!(rows.get(sel - 1), Some(Row::Header(_)))
        && heights[sel - 1..=sel].iter().sum::<usize>() <= area.height as usize
    {
        st.offset -= 1;
    }
    let offset = st.offset;

    let mut y = area.y;
    for (i, row) in rows.iter().enumerate().skip(offset) {
        let h = heights[i] as u16;
        if y + h > area.bottom() {
            // Indicador de que hay más contenido debajo.
            put(
                f,
                area.right().saturating_sub(3),
                area.bottom().saturating_sub(1),
                vec![Span::styled(" ▾ ", fg(app.pal.accent))],
            );
            break;
        }
        match row {
            Row::Header(text) => {
                let title = format!(" ━━ {text} ");
                let rest = width.saturating_sub(title.width() + 1);
                put(
                    f,
                    area.x,
                    y,
                    vec![Span::styled(
                        format!("{title}{}", "━".repeat(rest)),
                        fg(app.pal.accent).add_modifier(Modifier::BOLD),
                    )],
                );
            }
            Row::Note(text, kind) => {
                let (icon, color) = match kind {
                    NoteKind::Info => ("", app.pal.muted),
                    NoteKind::Warn => ("", app.pal.warn),
                };
                for (n, l) in wrap(text, width.saturating_sub(4)).into_iter().enumerate() {
                    put(
                        f,
                        area.x,
                        y + n as u16,
                        vec![
                            Span::styled(
                                if n == 0 {
                                    format!(" {icon} ")
                                } else {
                                    "   ".into()
                                },
                                fg(color),
                            ),
                            Span::styled(l, fg(color)),
                        ],
                    );
                }
            }
            Row::Field(_) | Row::Action(..) => {
                draw_item(f, app, area, y, i, row, i == sel && focused, i == sel)
            }
        }
        y += h;
    }
}

/// Fila de 3 líneas: nombre y descripción a la izquierda, control a la derecha.
#[allow(clippy::too_many_arguments)]
fn draw_item(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    y: u16,
    i: usize,
    row: &Row,
    focused_sel: bool,
    is_cursor: bool,
) {
    let width = area.width as usize;
    let hover_row = row_hovered(app, i);
    let lit = focused_sel || hover_row;
    let Some(ctl) = control_for(row) else { return };
    let cap = (width / 2).clamp(10, 34);
    let cw = ctl_width(&ctl, cap);
    let left_w = width.saturating_sub(cw + 3).max(12);

    let (name, desc, changed) = match row {
        Row::Field(fr) => (
            fr.def.label.clone(),
            fr.def.desc.clone(),
            app.store.get_original(&fr.bind) != fr.value,
        ),
        Row::Action(label, _) => (label.clone(), String::new(), false),
        _ => return,
    };
    let desc_lines = wrap(&desc, left_w.saturating_sub(6));

    // Línea 1: nombre (con ● si tiene un cambio sin aplicar).
    let mark = if lit { " ▌ " } else { "   " };
    let dot = if changed { " ●" } else { "" };
    let name_w = left_w.saturating_sub(mark.width() + dot.width());
    let name_style = if lit {
        Style::new()
            .fg(app.pal.bright)
            .bg(app.pal.soft_selection)
            .add_modifier(Modifier::BOLD)
    } else {
        fg(app.pal.bright).add_modifier(Modifier::BOLD)
    };
    let mut l1 = vec![
        Span::styled(mark, name_style.fg(app.pal.accent)),
        Span::styled(pad(&name, name_w), name_style),
    ];
    if changed {
        l1.push(Span::styled(dot, name_style.fg(app.pal.warn)));
    }
    put(f, area.x, y, l1);

    // Línea 2: descripción (└─ cuando está activa, como en Meca).
    let first = desc_lines.first().cloned().unwrap_or_default();
    let l2 = if lit {
        vec![
            Span::styled(" ▌ ", fg(app.pal.accent)),
            Span::styled(
                pad(&format!("└─ {first}"), left_w.saturating_sub(3)),
                fg(app.pal.fg),
            ),
        ]
    } else {
        vec![Span::styled(
            pad(&format!("   {first}"), left_w),
            fg(app.pal.muted),
        )]
    };
    put(f, area.x, y + 1, l2);

    // Línea 3: solo en la fila del cursor, información extra.
    if is_cursor {
        let extra = match row {
            Row::Field(fr) if fr.def.date_format => fr
                .effective()
                .and_then(Value::as_str)
                .map(|s| format!("{}: {}", t("ctl.format"), schema::escape_newlines(s))),
            Row::Field(fr) if app.advanced => Some(format!(
                "{}: {} · {}: {}",
                t("help.key"),
                match &fr.bind {
                    Bind::Json(p) => sj::path_to_string(p),
                    Bind::Toml(s, k) => format!("{s}.{k}"),
                },
                t("help.default"),
                fr.def
                    .default
                    .as_ref()
                    .map(value_label)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "—".into())
            )),
            _ => desc_lines.get(1).cloned(),
        };
        if let Some(extra) = extra {
            put(
                f,
                area.x,
                y + 2,
                vec![Span::styled(
                    pad(&format!("      {extra}"), left_w),
                    fg(app.pal.muted),
                )],
            );
        }
    }
    app.hits
        .push((Rect::new(area.x, y, left_w as u16 + 1, 3), Hit::Row(i)));

    // Control a la derecha.
    let cx = area.x + left_w as u16 + 1;
    draw_control(f, app, cx, y, i, &ctl, cap, lit);
}

#[allow(clippy::too_many_arguments)]
fn draw_control(
    f: &mut Frame,
    app: &mut App,
    x: u16,
    y: u16,
    i: usize,
    ctl: &Ctl,
    cap: usize,
    lit: bool,
) {
    let hov = |app: &App, s: Sub| hovered(app, Hit::Ctrl(i, s));
    match ctl {
        Ctl::Toggle(on) => {
            let h = hov(app, Sub::Main);
            let tn = tone(app, lit, h);
            let mark_bg = tn.bg.or(if *on {
                Some(app.pal.soft_selection)
            } else {
                None
            });
            let mut mark = Style::new().fg(if *on || h {
                app.pal.bright
            } else {
                app.pal.muted
            });
            if let Some(bg) = mark_bg {
                mark = mark.bg(bg);
            }
            if *on || h {
                mark = mark.add_modifier(Modifier::BOLD);
            }
            bevel_box(
                f,
                x,
                y,
                vec![Span::styled(if *on { " ■ " } else { "   " }, mark)],
                3,
                tn.border,
                tn.shadow,
                tn.bold,
            );
            let label = if *on { t("val.on") } else { t("val.off") };
            put(
                f,
                x + 5,
                y + 1,
                vec![Span::styled(
                    format!(" {label}"),
                    if *on || h {
                        fg(app.pal.bright).add_modifier(Modifier::BOLD)
                    } else {
                        fg(app.pal.muted)
                    },
                )],
            );
            let w = 6 + label.width() as u16;
            app.hits
                .push((Rect::new(x, y, w, 3), Hit::Ctrl(i, Sub::Main)));
        }
        Ctl::Stepper(text) => {
            for (dx, s, sym) in [(0u16, Sub::Minus, " - "), (12u16, Sub::Plus, " + ")] {
                let h = hov(app, s);
                let tn = tone(app, lit, h);
                bevel_box(
                    f,
                    x + dx,
                    y,
                    vec![Span::styled(sym, inner_style(app, &tn))],
                    3,
                    tn.border,
                    tn.shadow,
                    tn.bold,
                );
                app.hits.push((Rect::new(x + dx, y, 5, 3), Hit::Ctrl(i, s)));
            }
            let h = hov(app, Sub::Main);
            let mut st =
                fg(if h { app.pal.accent } else { app.pal.bright }).add_modifier(Modifier::BOLD);
            if h {
                st = st.bg(app.pal.soft_hover);
            }
            put(
                f,
                x + 5,
                y + 1,
                vec![Span::styled(center(&truncate(text, 7), 7), st)],
            );
            app.hits
                .push((Rect::new(x + 5, y, 7, 3), Hit::Ctrl(i, Sub::Main)));
        }
        Ctl::Slider { frac, text } => {
            let h = hov(app, Sub::Slider);
            let pos = (frac * 10.0).round() as usize;
            let track = format!("{}●{}", "─".repeat(pos), "─".repeat(10 - pos));
            let st = if h {
                fg(app.pal.accent).add_modifier(Modifier::BOLD)
            } else if lit {
                fg(app.pal.bright).add_modifier(Modifier::BOLD)
            } else {
                fg(app.pal.fg)
            };
            let hv = hov(app, Sub::Main);
            let mut vst = fg(app.pal.bright).add_modifier(Modifier::BOLD);
            if hv {
                vst = vst.fg(app.pal.accent).bg(app.pal.soft_hover);
            }
            put(
                f,
                x,
                y + 1,
                vec![
                    Span::styled(track, st),
                    Span::styled(format!(" {}", right_align(text, 5)), vst),
                ],
            );
            app.hits
                .push((Rect::new(x, y, 11, 3), Hit::Ctrl(i, Sub::Slider)));
            app.hits
                .push((Rect::new(x + 11, y, 6, 3), Hit::Ctrl(i, Sub::Main)));
        }
        Ctl::Select(text) | Ctl::Edit(text) | Ctl::Run(text) => {
            let h = hov(app, Sub::Main)
                || (matches!(ctl, Ctl::Select(_))
                    && matches!(&app.popup, Some(super::popup::Popup::Picker(p)) if p.anchor.is_some())
                    && is_cursor_row(app, i));
            let tn = tone(app, lit, h);
            let suffix = match ctl {
                Ctl::Select(_) => " ▾ ",
                Ctl::Edit(_) => " ✎ ",
                _ => " ▸ ",
            };
            let body = format!(" {}{suffix}", truncate(text, cap));
            let inner_w = body.width();
            bevel_box(
                f,
                x,
                y,
                vec![Span::styled(body, inner_style(app, &tn))],
                inner_w,
                tn.border,
                tn.shadow,
                tn.bold,
            );
            app.hits.push((
                Rect::new(x, y, inner_w as u16 + 2, 3),
                Hit::Ctrl(i, Sub::Main),
            ));
        }
        Ctl::Color(c, text) => {
            let h = hov(app, Sub::Main);
            let tn = tone(app, lit, h);
            let st = inner_style(app, &tn);
            let mut swatch = Style::new().fg(c.unwrap_or(app.pal.dim));
            if let Some(bg) = tn.bg {
                swatch = swatch.bg(bg);
            }
            let label = format!(" {} ✎ ", truncate(text, cap));
            let inner_w = 3 + label.width();
            bevel_box(
                f,
                x,
                y,
                vec![
                    Span::styled(" ", st),
                    Span::styled("██", swatch),
                    Span::styled(label, st),
                ],
                inner_w,
                tn.border,
                tn.shadow,
                tn.bold,
            );
            app.hits.push((
                Rect::new(x, y, inner_w as u16 + 2, 3),
                Hit::Ctrl(i, Sub::Main),
            ));
        }
    }
}

fn is_cursor_row(app: &App, i: usize) -> bool {
    let st = if app.section == Section::Widgets {
        &app.widgets.form
    } else {
        &app.forms[app.section.index()]
    };
    st.sel == i
}

// ---------------------------------------------------------------- widgets

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
    if area.height < 8 {
        return;
    }
    let w = area.width as usize;

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
    let inner = w.saturating_sub(4);
    let third = inner / 3;
    let bar_line = format!(
        "{}{}{}",
        pad(&truncate(&l, third), third),
        center(&truncate(&c, third), third),
        right_align(&truncate(&r, inner - 2 * third), inner - 2 * third)
    );
    put(
        f,
        area.x,
        area.y,
        vec![Span::styled(
            format!(" 󰍹 {} ({})", t("lay.preview"), t(&format!("pos.{pos}"))),
            fg(app.pal.muted),
        )],
    );
    put(
        f,
        area.x,
        area.y + 1,
        vec![
            Span::styled(" ▕", fg(app.pal.muted)),
            Span::styled(
                bar_line,
                Style::new().fg(app.pal.bright).bg(app.pal.surface),
            ),
            Span::styled("▏", fg(app.pal.muted)),
        ],
    );

    let detail_h = 3u16;
    let cols_area = Rect::new(
        area.x,
        area.y + 3,
        area.width,
        area.height.saturating_sub(3 + detail_h + 1),
    );
    let col_w = cols_area.width / 3;
    for s in 0..3 {
        let a = Rect::new(
            cols_area.x + s as u16 * col_w,
            cols_area.y,
            if s == 2 {
                cols_area.width - 2 * col_w
            } else {
                col_w
            },
            cols_area.height,
        );
        let active = focused && app.widgets.col == s;
        let col_hover = app.popup.is_none()
            && matches!(app.hover, Some(Hit::Column(c)) | Some(Hit::Widget(c, _)) | Some(Hit::AddWidget(c)) if c == s);
        let drop_target = app.widgets.drag.is_some()
            && matches!(app.widgets.drag_over, Some(Hit::Column(c)) | Some(Hit::Widget(c, _)) if c == s);
        let entries = sj::section(&app.store.json, s).to_vec();
        let border = if active || drop_target {
            app.pal.accent
        } else if col_hover {
            app.pal.bright
        } else {
            app.pal.muted
        };
        let block = Block::new()
            .borders(Borders::ALL)
            .border_type(if active || drop_target {
                BorderType::Thick
            } else {
                BorderType::Plain
            })
            .border_style(fg(border))
            .title(Span::styled(
                format!(
                    " {} ({}) ",
                    t(&format!("col.{}", sj::SECTIONS[s])),
                    entries.len()
                ),
                fg(if active {
                    app.pal.accent
                } else {
                    app.pal.bright
                })
                .add_modifier(Modifier::BOLD),
            ));
        let inner = block.inner(a);
        f.render_widget(block, a);
        app.hits.push((a, Hit::Column(s)));
        let iw = inner.width as usize;

        // Deja una fila para "＋ Añadir".
        let visible = (inner.height as usize).saturating_sub(1);
        let sel = app.widgets.row[s].min(entries.len().saturating_sub(1));
        let offset = sel.saturating_sub(visible.saturating_sub(1));
        let mut y = inner.y;
        for (i, e) in entries.iter().enumerate().skip(offset).take(visible) {
            let hit = Hit::Widget(s, i);
            let hover = hovered(app, hit);
            let is_sel = i == sel && app.widgets.col == s;
            let dragging = app.widgets.drag == Some((s, i));
            let over = app.widgets.drag.is_some() && app.widgets.drag_over == Some(hit);
            let mut text = widget_label(app, e);
            if app.advanced {
                let id = sj::entry_id(e);
                if !text.contains(&id) {
                    text = format!("{text}  {id}");
                }
            }
            let marker = if is_sel && focused {
                "▌"
            } else if hover {
                "▸"
            } else {
                " "
            };
            let style = if over {
                fg(app.pal.accent).add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
            } else if dragging {
                fg(app.pal.muted)
            } else if hover {
                Style::new()
                    .fg(app.pal.bright)
                    .bg(app.pal.soft_hover)
                    .add_modifier(Modifier::BOLD)
            } else if is_sel && focused {
                Style::new()
                    .fg(app.pal.bright)
                    .bg(app.pal.soft_selection)
                    .add_modifier(Modifier::BOLD)
            } else if is_sel {
                Style::new().fg(app.pal.bright).bg(app.pal.soft_muted)
            } else {
                fg(app.pal.fg)
            };
            put(
                f,
                inner.x,
                y,
                vec![
                    Span::styled(marker, fg(app.pal.accent)),
                    Span::styled(pad(&format!(" {text}"), iw.saturating_sub(1)), style),
                ],
            );
            app.hits.push((Rect::new(inner.x, y, inner.width, 1), hit));
            y += 1;
        }
        // Botón para añadir al final de la columna.
        if y < inner.bottom() {
            let hit = Hit::AddWidget(s);
            let hover = hovered(app, hit);
            let style = if hover {
                Style::new()
                    .fg(app.pal.bright)
                    .bg(app.pal.soft_hover)
                    .add_modifier(Modifier::BOLD)
            } else {
                fg(app.pal.muted)
            };
            put(
                f,
                inner.x,
                y,
                vec![Span::styled(
                    pad(
                        &format!("{} 󰐕 {}", if hover { "▸" } else { " " }, t("lay.add")),
                        iw,
                    ),
                    style,
                )],
            );
            app.hits.push((Rect::new(inner.x, y, inner.width, 1), hit));
        }
    }

    // Detalle del widget seleccionado.
    let dy = area.bottom() - detail_h;
    let col = app.widgets.col;
    let entries = sj::section(&app.store.json, col);
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
        put(
            f,
            area.x,
            dy,
            vec![
                Span::styled(
                    format!(" {}", widget_label(app, e)),
                    fg(app.pal.bright).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    truncate(
                        &format!("  {desc}"),
                        w.saturating_sub(widget_label(app, e).width() + 2),
                    ),
                    fg(app.pal.muted),
                ),
            ],
        );
        let settings: Vec<String> = sj::entry_settings(e)
            .iter()
            .map(|(k, v)| {
                format!(
                    "{k}={}",
                    match v {
                        Value::String(s) => schema::escape_newlines(s),
                        other => other.to_string(),
                    }
                )
            })
            .collect();
        if !settings.is_empty() {
            put(
                f,
                area.x,
                dy + 1,
                vec![Span::styled(
                    truncate(&format!(" {}", settings.join("  ")), w),
                    fg(app.pal.muted),
                )],
            );
        }
    }
    put(
        f,
        area.x,
        dy + 2,
        vec![Span::styled(
            truncate(&format!(" {}", t("lay.tip")), w),
            fg(app.pal.dim),
        )],
    );
}

// ---------------------------------------------------------------- plugins

fn draw_plugins(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    app.clamp_plugin_sel(true);
    let rows = app.plugin_rows();
    let w = area.width as usize;
    let detail_h = 4u16;

    // Buscador (clicable).
    let hit = Hit::Search;
    let hover = hovered(app, hit);
    let searching = app.plugins.filtering || !app.plugins.filter.is_empty();
    let text = if searching {
        format!(
            " 󰍉 {}{}",
            app.plugins.filter,
            if app.plugins.filtering { "▏" } else { "" }
        )
    } else {
        format!(" 󰍉 {}", t("pl.filter_hint"))
    };
    let style = if hover {
        Style::new().fg(app.pal.bright).bg(app.pal.soft_hover)
    } else if app.plugins.filtering {
        Style::new().fg(app.pal.bright).bg(app.pal.soft_selection)
    } else {
        fg(app.pal.muted)
    };
    put(
        f,
        area.x,
        area.y,
        vec![Span::styled(pad(&text, w.min(48)), style)],
    );
    app.hits
        .push((Rect::new(area.x, area.y, (w.min(48)) as u16, 1), hit));

    let list = Rect::new(
        area.x,
        area.y + 2,
        area.width,
        area.height.saturating_sub(2 + detail_h + 1),
    );
    let sel = app.plugins.sel.min(rows.len().saturating_sub(1));
    let h = list.height as usize;
    if app.plugins.offset > sel {
        app.plugins.offset = sel.saturating_sub(1);
    }
    if h > 0 && sel >= app.plugins.offset + h {
        app.plugins.offset = sel + 1 - h;
    }
    let offset = app.plugins.offset;
    for (i, row) in rows.iter().enumerate().skip(offset).take(h) {
        let y = list.y + (i - offset) as u16;
        match row {
            PluginRow::Header(title) => {
                let head = format!(" ━━ {title} ");
                put(
                    f,
                    list.x,
                    y,
                    vec![Span::styled(
                        format!("{head}{}", "━".repeat(w.saturating_sub(head.width() + 1))),
                        fg(app.pal.accent).add_modifier(Modifier::BOLD),
                    )],
                );
            }
            PluginRow::Item(id) => {
                let Some(p) = app.store.catalog.get(id).cloned() else {
                    continue;
                };
                let on = app.store.plugin_enabled(&p);
                let changed =
                    on != p.enabled_in(app.store.json_original()) || app.store.queued(id).is_some();
                let row_hit = Hit::Plugin(i);
                let sw_hit = Hit::PluginSwitch(i);
                let hover_row = hovered(app, row_hit) || hovered(app, sw_hit);
                let hover_sw = hovered(app, sw_hit);
                let is_sel = i == sel && focused;
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
                let bg = if is_sel {
                    Some(app.pal.soft_selection)
                } else if hover_row {
                    Some(app.pal.soft_muted)
                } else {
                    None
                };
                let with_bg = |s: Style| match bg {
                    Some(b) => s.bg(b),
                    None => s,
                };
                let switch = if on {
                    format!(" ■ {} ", t("val.on"))
                } else {
                    format!(" □ {} ", t("val.off"))
                };
                let sw_w = switch.width().max(7);
                let right = format!("{state} · {origin}  ");
                let mut label = name;
                if app.advanced {
                    label = format!("{label}  {id}");
                }
                if changed {
                    label.push_str(" ●");
                }
                let left_w = w.saturating_sub(4 + right.width() + sw_w + 1);
                let marker = if is_sel {
                    "▌"
                } else if hover_row {
                    "▸"
                } else {
                    " "
                };
                let mut spans = vec![
                    Span::styled(marker, fg(app.pal.accent)),
                    Span::styled(
                        if on { " ● " } else { " ○ " },
                        with_bg(fg(if on { app.pal.accent } else { app.pal.dim })),
                    ),
                    Span::styled(
                        pad(&label, left_w),
                        with_bg(
                            fg(if changed {
                                app.pal.warn
                            } else {
                                app.pal.bright
                            })
                            .add_modifier(if is_sel || hover_row {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                        ),
                    ),
                    Span::styled(right, with_bg(fg(app.pal.muted))),
                ];
                let sw_style = if hover_sw {
                    Style::new()
                        .fg(app.pal.bright)
                        .bg(app.pal.soft_hover)
                        .add_modifier(Modifier::BOLD)
                } else if on {
                    Style::new()
                        .fg(app.pal.bright)
                        .bg(app.pal.soft_selection)
                        .add_modifier(Modifier::BOLD)
                } else {
                    fg(app.pal.muted)
                };
                let used: usize = spans.iter().map(|s| s.content.width()).sum();
                spans.push(Span::styled(pad(&switch, sw_w), sw_style));
                put(f, list.x, y, spans);
                app.hits
                    .push((Rect::new(list.x, y, used as u16, 1), row_hit));
                app.hits
                    .push((Rect::new(list.x + used as u16, y, sw_w as u16, 1), sw_hit));
            }
        }
    }

    // Detalle del plugin seleccionado.
    let dy = area.bottom() - detail_h;
    put(
        f,
        area.x,
        dy - 1,
        vec![Span::styled("╌".repeat(w), fg(app.pal.dim))],
    );
    if let Some(p) = app.selected_plugin() {
        let mut head = vec![Span::styled(
            format!(" {}", curated::widget_name(&p.id, None, &app.store.catalog)),
            fg(app.pal.bright).add_modifier(Modifier::BOLD),
        )];
        if app.advanced {
            head.push(Span::styled(
                format!(
                    "  {} · {}",
                    p.kinds.join(", "),
                    if p.version.is_empty() {
                        "-".into()
                    } else {
                        format!("v{}", p.version)
                    }
                ),
                fg(app.pal.muted),
            ));
        }
        put(f, area.x, dy, head);
        let desc = curated::widget_description(&p);
        let mut y = dy + 1;
        for l in wrap(&desc, w.saturating_sub(2)).into_iter().take(2) {
            put(
                f,
                area.x,
                y,
                vec![Span::styled(format!(" {l}"), fg(app.pal.fg))],
            );
            y += 1;
        }
        let extra = if !p.first_party {
            Some((t("pl.warn_unsandboxed"), app.pal.warn))
        } else if app.advanced {
            Some((
                format!(
                    "{}  ·  {}{}",
                    p.id,
                    p.source_dir.display(),
                    if p.author.is_empty() {
                        String::new()
                    } else {
                        format!("  ·  {}", p.author)
                    }
                ),
                app.pal.dim,
            ))
        } else {
            None
        };
        if let Some((text, color)) = extra
            && y < area.bottom()
        {
            put(
                f,
                area.x,
                y,
                vec![Span::styled(truncate(&format!(" {text}"), w), fg(color))],
            );
        }
    }
}
