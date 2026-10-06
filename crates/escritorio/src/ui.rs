//! Dibujo de Escritorio con las piezas compartidas del núcleo: barra de
//! título, barra lateral, formularios, botones y barra de estado.

use lizarbe_core::form::NoteKind;
use lizarbe_core::ui::{Hint, fg, header_bar, put, status_bar};
use lizarbe_core::view::{self, ButtonSpec, Ctx, FormOpts, RowInfo, SideItem, SideToggle};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Span;

use crate::app::{App, Button, FieldRow, Focus, Hit, Popup, Section};
use crate::catalog::CATEGORIES;
use crate::i18n::{self, t, tf};

/// Alto de la zona de botones: separador + botones de 3 líneas.
const BUTTONS_H: u16 = 4;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = f.area();
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, app, header);
    let side_w = if area.width < 95 { 22 } else { 26 };
    let [side, sep, content] = Layout::horizontal([
        Constraint::Length(side_w),
        Constraint::Length(1),
        Constraint::Min(20),
    ])
    .areas(body);
    draw_sidebar(f, app, side);
    for y in sep.y..sep.bottom() {
        put(f, sep.x, y, vec![Span::styled("│", fg(app.pal.muted))]);
    }
    draw_content(f, app, content);
    draw_footer(f, app, footer);

    if let Some(popup) = app.popup.as_mut() {
        let mut ctx = Ctx {
            pal: &app.pal,
            hover: app.hover,
            hits: &mut app.hits,
            blocked: false,
        };
        lizarbe_core::modal::draw(f, &mut ctx, area, popup);
    }
}

fn ctx(app: &mut App) -> Ctx<'_, Hit> {
    Ctx {
        pal: &app.pal,
        hover: app.hover,
        hits: &mut app.hits,
        blocked: app.popup.is_some(),
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let name = t("app.name").to_uppercase();
    let left = format!("   {name}  ·  {} ", t("app.subtitle"));
    let left_short = format!("   {name} ");
    let mut parts: Vec<String> = vec![];
    if app.store.dirty() {
        parts.push(format!(
            "*{}*",
            tf("hdr.pending", &[("n", &app.pending().to_string())]).to_uppercase()
        ));
    }
    if app.store.paths.sandbox {
        parts.push("sandbox".into());
    } else if !app.store.live {
        parts.push(t("hdr.offline"));
    }
    parts.push(if app.advanced {
        t("hdr.advanced")
    } else {
        t("hdr.simple")
    });
    parts.push(i18n::lang().code().to_uppercase());
    parts.push(format!("q: {}", t("ft.quit")));
    header_bar(f, &app.pal, area, &left, &left_short, &parts);
}

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Focus::Sidebar && app.popup.is_none();
    let pending = app.pending();
    let categories: Vec<(String, Vec<SideItem>)> = CATEGORIES
        .iter()
        .map(|(cat, sections)| {
            let items = sections
                .iter()
                .map(|s| {
                    let mut title = s.title();
                    if *s == Section::Changes && pending > 0 {
                        title.push_str(&format!(" ({pending})"));
                    }
                    SideItem {
                        icon: s.icon(),
                        title,
                        selected: *s == app.section,
                    }
                })
                .collect();
            (t(cat), items)
        })
        .collect();
    let toggles = [
        SideToggle {
            hit: Hit::ModeToggle,
            icon: "󰘵",
            label: t("side.mode"),
            value: if app.advanced {
                t("hdr.advanced")
            } else {
                t("hdr.simple")
            },
        },
        SideToggle {
            hit: Hit::LangToggle,
            icon: "󰗊",
            label: t("side.lang"),
            value: if i18n::lang() == i18n::Lang::Es {
                "Español".to_string()
            } else {
                "English".to_string()
            },
        },
    ];
    view::draw_sidebar(f, &mut ctx(app), area, &categories, &toggles, active);
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Content && app.popup.is_none();
    let x = area.x + 1;
    let w = area.width.saturating_sub(2);
    if area.height < 10 || w < 30 {
        return;
    }
    view::section_header(
        f,
        &app.pal,
        Rect::new(x, area.y, w, 3),
        &app.section.title(),
        &app.section.description(),
        focused,
    );

    let body = Rect::new(x, area.y + 3, w, area.height.saturating_sub(3 + BUTTONS_H));
    draw_form(f, app, body, focused);

    let sep_y = area.bottom() - BUTTONS_H;
    put(
        f,
        x,
        sep_y,
        vec![Span::styled("─".repeat(w as usize), fg(app.pal.muted))],
    );
    draw_buttons(f, app, Rect::new(x, sep_y + 1, w, 3));
}

fn draw_form(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let rows = app.rows();
    let opts = FormOpts {
        focused,
        dropdown_open: matches!(&app.popup, Some(Popup::Picker(p)) if p.anchor.is_some()),
    };
    let advanced = app.advanced;
    let App {
        store,
        pal,
        hover,
        hits,
        popup,
        forms,
        section,
        ..
    } = app;
    let st = &mut forms[section.index()];
    let mut ctx = Ctx {
        pal,
        hover: *hover,
        hits,
        blocked: popup.is_some(),
    };
    let info = |fr: &FieldRow| RowInfo {
        changed: lizarbe_core::form::FieldStore::get_original(store, &fr.bind) != fr.value,
        extra: advanced.then(|| {
            format!(
                "{}: {} · {}: {}",
                t("help.key"),
                fr.def.key,
                t("help.default"),
                fr.def
                    .default
                    .as_ref()
                    .map(lizarbe_core::schema::value_label)
                    .unwrap_or_else(|| "—".into())
            )
        }),
    };
    view::draw_form(f, &mut ctx, area, &rows, st, &opts, info);
}

fn draw_buttons(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Buttons && app.popup.is_none();
    let buttons: Vec<ButtonSpec> = Button::ALL
        .iter()
        .map(|b| ButtonSpec {
            label: b.label(),
            enabled: app.button_enabled(*b),
            primary: *b == Button::Apply,
        })
        .collect();
    let pending = app.pending();
    let (status, color) = if pending > 0 {
        (
            format!("● {}", tf("btn.pending", &[("n", &pending.to_string())])),
            app.pal.warn,
        )
    } else {
        (format!("✓ {}", t("btn.clean")), app.pal.muted)
    };
    let selected = focused.then_some(app.button);
    view::draw_buttons(f, &mut ctx(app), area, &buttons, selected, (&status, color));
}

fn footer_hints(app: &App) -> Vec<Hint> {
    let k = |a: &str, b: &str, p: u8| (a.to_string(), t(b), p);
    if let Some(p) = &app.popup {
        return match p {
            Popup::Confirm { .. } => vec![k("Enter", "ft.yes", 3), k("Esc", "ft.no", 3)],
            Popup::Message { .. } => vec![k("Enter", "ft.close", 3)],
            Popup::Help { .. } => vec![k("↑↓", "ft.scroll", 2), k("Esc", "ft.close", 3)],
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
            Popup::Menu(_) => vec![
                k("↑↓", "ft.move", 2),
                k("Enter", "ft.choose", 3),
                k("Esc", "ft.close", 3),
            ],
        };
    }
    let mut v = match app.focus {
        Focus::Sidebar => vec![k("↑↓", "ft.section", 1), k("Enter", "ft.open", 2)],
        Focus::Buttons => vec![k("←→", "ft.choose_button", 2), k("Enter", "ft.press", 3)],
        Focus::Content => vec![
            k("←→", "ft.change", 2),
            k("Enter", "ft.edit", 2),
            k("r", "ft.reset", 1),
        ],
    };
    v.push(k("o", "ft.menu", 1));
    v.push(k("Tab", "ft.next_area", 0));
    v.push(k("?", "ft.help", 3));
    v
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let status = match &app.toast {
        Some(toast) => {
            let icon = match toast.kind {
                NoteKind::Info => "",
                NoteKind::Warn => "",
            };
            format!(" {icon} {}", toast.text)
        }
        None => app
            .hover_hint()
            .map(|h| format!(" 󰳽 {h}"))
            .unwrap_or_default(),
    };
    status_bar(
        f,
        &app.pal,
        area,
        &status,
        app.toast.is_some(),
        &footer_hints(app),
    );
}
