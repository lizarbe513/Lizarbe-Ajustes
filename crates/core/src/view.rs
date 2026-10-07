//! Piezas de interfaz compartidas por las aplicaciones: contexto de dibujo
//! (paleta, ratón y zonas clicables), cabecera, formularios con controles
//! planos, botones de texto `[A] Aplicar` y barra lateral.
//!
//! Estilo: minimalismo editorial. Sin bloques rellenos: el estado se marca con
//! `▍`, con el tono del texto y con el subrayado; un solo acento; líneas finas.

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
    button_spans, button_width, center, fg, put, right_align, rule_h, spaced, truncate, wrap,
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
    fn close() -> Self;
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

// ---------------------------------------------------------------- cabecera

/// Cabecera de una línea, sin relleno: el nombre de la aplicación espaciado a
/// la izquierda; a la derecha lo secundario (`parts`, atenuado), los cambios
/// pendientes (acento) y el botón `[Q] Cerrar`.
#[allow(clippy::too_many_arguments)]
pub fn draw_header<H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    name: &str,
    subtitle: &str,
    parts: &[String],
    pending: Option<&str>,
    close: (&str, &str),
) {
    let pal = ctx.pal;
    let w = area.width as usize;
    let close_w = button_width(close.0, close.1);
    let mut right_w = close_w + 2;
    let parts_text = parts.join("  ·  ");
    let pend_w = pending.map_or(0, |p| p.width() + 5);
    let mut show_parts = true;
    let mut show_pending = pending.is_some();
    let left = spaced(name);
    let sub = if subtitle.is_empty() {
        String::new()
    } else {
        format!("   ·   {subtitle}")
    };
    // Lo que no cabe se quita por prioridad: subtítulo, ajustes, cambios.
    let need = |parts_on: bool, pend_on: bool| {
        2 + left.width()
            + right_w
            + if parts_on { parts_text.width() + 5 } else { 0 }
            + if pend_on { pend_w } else { 0 }
    };
    let mut show_sub = need(true, true) + sub.width() <= w;
    if !show_sub && need(true, true) > w {
        show_parts = false;
        if need(false, true) > w {
            show_pending = false;
        }
    }
    if !show_parts && !show_pending && 2 + left.width() + right_w > w {
        right_w = 0;
        show_sub = false;
    }
    let mut spans = vec![Span::styled(
        left.clone(),
        fg(pal.accent).add_modifier(Modifier::BOLD),
    )];
    if show_sub {
        spans.push(Span::styled(sub, fg(pal.muted)));
    }
    put(f, area.x + 2, area.y, spans);

    let mut x = area.right().saturating_sub(right_w as u16);
    if right_w > 0 {
        let hit = H::close();
        let lit = ctx.hovered(hit);
        let spans = button_spans(pal, close.0, close.1, true, false, lit);
        put(f, x, area.y, spans);
        ctx.hit(Rect::new(x, area.y, close_w as u16, 1), hit);
    }
    if show_pending && let Some(p) = pending {
        let pw = (p.width() + 5) as u16;
        x = x.saturating_sub(pw);
        put(
            f,
            x,
            area.y,
            vec![Span::styled(
                format!("• {p}"),
                fg(pal.accent).add_modifier(Modifier::BOLD),
            )],
        );
    }
    if show_parts {
        let pw = (parts_text.width() + 5) as u16;
        x = x.saturating_sub(pw);
        put(f, x, area.y, vec![Span::styled(parts_text, fg(pal.muted))]);
    }
}

/// Título de sección en mayúsculas y su descripción (hasta dos líneas), más una
/// línea en blanco. Devuelve cuántas líneas ocupa en total.
pub fn section_header(
    f: &mut Frame,
    pal: &Palette,
    area: Rect,
    title: &str,
    desc: &str,
    focused: bool,
) -> u16 {
    let w = area.width as usize;
    put(
        f,
        area.x,
        area.y,
        vec![
            Span::styled(
                if focused { "▍" } else { " " },
                fg(pal.accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", truncate(&title.to_uppercase(), w.saturating_sub(2))),
                fg(pal.bright).add_modifier(Modifier::BOLD),
            ),
        ],
    );
    let mut lines = wrap(desc, w.saturating_sub(4));
    if lines.len() > 2 {
        lines.truncate(2);
        if let Some(last) = lines.last_mut() {
            *last = truncate(&format!("{last}…"), w.saturating_sub(4));
        }
    }
    for (n, l) in lines.iter().enumerate() {
        put(
            f,
            area.x + 2,
            area.y + 1 + n as u16,
            vec![Span::styled(l.clone(), fg(pal.muted))],
        );
    }
    1 + lines.len() as u16 + 1
}

// ---------------------------------------------------------------- botones

/// Botón de la fila inferior: `[tecla] etiqueta`.
pub struct ButtonSpec {
    pub label: String,
    /// Tecla que lo activa, como se muestra: "A", "⏎"…
    pub key: String,
    pub enabled: bool,
    pub primary: bool,
}

/// Fila de botones de texto a la izquierda, con `status` a la derecha.
/// `selected` es el botón con el foco del teclado. Ocupa la primera línea
/// de `area` (la línea fina de encima la dibuja la aplicación con `rule_h`).
pub fn draw_buttons<H: CoreHit>(
    f: &mut Frame,
    ctx: &mut Ctx<H>,
    area: Rect,
    buttons: &[ButtonSpec],
    selected: Option<usize>,
    status: (&str, Color),
) {
    let gap = 3u16;
    let mut x = area.x + 2;
    let mut used = 2u16;
    for (i, b) in buttons.iter().enumerate() {
        let w = button_width(&b.key, &b.label) as u16;
        let hit = H::button(i);
        let lit = b.enabled && (ctx.hovered(hit) || selected == Some(i));
        put(
            f,
            x,
            area.y,
            button_spans(
                ctx.pal,
                &b.key,
                &b.label,
                b.enabled,
                b.primary && b.enabled,
                lit,
            ),
        );
        ctx.hit(Rect::new(x, area.y, w, 1), hit);
        x += w + gap;
        used += w + gap;
    }
    let room = (area.width.saturating_sub(used + 2)) as usize;
    if room > 6 && !status.0.is_empty() {
        let text = truncate(status.0, room);
        let tw = text.width() as u16;
        put(
            f,
            area.right().saturating_sub(tw + 2),
            area.y,
            vec![Span::styled(text, fg(status.1))],
        );
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

/// Barra lateral por categorías, sin caja: el encabezado de cada categoría en
/// mayúsculas atenuadas y la entrada activa con `▍`. Las entradas se numeran
/// en orden de aparición para `H::sidebar(i)`. `active`: el foco está en la barra.
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
    let mut y = area.y + 1;
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
            area.x + 2,
            y,
            vec![Span::styled(
                truncate(&cat.to_uppercase(), w.saturating_sub(3)),
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
            let style = if hover {
                Style::new()
                    .fg(pal.bright)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if it.selected {
                Style::new().fg(pal.bright).add_modifier(Modifier::BOLD)
            } else {
                fg(pal.fg)
            };
            let marker = if it.selected {
                Span::styled(
                    "▍",
                    fg(if active { pal.accent } else { pal.muted }).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw(" ")
            };
            put(
                f,
                area.x + 1,
                y,
                vec![
                    marker,
                    Span::raw("  "),
                    Span::styled(truncate(&it.title, w.saturating_sub(5)), style),
                ],
            );
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
    rule_h(
        f,
        area.x + 2,
        bottom - n - 1,
        (w as u16).saturating_sub(4),
        pal.rule,
    );
    for (k, tg) in toggles.iter().enumerate() {
        let y = bottom - n + k as u16;
        let hover = ctx.hovered(tg.hit);
        let value = format!("{} ⇄", tg.value);
        let room = w.saturating_sub(4);
        let label = if tg.label.width() + 2 + value.width() <= room {
            tg.label.clone()
        } else {
            String::new()
        };
        let gap = room.saturating_sub(label.width() + value.width());
        let mut label_style = fg(pal.muted);
        let mut val_style = fg(pal.bright).add_modifier(Modifier::BOLD);
        if hover {
            label_style = label_style
                .fg(pal.bright)
                .add_modifier(Modifier::UNDERLINED);
            val_style = fg(pal.accent).add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        put(
            f,
            area.x + 2,
            y,
            vec![
                Span::styled(label, label_style),
                Span::raw(" ".repeat(gap)),
                Span::styled(value, val_style),
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
    match c {
        Ctl::Toggle(_) => 3 + 1 + t("val.on").width().max(t("val.off").width()),
        Ctl::Stepper(_) => 13,
        Ctl::Slider { .. } => 17,
        Ctl::Select(s) | Ctl::Edit(s) | Ctl::Run(s) => s.width().min(cap) + 2,
        Ctl::Color(_, s) => 3 + s.width().min(cap),
    }
}

/// Líneas de descripción de una fila (como máximo tres).
fn desc_lines<B>(fr: &FieldRow<B>, width: usize) -> Vec<String> {
    let mut lines = wrap(&fr.def.desc, width.saturating_sub(6));
    lines.truncate(3);
    lines
}

fn row_height<B, A>(row: &Row<B, A>, width: usize) -> usize {
    match row {
        Row::Note(text, _) => wrap(text, width.saturating_sub(4)).len(),
        Row::Header(_) => 2,
        // Nombre, descripción (una o más líneas) y una línea de aire.
        Row::Field(fr) => 2 + desc_lines(fr, width).len().max(1),
        Row::Action(..) => 3,
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
                vec![Span::styled(" ▾ ", fg(pal.muted))],
            );
            break;
        }
        match row {
            Row::Header(text) => {
                let title = text.to_uppercase();
                let line = (width.saturating_sub(title.width() + 6)).min(6);
                put(
                    f,
                    area.x + 2,
                    y,
                    vec![
                        Span::styled(title, fg(pal.muted).add_modifier(Modifier::BOLD)),
                        Span::styled(format!(" {}", "─".repeat(line)), fg(pal.rule)),
                    ],
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

/// Fila de 3 líneas: nombre (con el control a la derecha) y descripción
/// debajo; la tercera queda en blanco o muestra el detalle de la fila elegida.
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
    let cap = (width / 2).clamp(10, 30);
    let cw = ctl_width(&ctl, cap);
    // Margen derecho de 2; el control se alinea a la derecha.
    let cx = area.right().saturating_sub(cw as u16 + 2).max(area.x + 14);
    let left_w = (cx.saturating_sub(area.x) as usize)
        .saturating_sub(3)
        .max(10);

    let name = match row {
        Row::Field(fr) => fr.def.label.clone(),
        Row::Action(label, _) => label.clone(),
        _ => return,
    };
    let changed = info.changed;
    let desc_lines = match row {
        Row::Field(fr) => desc_lines(fr, width),
        _ => vec![],
    };

    // Línea 1: marca, nombre y, si tiene un cambio sin aplicar, un punto.
    let name_style = if lit {
        Style::new().fg(pal.bright).add_modifier(Modifier::BOLD)
    } else {
        fg(pal.fg)
    };
    let dot_w = if changed { 2 } else { 0 };
    let mut l1 = vec![
        Span::styled(
            if focused_sel { "▍" } else { " " },
            fg(pal.accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            truncate(&name, left_w.saturating_sub(2 + dot_w)),
            name_style,
        ),
    ];
    if changed {
        l1.push(Span::styled(
            " •",
            fg(pal.accent).add_modifier(Modifier::BOLD),
        ));
    }
    put(f, area.x, y, l1);

    // Descripción atenuada, en su propia línea (usa todo el ancho).
    for (n, l) in desc_lines.iter().enumerate() {
        put(
            f,
            area.x + 2,
            y + 1 + n as u16,
            vec![Span::styled(
                truncate(l, width.saturating_sub(4)),
                fg(pal.muted),
            )],
        );
    }
    // En la fila del cursor, el detalle del modo avanzado ocupa la línea de aire.
    if is_cursor
        && desc_lines.len() <= 1
        && let Some(extra) = info.extra.clone()
    {
        put(
            f,
            area.x + 2,
            y + 1 + desc_lines.len().max(1) as u16,
            vec![Span::styled(
                truncate(&extra, width.saturating_sub(4)),
                fg(pal.dim),
            )],
        );
    }
    ctx.hit(
        Rect::new(
            area.x,
            y,
            left_w as u16 + 2,
            1 + desc_lines.len().max(1) as u16,
        ),
        H::row(i),
    );

    draw_control(f, ctx, cx, y, i, &ctl, cap, lit, dropdown);
}

/// Estilo del valor de un control: acento si la fila está activa, tono
/// normal si no, y subrayado con el ratón encima.
fn value_style(pal: &Palette, lit: bool, hover: bool) -> Style {
    if hover {
        Style::new()
            .fg(pal.bright)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else if lit {
        fg(pal.accent).add_modifier(Modifier::BOLD)
    } else {
        fg(pal.fg)
    }
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
            let (knob, style) = if *on {
                (
                    "━━●",
                    if h || lit {
                        value_style(pal, lit, h)
                    } else {
                        fg(pal.bright).add_modifier(Modifier::BOLD)
                    },
                )
            } else {
                (
                    "○──",
                    if h {
                        value_style(pal, lit, true)
                    } else {
                        fg(pal.muted)
                    },
                )
            };
            let label = if *on { t("val.on") } else { t("val.off") };
            let label_style = if *on && (lit || h) {
                value_style(pal, lit, h)
            } else if *on {
                fg(pal.bright)
            } else {
                fg(pal.muted)
            };
            put(
                f,
                x,
                y,
                vec![
                    Span::styled(knob, style),
                    Span::styled(format!(" {label}"), label_style),
                ],
            );
            let w = 3 + 1 + t("val.on").width().max(t("val.off").width()) as u16;
            ctx.hit(Rect::new(x, y, w, 2), H::ctrl(i, Sub::Main));
        }
        Ctl::Stepper(text) => {
            let arrow = |ctx: &Ctx<H>, s: Sub| {
                if ctx.hovered(H::ctrl(i, s)) {
                    fg(pal.bright).add_modifier(Modifier::BOLD)
                } else if lit {
                    fg(pal.accent).add_modifier(Modifier::BOLD)
                } else {
                    fg(pal.muted)
                }
            };
            let hm = hov(ctx, Sub::Main);
            put(
                f,
                x,
                y,
                vec![
                    Span::raw(" "),
                    Span::styled("‹", arrow(ctx, Sub::Minus)),
                    Span::styled(center(&truncate(text, 9), 9), value_style(pal, lit, hm)),
                    Span::styled("›", arrow(ctx, Sub::Plus)),
                ],
            );
            ctx.hit(Rect::new(x, y, 3, 2), H::ctrl(i, Sub::Minus));
            ctx.hit(Rect::new(x + 10, y, 3, 2), H::ctrl(i, Sub::Plus));
            ctx.hit(Rect::new(x + 3, y, 7, 2), H::ctrl(i, Sub::Main));
        }
        Ctl::Slider { frac, text } => {
            let h = hov(ctx, Sub::Slider);
            let pos = (frac * 10.0).round() as usize;
            let on = if h || lit {
                fg(pal.accent).add_modifier(Modifier::BOLD)
            } else {
                fg(pal.fg)
            };
            let hv = hov(ctx, Sub::Main);
            put(
                f,
                x,
                y,
                vec![
                    Span::styled("━".repeat(pos), on),
                    Span::styled("●", on),
                    Span::styled("─".repeat(10 - pos), fg(pal.rule)),
                    Span::styled(
                        format!(" {}", right_align(text, 5)),
                        value_style(pal, lit, hv),
                    ),
                ],
            );
            ctx.hit(Rect::new(x, y, 11, 2), H::ctrl(i, Sub::Slider));
            ctx.hit(Rect::new(x + 11, y, 6, 2), H::ctrl(i, Sub::Main));
        }
        Ctl::Select(text) | Ctl::Edit(text) | Ctl::Run(text) => {
            let h = hov(ctx, Sub::Main) || (matches!(ctl, Ctl::Select(_)) && dropdown);
            let suffix = match ctl {
                Ctl::Select(_) => " ▾",
                Ctl::Edit(_) => " ✎",
                _ => " →",
            };
            let shown = truncate(text, cap);
            let w = shown.width() as u16 + 2;
            let hint = if lit || h {
                fg(pal.accent)
            } else {
                fg(pal.muted)
            };
            put(
                f,
                x,
                y,
                vec![
                    Span::styled(shown, value_style(pal, lit, h)),
                    Span::styled(suffix, hint),
                ],
            );
            ctx.hit(Rect::new(x, y, w, 2), H::ctrl(i, Sub::Main));
        }
        Ctl::Color(c, text) => {
            let h = hov(ctx, Sub::Main);
            let shown = truncate(text, cap);
            let w = 3 + shown.width() as u16;
            put(
                f,
                x,
                y,
                vec![
                    Span::styled("██", fg(c.unwrap_or(pal.dim))),
                    Span::raw(" "),
                    Span::styled(shown, value_style(pal, lit, h)),
                ],
            );
            ctx.hit(Rect::new(x, y, w, 2), H::ctrl(i, Sub::Main));
        }
    }
}
