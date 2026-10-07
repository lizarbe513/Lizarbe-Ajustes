//! Dibujo del estudio: cabecera, pestañas numeradas, editor a la izquierda y
//! maqueta en vivo a la derecha, botones de texto y barra de pistas.

use lizarbe_core::form::{FieldStore, NoteKind, Row};
use lizarbe_core::ui::{
    Hint, button_spans, button_width, fg, put, rule_h, rule_v, status_bar, truncate, wrap,
};
use lizarbe_core::view::{self, ButtonSpec, Ctx, FormOpts, RowInfo};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Button, Focus, Hit, Popup, Screen, Tab};
use crate::i18n::{self, t, tf};
use crate::mockup;

/// Alto de la zona de botones: la línea fina, los botones y una línea de aire.
const BUTTONS_H: u16 = 3;
/// Ancho mínimo de la ventana para mostrar la maqueta al lado del editor.
const WIDE: u16 = 110;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = f.area();
    let [header, _gap, tabs, _gap2, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(if app.screen == Screen::Edit { 2 } else { 0 }),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, app, header);
    match app.screen {
        Screen::Home => draw_home(f, app, body),
        Screen::Edit => {
            draw_tabs(f, app, tabs);
            draw_editor(f, app, body);
        }
    }
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

fn draw_header(f: &mut Frame, app: &mut App, area: Rect) {
    let name = t("app.name");
    let subtitle = match (&app.screen, &app.draft) {
        (Screen::Edit, Some(d)) => {
            let n = if d.name.trim().is_empty() {
                t("new.untitled")
            } else {
                d.name.clone()
            };
            n.to_string()
        }
        _ => t("app.subtitle"),
    };
    let mut parts: Vec<String> = vec![];
    if app.sandbox {
        parts.push("sandbox".into());
    }
    parts.push(i18n::lang().code().to_uppercase());
    let pending = (app.dirty()).then(|| t("status.unsaved"));
    let close = t("btn.close");
    let back_label = t("btn.back");
    let search =
        (app.screen == Screen::Edit && app.popup.is_none()).then(|| ("/", t("btn.search")));
    let back =
        (app.screen == Screen::Edit && app.popup.is_none()).then_some(("Esc", back_label.as_str()));
    view::draw_header(
        f,
        &mut ctx(app),
        area,
        &name,
        &subtitle,
        &parts,
        pending.as_deref(),
        ("Q", &close),
        back,
        search.as_ref().map(|(k, l)| (*k, l.as_str())),
    );
}

// ---------------------------------------------------------------- inicio

fn draw_home(f: &mut Frame, app: &mut App, area: Rect) {
    if area.height < 8 || area.width < 40 {
        return;
    }
    let head_h = view::section_header(
        f,
        &app.pal,
        Rect::new(area.x + 1, area.y, area.width.saturating_sub(2), 4),
        &t("home.title"),
        &t("home.desc"),
        true,
    );
    let rows = app.rows();
    let sel = app.forms.last().map_or(0, |s| s.sel);
    let top = area.y + head_h + 1;
    let mut y = top;
    let w = area.width as usize;
    for (i, row) in rows.iter().enumerate() {
        if y >= area.bottom().saturating_sub(1) {
            break;
        }
        let pal = &app.pal;
        match row {
            Row::Note(text, kind) => {
                let color = if *kind == NoteKind::Warn {
                    pal.warn
                } else {
                    pal.muted
                };
                for l in wrap(text, w.saturating_sub(6)) {
                    put(f, area.x + 2, y, vec![Span::styled(l, fg(color))]);
                    y += 1;
                }
                y += 1;
            }
            Row::Header(text) => {
                y += 1;
                put(
                    f,
                    area.x + 2,
                    y,
                    vec![
                        Span::styled(
                            text.to_uppercase(),
                            fg(pal.muted).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" ──────", fg(pal.rule)),
                    ],
                );
                y += 2;
            }
            Row::Action(label, act) => {
                let on = i == sel;
                let hover = app.popup.is_none() && app.hover == Some(Hit::Row(i));
                let name_style = if hover {
                    fg(pal.bright).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else if on {
                    fg(pal.bright).add_modifier(Modifier::BOLD)
                } else {
                    fg(pal.fg)
                };
                let marker = Span::styled(
                    if on { "▍" } else { " " },
                    fg(pal.accent).add_modifier(Modifier::BOLD),
                );
                let mut spans = vec![marker, Span::raw(" ")];
                if let crate::app::Act::Open(slug) = act {
                    spans.push(Span::styled(
                        format!("{:<26}", truncate(label, 26)),
                        name_style,
                    ));
                    if let Some(item) = app.home.iter().find(|h| &h.slug == slug) {
                        for c in item.strip.iter() {
                            if let Some(rgb) = lizarbe_core::color::parse_hex(c) {
                                spans.push(Span::styled(
                                    "██ ",
                                    fg(ratatui::style::Color::Rgb(rgb.0, rgb.1, rgb.2)),
                                ));
                            }
                        }
                        spans.push(Span::styled(
                            format!(
                                " {}",
                                if item.mode == "light" {
                                    t("o.mode=light")
                                } else {
                                    t("o.mode=dark")
                                }
                            ),
                            fg(pal.muted),
                        ));
                    }
                } else {
                    spans.push(Span::styled(
                        format!("+ {label}"),
                        if on {
                            fg(pal.accent).add_modifier(Modifier::BOLD)
                        } else {
                            name_style
                        },
                    ));
                }
                put(f, area.x + 1, y, spans);
                app.hits
                    .push((Rect::new(area.x, y, area.width, 1), Hit::Row(i)));
                y += 1;
                if matches!(act, crate::app::Act::NewTheme) {
                    y += 1;
                }
            }
            Row::Field(_) => {}
        }
    }
}

// ---------------------------------------------------------------- pestañas

fn draw_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    if area.height < 2 {
        return;
    }
    let full: Vec<String> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, tb)| format!("{} {}", i + 1, t(tb.key())))
        .collect();
    let total: usize = full.iter().map(|s| s.width() + 3).sum();
    let compact = total + 4 > area.width as usize;
    let mut x = area.x + 2;
    for (i, label) in full.iter().enumerate() {
        let active = i == app.tab;
        let text = if compact && !active {
            format!("{}", i + 1)
        } else {
            label.clone()
        };
        let w = text.width() as u16;
        let hit = Hit::Tab(i);
        let hover = app.popup.is_none() && app.hover == Some(hit);
        let focus_here = active && app.focus == Focus::Tabs;
        let style = if active {
            fg(app.pal.bright).add_modifier(Modifier::BOLD)
        } else if hover {
            fg(app.pal.bright).add_modifier(Modifier::UNDERLINED)
        } else {
            fg(app.pal.muted)
        };
        let (num, rest) = text
            .split_once(' ')
            .map_or((text.as_str(), ""), |(a, b)| (a, b));
        put(
            f,
            x,
            area.y,
            vec![
                Span::styled(
                    num.to_string(),
                    fg(if active {
                        app.pal.accent
                    } else {
                        app.pal.muted
                    })
                    .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    if rest.is_empty() {
                        String::new()
                    } else {
                        format!(" {rest}")
                    },
                    style,
                ),
            ],
        );
        if active {
            let color = if focus_here || app.focus != Focus::Tabs {
                app.pal.accent
            } else {
                app.pal.muted
            };
            put(
                f,
                x,
                area.y + 1,
                vec![Span::styled("▔".repeat(w as usize), fg(color))],
            );
        }
        app.hits.push((Rect::new(x, area.y, w, 1), hit));
        x += w + 3;
        if x >= area.right() {
            break;
        }
    }
}

// ---------------------------------------------------------------- editor + maqueta

fn draw_editor(f: &mut Frame, app: &mut App, area: Rect) {
    if area.height < 9 || area.width < 40 {
        return;
    }
    let rule_y = area.bottom().saturating_sub(BUTTONS_H);
    let wide = area.width >= WIDE;
    let left_w = if wide {
        (area.width * 46 / 100).clamp(52, 66)
    } else {
        area.width
    };
    let left = Rect::new(area.x, area.y, left_w, rule_y.saturating_sub(area.y));
    let focused = app.focus == Focus::Editor && app.popup.is_none();

    let tab = app.tab_kind();
    let head_h = view::section_header(
        f,
        &app.pal,
        Rect::new(left.x + 1, left.y, left.width.saturating_sub(2), 4),
        &t(tab.key()),
        &t(&format!("{}.d", tab.key())),
        focused,
    );
    let form = Rect::new(
        left.x,
        left.y + head_h,
        left.width,
        left.height.saturating_sub(head_h),
    );
    draw_form(f, app, form, focused);

    rule_h(
        f,
        area.x + 2,
        rule_y,
        area.width.saturating_sub(4),
        app.pal.rule,
    );
    if wide {
        let vx = left.right();
        rule_v(f, vx, area.y, rule_y.saturating_sub(area.y), app.pal.rule);
        put(f, vx, rule_y, vec![Span::styled("┴", fg(app.pal.rule))]);
        let right = Rect::new(
            vx + 2,
            area.y,
            area.right().saturating_sub(vx + 3),
            rule_y.saturating_sub(area.y),
        );
        put(
            f,
            right.x,
            right.y,
            vec![Span::styled(
                t("preview.title").to_uppercase(),
                fg(app.pal.muted).add_modifier(Modifier::BOLD),
            )],
        );
        let scene = Rect::new(
            right.x,
            right.y + 2,
            right.width,
            right.height.saturating_sub(2),
        );
        match pane_image(app) {
            Some((path, caption)) => draw_image(f, app, scene, &path, &caption),
            None => {
                if let Some(spec) = app.preview_spec() {
                    mockup::draw(f, scene, &spec);
                }
            }
        }
    } else {
        put(
            f,
            area.x + 2,
            rule_y.saturating_sub(1),
            vec![Span::styled(t("preview.narrow"), fg(app.pal.dim))],
        );
    }

    if app.trial.is_some() {
        draw_trial(f, app, Rect::new(area.x, rule_y + 1, area.width, 1));
    } else {
        draw_buttons(f, app, Rect::new(area.x, rule_y + 1, area.width, 1));
    }
}

/// Imagen que se muestra a la derecha en lugar de la maqueta: el fondo elegido
/// (pestaña Fondos) o la vista previa del tema (pestaña Guardar).
fn pane_image(app: &App) -> Option<(std::path::PathBuf, String)> {
    let d = app.draft.as_ref()?;
    match app.tab_kind() {
        Tab::Backgrounds => {
            let rows = app.rows();
            let sel = app.forms[app.tab].sel;
            let idx = match rows.get(sel) {
                Some(Row::Action(_, crate::app::Act::Background(i))) => *i,
                _ => 0,
            };
            let p = d.spec.backgrounds.get(idx)?;
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            Some((p.clone(), name.to_string()))
        }
        Tab::Save => d.spec.preview.clone().map(|p| (p, t("preview.image"))),
        _ => None,
    }
}

fn draw_image(f: &mut Frame, app: &mut App, area: Rect, path: &std::path::Path, caption: &str) {
    use ratatui_image::{Resize, StatefulImage};
    if area.height < 3 {
        return;
    }
    app.images.request(path);
    put(
        f,
        area.x,
        area.y,
        vec![Span::styled(
            truncate(caption, area.width as usize),
            fg(app.pal.fg),
        )],
    );
    let r = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
    let (warn, muted) = (app.pal.warn, app.pal.muted);
    match app.images.entry(path) {
        Some(crate::images::Entry::Ready(proto)) => {
            f.render_stateful_widget(
                StatefulImage::default().resize(Resize::Fit(None)),
                r,
                &mut **proto,
            );
        }
        Some(crate::images::Entry::Failed) => {
            put(
                f,
                r.x,
                r.y,
                vec![Span::styled(t("preview.failed"), fg(warn))],
            );
        }
        _ => {
            put(
                f,
                r.x,
                r.y,
                vec![Span::styled(t("preview.loading"), fg(muted))],
            );
        }
    }
}

fn draw_form(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let rows = app.rows();
    let opts = FormOpts {
        focused,
        dropdown_open: matches!(&app.popup, Some(Popup::Picker(p)) if p.anchor.is_some()),
    };
    let App {
        draft,
        pal,
        hover,
        hits,
        popup,
        forms,
        tab,
        ..
    } = app;
    let Some(d) = draft.as_ref() else { return };
    let st = &mut forms[*tab];
    let mut ctx = Ctx {
        pal,
        hover: *hover,
        hits,
        blocked: popup.is_some(),
    };
    let info = |fr: &lizarbe_core::form::FieldRow<crate::draft::Bind>| RowInfo {
        changed: d.get_original(&fr.bind) != fr.value,
        extra: None,
    };
    view::draw_form(f, &mut ctx, area, &rows, st, &opts, info);
}

fn draw_buttons(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Buttons && app.popup.is_none();
    let can_undo = app.draft.as_ref().is_some_and(|d| d.can_undo());
    let buttons: Vec<ButtonSpec> = Button::ALL
        .iter()
        .map(|b| ButtonSpec {
            label: b.label(),
            key: b.key().to_string(),
            enabled: match b {
                Button::Undo => can_undo,
                _ => true,
            },
            primary: *b == Button::Save,
        })
        .collect();
    let (status, color) = match &app.busy {
        Some(msg) => (format!("◌ {msg}"), app.pal.muted),
        None if app.dirty() => (format!("• {}", t("status.unsaved")), app.pal.accent),
        None => (format!("✓ {}", t("status.saved")), app.pal.ok),
    };
    let selected = focused.then_some(app.button);
    view::draw_buttons(f, &mut ctx(app), area, &buttons, selected, (&status, color));
}

/// Mientras se prueba el tema en el escritorio, esta franja sustituye a los botones.
fn draw_trial(f: &mut Frame, app: &mut App, area: Rect) {
    let secs = app.countdown().unwrap_or(0);
    let pal = &app.pal;
    let mut x = area.x + 2;
    let label = tf("trial.running", &[("s", &secs.to_string())]);
    put(
        f,
        x,
        area.y,
        vec![Span::styled(
            label.clone(),
            fg(pal.accent).add_modifier(Modifier::BOLD),
        )],
    );
    x += label.width() as u16 + 4;
    let items = [
        ("K", t("trial.keep"), Hit::Keep),
        ("Esc", t("trial.revert"), Hit::Revert),
    ];
    for (key, text, hit) in items {
        let lit = app.hover == Some(hit);
        let w = button_width(key, &text) as u16;
        put(
            f,
            x,
            area.y,
            button_spans(pal, key, &text, true, key == "K", lit),
        );
        app.hits.push((Rect::new(x, area.y, w, 1), hit));
        x += w + 3;
    }
    put(
        f,
        x,
        area.y,
        button_spans(pal, "C", &t("trial.capture"), true, false, false),
    );
}

fn footer_hints(app: &App) -> Vec<Hint> {
    let k = |a: &str, b: &str, p: u8| (a.to_string(), t(b), p);
    if app.trial.is_some() {
        return vec![
            k("K", "trial.keep", 3),
            k("Esc", "trial.revert", 3),
            k("C", "trial.capture", 2),
        ];
    }
    if let Some(p) = &app.popup {
        return match p {
            Popup::Color(_) => vec![
                k("←→", "ft.change", 3),
                k("↑↓", "ft.channel", 2),
                k("Enter", "ft.save", 3),
                k("Esc", "ft.cancel", 3),
            ],
            Popup::Picker(_) => vec![
                k("↑↓", "ft.move", 1),
                k("Enter", "ft.choose", 3),
                k("abc", "ft.filter", 2),
                k("Esc", "ft.cancel", 3),
            ],
            Popup::Input(_) => vec![k("Enter", "ft.save", 3), k("Esc", "ft.cancel", 3)],
            Popup::Confirm { .. } => vec![k("Enter", "ft.yes", 3), k("Esc", "ft.no", 3)],
            _ => vec![k("Enter", "ft.close", 3)],
        };
    }
    if app.screen == Screen::Home {
        return vec![
            k("↑↓", "ft.move", 2),
            k("Enter", "ft.open", 3),
            k("n", "ft.new", 3),
            k("d", "ft.delete", 2),
            k("m", "ft.menu", 1),
            k("q", "ft.quit", 1),
        ];
    }
    let mut v = match app.focus {
        Focus::Tabs => vec![
            k("←→", "ft.tab", 3),
            k("Enter", "ft.open", 2),
            k("Esc", "ft.back", 3),
        ],
        Focus::Buttons => vec![
            k("←→", "ft.choose_button", 2),
            k("Enter", "ft.press", 3),
            k("Esc", "ft.back", 3),
        ],
        Focus::Editor => vec![
            k("←→", "ft.change", 2),
            k("Enter", "ft.edit", 3),
            k("r", "ft.reset", 1),
            k("m", "ft.menu", 1),
            k("Esc", "ft.back", 3),
        ],
    };
    v.push(k("/", "ft.search", 2));
    v.push(k("1-8", "ft.tabs", 1));
    v.push(k("z", "ft.undo", 1));
    v.push(k("?", "ft.help", 3));
    v
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let status = match &app.toast {
        Some(toast) => {
            let icon = if toast.kind == NoteKind::Warn {
                "▲"
            } else {
                "✓"
            };
            format!("{icon} {}", toast.text)
        }
        None => String::new(),
    };
    status_bar(
        f,
        &app.pal,
        area,
        &status,
        app.toast.is_some(),
        &footer_hints(app),
    );
    let _ = Style::new();
}

#[cfg(test)]
mod tests {
    use super::*;
    use lizarbe_core::theme::Palette;
    use lizarbe_core::themes::Dirs;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn app() -> (tempfile::TempDir, App) {
        let t = tempfile::tempdir().unwrap();
        let system = t.path().join("system/tokyo-night");
        std::fs::create_dir_all(system.join("backgrounds")).unwrap();
        std::fs::write(
            system.join("colors.toml"),
            "mode = \"dark\"\naccent = \"#7aa2f7\"\nbackground = \"#1a1b26\"\nforeground = \"#a9b1d6\"\nmuted = \"#414868\"\nred = \"#f7768e\"\ngreen = \"#9ece6a\"\nyellow = \"#e0af68\"\nblue = \"#7aa2f7\"\nmagenta = \"#ad8ee6\"\ncyan = \"#449dab\"\n",
        )
        .unwrap();
        std::fs::write(system.join("icons.theme"), "Yaru-blue\n").unwrap();
        let dirs = Dirs {
            user: t.path().join("user"),
            system: t.path().join("system"),
        };
        std::fs::create_dir_all(&dirs.user).unwrap();
        i18n::set_lang(i18n::Lang::Es);
        let app = App::new(
            dirs,
            Palette::default(),
            true,
            t.path().join("cache"),
            ratatui_image::picker::Picker::halfblocks(),
            t.path().join("colors.toml"),
        );
        (t, app)
    }

    fn render(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect())
            .collect()
    }

    fn open_editor(app: &mut App) {
        app.start_new("tokyo-night");
        // escribe el nombre y acepta
        for c in "Mi tema".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn home_screen_offers_a_new_theme_and_shows_the_close_button() {
        let (_t, mut app) = app();
        let text = render(&mut app, 100, 30).join("\n");
        assert!(text.contains("Crear un tema nuevo"), "{text}");
        assert!(text.contains("[Q] Cerrar"));
    }

    #[test]
    fn creating_a_theme_opens_the_editor_with_tabs_and_preview() {
        let (_t, mut app) = app();
        open_editor(&mut app);
        assert_eq!(app.screen, Screen::Edit);
        let text = render(&mut app, 140, 40).join("\n");
        for needle in [
            "1 Tema",
            "2 Paleta",
            "8 Guardar",
            "VISTA PREVIA",
            "terminal",
            "[S] Guardar",
            "[P] Probar",
            "[Esc] Volver",
            "Mi tema",
        ] {
            assert!(text.contains(needle), "falta {needle}\n{text}");
        }
    }

    #[test]
    fn every_tab_draws_at_every_width_and_has_no_untranslated_labels() {
        let (_t, mut app) = app();
        open_editor(&mut app);
        for w in [70u16, 100, 130, 170] {
            for i in 0..Tab::ALL.len() {
                app.tab = i;
                let lines = render(&mut app, w, 38);
                assert!(lines.iter().all(|l| l.chars().count() <= w as usize));
                for row in app.rows() {
                    if let Row::Field(f) = row {
                        assert!(
                            !f.def.label.contains('.') || f.def.label.contains(' '),
                            "sin traducir: {}",
                            f.def.label
                        );
                        assert!(
                            !f.def.desc.starts_with("o.") && !f.def.desc.starts_with("c."),
                            "sin traducir: {}",
                            f.def.desc
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn color_picker_updates_the_preview_live_and_esc_restores() {
        let (_t, mut app) = app();
        open_editor(&mut app);
        app.tab = 1;
        app.focus = Focus::Editor;
        // fila del acento: la primera seleccionable
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.popup,
            Some(lizarbe_core::popup::Popup::Color(_))
        ));
        let before = app.preview_spec().unwrap().colors["accent"].clone();
        app.on_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)); // canal H
        for _ in 0..10 {
            app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
        let live = app.preview_spec().unwrap().colors["accent"].clone();
        assert_ne!(before, live, "la maqueta sigue al selector");
        assert_eq!(
            app.draft.as_ref().unwrap().spec.colors["accent"],
            before,
            "el borrador no cambia hasta aceptar"
        );
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.popup.is_none());
        assert_eq!(app.draft.as_ref().unwrap().spec.colors["accent"], before);
        // aceptar sí cambia
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_ne!(app.draft.as_ref().unwrap().spec.colors["accent"], before);
        assert!(app.dirty());
    }

    #[test]
    fn generate_and_undo_and_save() {
        let (t, mut app) = app();
        open_editor(&mut app);
        app.tab = 1;
        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(app.draft.as_ref().unwrap().can_undo());
        app.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        assert!(!app.dirty());
        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        assert!(t.path().join("user/mi-tema/colors.toml").is_file());
        assert!(!app.dirty());
        // el tema de Omarchy no se tocó
        assert!(t.path().join("system/tokyo-night/colors.toml").is_file());
    }

    #[test]
    fn trial_shows_the_countdown_and_reverts() {
        let (t, mut app) = app();
        open_editor(&mut app);
        app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        assert!(app.trial.is_some());
        assert!(t.path().join("user/lizarbe-prueba/colors.toml").is_file());
        let text = render(&mut app, 120, 36).join("\n");
        assert!(text.contains("PROBANDO EN EL ESCRITORIO"), "{text}");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.trial.is_none());
        assert!(!t.path().join("user/lizarbe-prueba").exists());
    }

    #[test]
    fn existing_name_is_rejected() {
        let (_t, mut app) = app();
        app.start_new("tokyo-night");
        for c in "tokyo night".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(
            app.screen,
            Screen::Home,
            "no abre el editor con un nombre repetido"
        );
    }

    fn wait_ticks(app: &mut App, mut f: impl FnMut(&mut App) -> bool) {
        for _ in 0..1500 {
            app.tick();
            if f(app) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!(
            "tiempo agotado esperando la imagen (popup={:?}, busy={:?}, trial={})",
            app.popup.is_some(),
            app.busy,
            app.trial.is_some()
        );
    }

    #[test]
    fn palette_from_a_wallpaper_and_its_thumbnail() {
        let (t, mut app) = app();
        open_editor(&mut app);
        // un fondo azul oscuro con una franja turquesa
        let img = t.path().join("fondo.png");
        let mut im = image::RgbImage::from_pixel(160, 90, image::Rgb([14, 18, 40]));
        for x in 0..160 {
            for y in 60..90 {
                im.put_pixel(x, y, image::Rgb([20, 170, 190]));
            }
        }
        im.save(&img).unwrap();
        app.draft.as_mut().unwrap().spec.backgrounds = vec![img.clone()];
        app.draft.as_mut().unwrap().orig.backgrounds = vec![img.clone()];

        // la miniatura aparece en la pestaña Fondos
        app.tab = 4;
        let _ = render(&mut app, 150, 40);
        wait_ticks(&mut app, |a| {
            matches!(a.images.entry(&img), Some(crate::images::Entry::Ready(_)))
        });
        let text = render(&mut app, 150, 40).join("\n");
        assert!(text.contains("fondo.png"), "{text}");

        // y de ahí sale la paleta (tecla f en la pestaña Paleta)
        app.tab = 1;
        let before = app.draft.as_ref().unwrap().spec.colors.clone();
        app.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        assert!(matches!(
            app.popup,
            Some(lizarbe_core::popup::Popup::Picker(_))
        ));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        wait_ticks(&mut app, |a| {
            a.draft.as_ref().unwrap().spec.colors != before
        });
        let d = app.draft.as_ref().unwrap();
        assert_eq!(d.spec.mode, "dark");
        assert!(d.can_undo());
        let acc = lizarbe_core::color::parse_hex(&d.spec.colors["accent"]).unwrap();
        let (h, s, _) = lizarbe_core::color::to_hsl(acc);
        assert!(s > 0.3 && (170.0..200.0).contains(&h), "{h} {s}");
    }

    #[test]
    fn right_click_menu_removes_the_wallpapers_of_the_base_theme() {
        use crate::app::Act;
        use lizarbe_core::popup::Popup as P;
        use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let (t, mut app) = app();
        open_editor(&mut app);
        let inherited = t.path().join("system/tokyo-night/backgrounds/a.png");
        std::fs::write(&inherited, b"x").unwrap();
        let own = t.path().join("propio.png");
        std::fs::write(&own, b"x").unwrap();
        app.draft.as_mut().unwrap().spec.backgrounds = vec![inherited.clone(), own.clone()];
        app.tab = 4;
        let text = render(&mut app, 150, 40).join("\n");
        assert!(text.contains("(del tema base)"), "{text}");

        let idx = app
            .rows()
            .iter()
            .position(|r| matches!(r, Row::Action(_, Act::Background(0))))
            .unwrap();
        let (rect, _) = *app
            .hits
            .iter()
            .rev()
            .find(|(_, h)| *h == Hit::Row(idx))
            .unwrap();
        let click = |app: &mut App, x: u16, y: u16| {
            app.on_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Right),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            })
        };
        click(&mut app, rect.x + 2, rect.y);
        let Some(P::Menu(m)) = &app.popup else {
            panic!("el clic derecho debe abrir el menú")
        };
        assert_eq!(m.items.len(), 5);
        let text = render(&mut app, 150, 40).join("\n");
        assert!(text.contains("Quitar los fondos del tema base"), "{text}");

        // «subir» está desactivado en el primero: se salta
        for _ in 0..2 {
            app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.popup.is_none());
        assert_eq!(app.draft.as_ref().unwrap().spec.backgrounds, vec![own]);
        // y se puede deshacer
        app.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        assert_eq!(app.draft.as_ref().unwrap().spec.backgrounds.len(), 2);
    }

    #[test]
    fn search_finds_an_option_in_another_tab() {
        use crate::app::Act;
        let (_t, mut app) = app();
        // en el inicio no hay nada que buscar
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(app.popup.is_none());
        open_editor(&mut app);
        let text = render(&mut app, 120, 36).join("\n");
        assert!(text.contains("[/] Buscar"), "{text}");
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(matches!(
            app.popup,
            Some(lizarbe_core::popup::Popup::Picker(_))
        ));
        for c in "añadir fondo".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.popup.is_none());
        assert_eq!(app.tab_kind(), Tab::Backgrounds);
        let rows = app.rows();
        let sel = app.forms[app.tab].sel;
        assert!(matches!(
            rows.get(sel),
            Some(Row::Action(_, Act::AddBackground))
        ));
    }

    /// `cargo test dump_screens -- --ignored --nocapture` imprime las pantallas.
    #[test]
    #[ignore = "solo para revisar el aspecto a mano"]
    fn dump_screens() {
        let (_t, mut app) = app();
        println!("===== inicio =====");
        for l in render(&mut app, 150, 40) {
            println!("{}", l.trim_end());
        }
        open_editor(&mut app);
        for i in 0..Tab::ALL.len() {
            app.tab = i;
            println!("\n===== pestaña {} =====", i + 1);
            for l in render(&mut app, 150, 40) {
                println!("{}", l.trim_end());
            }
        }
    }
}
