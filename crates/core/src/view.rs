//! Piezas de interfaz compartidas por las aplicaciones: contexto de dibujo
//! (paleta, ratón y zonas clicables), formularios con controles en relieve,
//! fila de botones Restaurar · Cancelar · Aplicar y barra lateral.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use serde_json::Value;
use unicode_width::UnicodeWidthStr;

use crate::form::{FieldRow, FormState, NoteKind, Row};
use crate::i18n::t;
use crate::schema::{self, Kind, value_label};
use crate::theme::{Palette, hex};
use crate::ui::{
    bevel_box, button_look, center, fg, inner_style, pad, put, right_align, tone, truncate, wrap,
};

/// Parte de un control sobre la que está el ratón.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sub {
    Main,
    Minus,
    Plus,
    Slider,
}

/// Zonas clicables que dibujan las piezas compartidas. Cada aplicación las
/// mapea a variantes de su propio tipo de zona.
pub trait CoreHit: Copy + PartialEq {
    fn row(i: usize) -> Self;
    fn ctrl(i: usize, sub: Sub) -> Self;
    fn button(i: usize) -> Self;
    fn sidebar(i: usize) -> Self;
    fn mode_toggle() -> Self;
    fn lang_toggle() -> Self;
    fn popup_item(i: usize) -> Self;
    fn modal_button(i: usize) -> Self;
    fn menu_item(i: usize) -> Self;
    /// Si es una fila o un control de la fila `i` de un formulario.
    fn row_index(&self) -> Option<usize>;
}

/// Lo que necesita una pieza para dibujarse y registrar dónde se puede hacer clic.
pub struct Ctx<'a, H> {
    pub pal: &'a Palette,
    /// Zona bajo el ratón.
    pub hover: Option<H>,
    pub hits: &'a mut Vec<(Rect, H)>,
    /// Hay una ventana emergente encima: lo de debajo no reacciona al ratón.
    pub blocked: bool,
}

impl<H: Copy + PartialEq> Ctx<'_, H> {
    /// ¿Está el ratón sobre `hit` (y no hay ventana encima)?
    pub fn hovered(&self, hit: H) -> bool {
        !self.blocked && self.hover == Some(hit)
    }

    /// ¿Está el ratón sobre `hit`, haya o no ventana encima?
    pub fn hover_is(&self, hit: H) -> bool {
        self.hover == Some(hit)
    }

    pub fn hit(&mut self, r: Rect, hit: H) {
        self.hits.push((r, hit));
    }
}

// ---------------------------------------------------------------- encabezado

/// Título de sección en mayúsculas, descripción y separador (3 líneas).
pub fn section_header(
    f: &mut Frame,
    pal: &Palette,
    area: Rect,
    title: &str,
    desc: &str,
    focused: bool,
) {
    let x = area.x;
    let w = area.width as usize;
    put(
        f,
        x,
        area.y,
        vec![
            Span::styled(
                if focused { "▍" } else { " " },
                fg(pal.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                truncate(&title.to_uppercase(), w.saturating_sub(1)),
                fg(pal.bright).add_modifier(Modifier::BOLD),
            ),
        ],
    );
    put(
        f,
        x,
        area.y + 1,
        vec![Span::styled(
            truncate(&format!(" {desc}"), w),
            fg(pal.muted),
        )],
    );
    put(
        f,
        x,
        area.y + 2,
        vec![Span::styled("─".repeat(w), fg(pal.muted))],
    );
}

// ---------------------------------------------------------------- botones

/// Botón de la fila inferior.
pub struct ButtonSpec {
    pub label: String,
    pub enabled: bool,
    pub primary: bool,
}

/// Fila de botones en relieve alineados a la derecha, con `status` a la
/// izquierda. `selected` es el botón con el foco del teclado.
pub fn draw_buttons<H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    buttons: &[ButtonSpec],
    selected: Option<usize>,
    status: (&str, Color),
) {
    let gap = 2u16;
    let total: u16 = buttons
        .iter()
        .map(|b| b.label.width() as u16 + 2)
        .sum::<u16>()
        + gap * (buttons.len() as u16).saturating_sub(1);
    let start = area.right().saturating_sub(total + 1);

    let status_w = start.saturating_sub(area.x + 1) as usize;
    if status_w > 6 {
        put(
            f,
            area.x,
            area.y + 1,
            vec![Span::styled(
                truncate(&format!(" {}", status.0), status_w),
                fg(status.1),
            )],
        );
    }

    let mut x = start;
    for (i, b) in buttons.iter().enumerate() {
        let inner_w = b.label.width();
        let hit = H::button(i);
        let hover = ctx.hovered(hit);
        let sel = selected == Some(i);
        let primary = b.primary && b.enabled;
        let (border, shadow, text) = button_look(ctx.pal, b.enabled, hover, sel, primary);
        bevel_box(
            f,
            x,
            area.y,
            vec![Span::styled(b.label.clone(), text)],
            inner_w,
            border,
            shadow,
            sel || primary || hover,
        );
        ctx.hit(Rect::new(x, area.y, inner_w as u16 + 2, 3), hit);
        x += inner_w as u16 + 2 + gap;
    }
}

// ---------------------------------------------------------------- sidebar

/// Entrada de la barra lateral.
pub struct SideItem {
    pub icon: &'static str,
    pub title: String,
    pub selected: bool,
}

/// Interruptor al pie de la barra lateral (modo, idioma).
pub struct SideToggle<H> {
    pub hit: H,
    pub icon: &'static str,
    pub label: String,
    pub value: String,
}

/// Barra lateral por categorías. Las entradas se numeran en orden de
/// aparición para `H::sidebar(i)`. `active`: el foco está en la barra.
pub fn draw_sidebar<H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    categories: &[(String, Vec<SideItem>)],
    toggles: &[SideToggle<H>],
    active: bool,
) {
    let w = area.width as usize;
    let pal = ctx.pal;
    let mut y = area.y;
    let mut index = 0;
    for (n, (cat, items)) in categories.iter().enumerate() {
        if n > 0 {
            y += 1;
        }
        if y >= area.bottom() {
            break;
        }
        put(
            f,
            area.x,
            y,
            vec![Span::styled(
                pad(&format!(" {}", cat.to_uppercase()), w),
                fg(pal.muted).add_modifier(Modifier::BOLD),
            )],
        );
        y += 1;
        for it in items {
            if y >= area.bottom() {
                break;
            }
            let hit = H::sidebar(index);
            index += 1;
            let hover = ctx.hovered(hit);
            let prefix = if hover { " ▸" } else { "  " };
            let text = pad(&format!("{prefix}{}  {}", it.icon, it.title), w);
            let strong = |bg: Color| {
                Style::new()
                    .fg(pal.bright)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD)
            };
            let style = if it.selected && (active || hover) {
                strong(pal.soft_selection)
            } else if it.selected || hover {
                strong(pal.soft_muted)
            } else {
                fg(pal.fg)
            };
            put(f, area.x, y, vec![Span::styled(text, style)]);
            ctx.hit(Rect::new(area.x, y, area.width, 1), hit);
            y += 1;
        }
    }

    // Interruptores al pie, también clicables.
    let bottom = area.bottom();
    let n = toggles.len() as u16;
    if n == 0 || bottom < y + n + 2 {
        return;
    }
    put(
        f,
        area.x,
        bottom - n - 1,
        vec![Span::styled(
            format!(" {}", "─".repeat(w.saturating_sub(2))),
            fg(pal.muted),
        )],
    );
    for (k, tg) in toggles.iter().enumerate() {
        let y = bottom - n + k as u16;
        let hover = ctx.hovered(tg.hit);
        let lead = if hover { " ▸" } else { "  " };
        let right = format!("{} ⇄ ", tg.value);
        // Sin espacio suficiente solo se muestra el icono.
        let left = if format!("{lead}{}  {} {right}", tg.icon, tg.label).width() <= w {
            format!("{lead}{}  {}", tg.icon, tg.label)
        } else {
            format!("{lead}{}", tg.icon)
        };
        let style = if hover {
            Style::new()
                .fg(pal.bright)
                .bg(pal.soft_hover)
                .add_modifier(Modifier::BOLD)
        } else {
            fg(pal.fg)
        };
        let val_style = if hover {
            style
        } else {
            fg(pal.accent).add_modifier(Modifier::BOLD)
        };
        let gap = w.saturating_sub(left.width() + right.width());
        put(
            f,
            area.x,
            y,
            vec![
                Span::styled(format!("{left}{}", " ".repeat(gap)), style),
                Span::styled(right, val_style),
            ],
        );
        ctx.hit(Rect::new(area.x, y, area.width, 1), tg.hit);
    }
}

// ---------------------------------------------------------------- formularios

/// Datos extra de una fila de campo que solo conoce la aplicación.
#[derive(Default)]
pub struct RowInfo {
    /// Tiene un cambio sin aplicar (se marca con ●).
    pub changed: bool,
    /// Tercera línea en la fila del cursor (clave, valor por defecto…).
    pub extra: Option<String>,
}

/// Opciones de dibujo de un formulario.
pub struct FormOpts {
    /// El foco del teclado está en el formulario.
    pub focused: bool,
    /// Hay un desplegable abierto pegado al control de la fila del cursor.
    pub dropdown_open: bool,
}

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

fn control_for<B, A>(row: &Row<B, A>) -> Option<Ctl> {
    match row {
        Row::Action(..) => Some(Ctl::Run(t("ctl.run"))),
        Row::Field(fr) => Some(field_control(fr)),
        _ => None,
    }
}

fn field_control<B>(fr: &FieldRow<B>) -> Ctl {
    let def = &fr.def;
    let v = fr.effective();
    match &def.kind {
        Kind::Bool => Ctl::Toggle(v.and_then(Value::as_bool).unwrap_or(false)),
        Kind::Enum(_) | Kind::Multi(_) => Ctl::Select(crate::form::display_value(fr)),
        _ if !def.presets.is_empty() => {
            let text = match (v, def.preview) {
                (Some(Value::String(s)), Some(preview)) => preview.show(s),
                _ => crate::form::display_value(fr),
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
            Some(Value::Null) => "—".into(),
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

fn row_height<B, A>(row: &Row<B, A>, width: usize) -> usize {
    match row {
        Row::Note(text, _) => wrap(text, width.saturating_sub(4)).len(),
        Row::Header(_) => 2,
        _ => 3,
    }
}

fn row_hovered<H: CoreHit>(ctx: &Ctx<H>, i: usize) -> bool {
    !ctx.blocked && ctx.hover.and_then(|h| h.row_index()) == Some(i)
}

/// Formulario con desplazamiento: encabezados, notas y filas de 3 líneas
/// con el control a la derecha. `info` aporta lo que solo sabe la aplicación.
pub fn draw_form<H: CoreHit, B, A>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    rows: &[Row<B, A>],
    st: &mut FormState,
    opts: &FormOpts,
    info: impl Fn(&FieldRow<B>) -> RowInfo,
) {
    st.clamp(rows);
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
    let pal = ctx.pal;

    let mut y = area.y;
    for (i, row) in rows.iter().enumerate().skip(offset) {
        let h = heights[i] as u16;
        if y + h > area.bottom() {
            // Indicador de que hay más contenido debajo.
            put(
                f,
                area.right().saturating_sub(3),
                area.bottom().saturating_sub(1),
                vec![Span::styled(" ▾ ", fg(pal.accent))],
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
                        fg(pal.accent).add_modifier(Modifier::BOLD),
                    )],
                );
            }
            Row::Note(text, kind) => {
                let (icon, color) = match kind {
                    NoteKind::Info => ("", pal.muted),
                    NoteKind::Warn => ("", pal.warn),
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
                let ri = match row {
                    Row::Field(fr) => info(fr),
                    _ => RowInfo::default(),
                };
                let dropdown = opts.dropdown_open && i == sel;
                draw_item(
                    f,
                    ctx,
                    area,
                    y,
                    i,
                    row,
                    i == sel && opts.focused,
                    i == sel,
                    &ri,
                    dropdown,
                )
            }
        }
        y += h;
    }
}

/// Fila de 3 líneas: nombre y descripción a la izquierda, control a la derecha.
#[allow(clippy::too_many_arguments)]
fn draw_item<H: CoreHit, B, A>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    y: u16,
    i: usize,
    row: &Row<B, A>,
    focused_sel: bool,
    is_cursor: bool,
    info: &RowInfo,
    dropdown: bool,
) {
    let pal = ctx.pal;
    let width = area.width as usize;
    let hover_row = row_hovered(ctx, i);
    let lit = focused_sel || hover_row;
    let Some(ctl) = control_for(row) else { return };
    let cap = (width / 2).clamp(10, 34);
    let cw = ctl_width(&ctl, cap);
    let left_w = width.saturating_sub(cw + 3).max(12);

    let (name, desc) = match row {
        Row::Field(fr) => (fr.def.label.clone(), fr.def.desc.clone()),
        Row::Action(label, _) => (label.clone(), String::new()),
        _ => return,
    };
    let changed = info.changed;
    let desc_lines = wrap(&desc, left_w.saturating_sub(6));

    // Línea 1: nombre (con ● si tiene un cambio sin aplicar).
    let mark = if lit { " ▌ " } else { "   " };
    let dot = if changed { " ●" } else { "" };
    let name_w = left_w.saturating_sub(mark.width() + dot.width());
    let name_style = if lit {
        Style::new()
            .fg(pal.bright)
            .bg(pal.soft_selection)
            .add_modifier(Modifier::BOLD)
    } else {
        fg(pal.bright).add_modifier(Modifier::BOLD)
    };
    let mut l1 = vec![
        Span::styled(mark, name_style.fg(pal.accent)),
        Span::styled(pad(&name, name_w), name_style),
    ];
    if changed {
        l1.push(Span::styled(dot, name_style.fg(pal.warn)));
    }
    put(f, area.x, y, l1);

    // Línea 2: descripción (└─ cuando está activa, como en Meca).
    let first = desc_lines.first().cloned().unwrap_or_default();
    let l2 = if lit {
        vec![
            Span::styled(" ▌ ", fg(pal.accent)),
            Span::styled(
                pad(&format!("└─ {first}"), left_w.saturating_sub(3)),
                fg(pal.fg),
            ),
        ]
    } else {
        vec![Span::styled(
            pad(&format!("   {first}"), left_w),
            fg(pal.muted),
        )]
    };
    put(f, area.x, y + 1, l2);

    // Línea 3: solo en la fila del cursor, información extra.
    if is_cursor && let Some(extra) = info.extra.clone().or_else(|| desc_lines.get(1).cloned()) {
        put(
            f,
            area.x,
            y + 2,
            vec![Span::styled(
                pad(&format!("      {extra}"), left_w),
                fg(pal.muted),
            )],
        );
    }
    ctx.hit(Rect::new(area.x, y, left_w as u16 + 1, 3), H::row(i));

    // Control a la derecha.
    let cx = area.x + left_w as u16 + 1;
    draw_control(f, ctx, cx, y, i, &ctl, cap, lit, dropdown);
}

#[allow(clippy::too_many_arguments)]
fn draw_control<H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    x: u16,
    y: u16,
    i: usize,
    ctl: &Ctl,
    cap: usize,
    lit: bool,
    dropdown: bool,
) {
    let pal = ctx.pal;
    let hov = |ctx: &Ctx<H>, s: Sub| ctx.hovered(H::ctrl(i, s));
    match ctl {
        Ctl::Toggle(on) => {
            let h = hov(ctx, Sub::Main);
            let tn = tone(pal, lit, h);
            let mark_bg = tn.bg.or(if *on { Some(pal.soft_selection) } else { None });
            let mut mark = Style::new().fg(if *on || h { pal.bright } else { pal.muted });
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
                        fg(pal.bright).add_modifier(Modifier::BOLD)
                    } else {
                        fg(pal.muted)
                    },
                )],
            );
            let w = 6 + label.width() as u16;
            ctx.hit(Rect::new(x, y, w, 3), H::ctrl(i, Sub::Main));
        }
        Ctl::Stepper(text) => {
            for (dx, s, sym) in [(0u16, Sub::Minus, " - "), (12u16, Sub::Plus, " + ")] {
                let h = hov(ctx, s);
                let tn = tone(pal, lit, h);
                bevel_box(
                    f,
                    x + dx,
                    y,
                    vec![Span::styled(sym, inner_style(pal, &tn))],
                    3,
                    tn.border,
                    tn.shadow,
                    tn.bold,
                );
                ctx.hit(Rect::new(x + dx, y, 5, 3), H::ctrl(i, s));
            }
            let h = hov(ctx, Sub::Main);
            let mut st = fg(if h { pal.accent } else { pal.bright }).add_modifier(Modifier::BOLD);
            if h {
                st = st.bg(pal.soft_hover);
            }
            put(
                f,
                x + 5,
                y + 1,
                vec![Span::styled(center(&truncate(text, 7), 7), st)],
            );
            ctx.hit(Rect::new(x + 5, y, 7, 3), H::ctrl(i, Sub::Main));
        }
        Ctl::Slider { frac, text } => {
            let h = hov(ctx, Sub::Slider);
            let pos = (frac * 10.0).round() as usize;
            let track = format!("{}●{}", "─".repeat(pos), "─".repeat(10 - pos));
            let st = if h {
                fg(pal.accent).add_modifier(Modifier::BOLD)
            } else if lit {
                fg(pal.bright).add_modifier(Modifier::BOLD)
            } else {
                fg(pal.fg)
            };
            let hv = hov(ctx, Sub::Main);
            let mut vst = fg(pal.bright).add_modifier(Modifier::BOLD);
            if hv {
                vst = vst.fg(pal.accent).bg(pal.soft_hover);
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
            ctx.hit(Rect::new(x, y, 11, 3), H::ctrl(i, Sub::Slider));
            ctx.hit(Rect::new(x + 11, y, 6, 3), H::ctrl(i, Sub::Main));
        }
        Ctl::Select(text) | Ctl::Edit(text) | Ctl::Run(text) => {
            let h = hov(ctx, Sub::Main) || (matches!(ctl, Ctl::Select(_)) && dropdown);
            let tn = tone(pal, lit, h);
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
                vec![Span::styled(body, inner_style(pal, &tn))],
                inner_w,
                tn.border,
                tn.shadow,
                tn.bold,
            );
            ctx.hit(
                Rect::new(x, y, inner_w as u16 + 2, 3),
                H::ctrl(i, Sub::Main),
            );
        }
        Ctl::Color(c, text) => {
            let h = hov(ctx, Sub::Main);
            let tn = tone(pal, lit, h);
            let st = inner_style(pal, &tn);
            let mut swatch = Style::new().fg(c.unwrap_or(pal.dim));
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
            ctx.hit(
                Rect::new(x, y, inner_w as u16 + 2, 3),
                H::ctrl(i, Sub::Main),
            );
        }
    }
}
