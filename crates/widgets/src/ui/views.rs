//! Panel de contenido: encabezado de sección, formularios con filas de tres
//! líneas y controles en relieve (estilo Meca), columnas de widgets, lista de
//! plugins y botones 3D Restaurar · Cancelar · Aplicar.

use lizarbe_core::ui::{center, right_align};
use lizarbe_core::view::{self, ButtonSpec, Ctx, FormOpts, RowInfo};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Span;
use serde_json::Value;
use unicode_width::UnicodeWidthStr;

use super::form::FieldRow;
use super::popup::Popup;
use super::{ctx, fg, hovered, pad, put, truncate, wrap};
use crate::app::{App, Button, Focus, Hit, PluginRow, Section};
use crate::i18n::{t, tf};
use crate::omarchy::curated;
use crate::omarchy::schema::{self, value_label};
use crate::omarchy::shell_json as sj;
use crate::store::Bind;

/// Alto de la zona de botones: la línea fina, los botones y una línea de aire.
pub(super) const BUTTONS_H: u16 = 3;

pub(super) fn draw_content(f: &mut Frame, app: &mut App, area: Rect, rule_y: u16) {
    let focused = app.focus == Focus::Content && app.popup.is_none();
    let x = area.x;
    let w = area.width;
    if area.height < 9 || w < 30 {
        return;
    }

    // Encabezado: título en mayúsculas y descripción.
    let (title, desc) = section_header(app);
    let head_h = view::section_header(
        f,
        &app.pal,
        Rect::new(x + 1, area.y + 1, w.saturating_sub(2), 4),
        &title,
        &desc,
        focused,
    );

    let top = area.y + 1 + head_h;
    let body = Rect::new(x, top, w, rule_y.saturating_sub(top));
    match app.section {
        Section::Widgets if app.widgets.editing.is_none() => draw_layout(f, app, body, focused),
        Section::Plugins => draw_plugins(f, app, body, focused),
        _ => draw_form(f, app, body, focused),
    }
    draw_buttons(f, app, Rect::new(x, rule_y + 1, w, 1));
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

// ---------------------------------------------------------------- botones

fn draw_buttons(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Buttons && app.popup.is_none();
    let buttons: Vec<ButtonSpec> = Button::ALL
        .iter()
        .map(|b| ButtonSpec {
            label: b.label(),
            key: b.key().to_string(),
            enabled: app.button_enabled(*b),
            primary: *b == Button::Apply,
        })
        .collect();
    // A la izquierda: estado de los cambios pendientes.
    let pending = app.store.changes().len() + app.store.ops.len();
    let (status, color) = if pending > 0 {
        (
            format!("• {}", tf("btn.pending", &[("n", &pending.to_string())])),
            app.pal.accent,
        )
    } else {
        (format!("✓ {}", t("btn.clean")), app.pal.ok)
    };
    let selected = focused.then_some(app.button);
    view::draw_buttons(f, &mut ctx(app), area, &buttons, selected, (&status, color));
}

// ---------------------------------------------------------------- formularios

fn draw_form(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    let rows = app.rows();
    let opts = FormOpts {
        focused,
        action_hint: None,
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
        widgets,
        section,
        ..
    } = app;
    let st = if *section == Section::Widgets {
        &mut widgets.form
    } else {
        &mut forms[section.index()]
    };
    let mut ctx = Ctx {
        pal,
        hover: *hover,
        hits,
        blocked: popup.is_some(),
    };
    let info = |fr: &FieldRow| RowInfo {
        changed: store.get_original(&fr.bind) != fr.value,
        extra: if fr.def.preview.is_some() {
            fr.effective()
                .and_then(Value::as_str)
                .map(|s| format!("{}: {}", t("ctl.format"), schema::escape_newlines(s)))
        } else if advanced {
            Some(format!(
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
            ))
        } else {
            None
        },
    };
    view::draw_form(f, &mut ctx, area, &rows, st, &opts, info);
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
    let inner = w.saturating_sub(6);
    let third = inner / 3;
    let bar_line = format!(
        "{}{}{}",
        pad(&truncate(&l, third), third),
        center(&truncate(&c, third), third),
        right_align(&truncate(&r, inner - 2 * third), inner - 2 * third)
    );
    put(
        f,
        area.x + 2,
        area.y,
        vec![Span::styled(
            format!(
                "{} · {}",
                t("lay.preview").to_uppercase(),
                t(&format!("pos.{pos}"))
            ),
            fg(app.pal.muted).add_modifier(Modifier::BOLD),
        )],
    );
    put(
        f,
        area.x,
        area.y + 1,
        vec![
            Span::styled(" │ ", fg(app.pal.rule)),
            Span::styled(bar_line, fg(app.pal.bright)),
            Span::styled(" │", fg(app.pal.rule)),
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
        let drop_target = app.widgets.drag.is_some()
            && matches!(app.widgets.drag_over, Some(Hit::Column(c)) | Some(Hit::Widget(c, _)) if c == s);
        let entries = sj::section(&app.store.json, s).to_vec();
        // Las columnas se separan con una línea fina; sin cajas.
        if s > 0 {
            lizarbe_core::ui::rule_v(f, a.x, a.y, a.height, app.pal.rule);
        }
        let inner = Rect::new(
            a.x + 2,
            a.y + 2,
            a.width.saturating_sub(3),
            a.height.saturating_sub(2),
        );
        let heading = format!(
            "{} · {}",
            t(&format!("col.{}", sj::SECTIONS[s])).to_uppercase(),
            entries.len()
        );
        put(
            f,
            a.x + 2,
            a.y,
            vec![Span::styled(
                truncate(&heading, a.width.saturating_sub(3) as usize),
                if active || drop_target {
                    fg(app.pal.accent).add_modifier(Modifier::BOLD)
                } else {
                    fg(app.pal.muted).add_modifier(Modifier::BOLD)
                },
            )],
        );
        lizarbe_core::ui::rule_h(
            f,
            a.x + 2,
            a.y + 1,
            a.width.saturating_sub(4),
            if active || drop_target {
                app.pal.accent
            } else {
                app.pal.rule
            },
        );
        app.hits.push((a, Hit::Column(s)));
        let iw = inner.width as usize;

        // Deja una fila para "+ Añadir".
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
            let marker = if is_sel && focused { "▍" } else { " " };
            let style = if over {
                fg(app.pal.accent).add_modifier(Modifier::UNDERLINED | Modifier::BOLD)
            } else if dragging {
                fg(app.pal.dim)
            } else if hover {
                fg(app.pal.bright).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if is_sel && focused {
                fg(app.pal.bright).add_modifier(Modifier::BOLD)
            } else if is_sel {
                fg(app.pal.bright)
            } else {
                fg(app.pal.fg)
            };
            put(
                f,
                inner.x - 1,
                y,
                vec![
                    Span::styled(marker, fg(app.pal.accent).add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" {}", truncate(&text, iw.saturating_sub(1))), style),
                ],
            );
            app.hits.push((Rect::new(inner.x, y, inner.width, 1), hit));
            y += 1;
        }
        // Acción para añadir al final de la columna.
        if y < inner.bottom() {
            let hit = Hit::AddWidget(s);
            let hover = hovered(app, hit);
            let style = if hover {
                fg(app.pal.bright).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                fg(app.pal.muted)
            };
            put(
                f,
                inner.x,
                y,
                vec![Span::styled(
                    truncate(&format!("+ {}", t("lay.add")), iw),
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
                    format!("  {}", widget_label(app, e)),
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
                    truncate(&format!("  {}", settings.join("  ")), w),
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
            truncate(&format!("  {}", t("lay.tip")), w),
            fg(app.pal.dim),
        )],
    );
}

// ---------------------------------------------------------------- plugins

fn draw_plugins(f: &mut Frame, app: &mut App, area: Rect, focused: bool) {
    app.clamp_plugin_sel(true);
    let rows = app.plugin_rows();
    let w = area.width as usize;
    let detail_h = 5u16;

    // Buscador (clicable).
    let hit = Hit::Search;
    let hover = hovered(app, hit);
    let searching = app.plugins.filtering || !app.plugins.filter.is_empty();
    let text = if searching {
        format!(
            "  / {}{}",
            app.plugins.filter,
            if app.plugins.filtering { "▏" } else { "" }
        )
    } else {
        format!("  / {}", t("pl.filter_hint"))
    };
    let style = if hover || app.plugins.filtering {
        fg(app.pal.bright).add_modifier(Modifier::UNDERLINED)
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
    let search_w = w.min(48) as u16 + 2;
    let global = [
        ('n', "", t("pl.btn.add"), true),
        ('U', "", t("pl.btn.update_all"), true),
    ];
    draw_chips(
        f,
        app,
        area.x + search_w,
        area.y,
        w.saturating_sub(search_w as usize),
        &global,
    );

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
                put(
                    f,
                    list.x + 2,
                    y,
                    vec![
                        Span::styled(
                            title.to_uppercase(),
                            fg(app.pal.muted).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" ──────", fg(app.pal.rule)),
                    ],
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
                let switch = if on {
                    format!("━━● {}", t("val.on"))
                } else {
                    format!("○── {}", t("val.off"))
                };
                let sw_w = switch.width().max(7) + 2;
                let right = format!("{state} · {origin}  ");
                let mut label = name;
                if app.advanced {
                    label = format!("{label}  {id}");
                }
                let left_w =
                    w.saturating_sub(4 + right.width() + sw_w + 1 + if changed { 2 } else { 0 });
                let marker = if is_sel { "▍" } else { " " };
                let name_style = if is_sel || hover_row {
                    fg(app.pal.bright).add_modifier(Modifier::BOLD)
                } else {
                    fg(app.pal.fg)
                };
                let mut spans = vec![
                    Span::styled(marker, fg(app.pal.accent).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        if on { " ● " } else { " ○ " },
                        fg(if on { app.pal.fg } else { app.pal.dim }),
                    ),
                    Span::styled(pad(&label, left_w), name_style),
                ];
                if changed {
                    spans.push(Span::styled(
                        " •",
                        fg(app.pal.accent).add_modifier(Modifier::BOLD),
                    ));
                }
                spans.push(Span::styled(right, fg(app.pal.muted)));
                let sw_style = if hover_sw {
                    fg(app.pal.bright).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else if on && is_sel {
                    fg(app.pal.accent).add_modifier(Modifier::BOLD)
                } else if on {
                    fg(app.pal.bright)
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
        area.x + 2,
        dy - 1,
        vec![Span::styled(
            "─".repeat(w.saturating_sub(4)),
            fg(app.pal.rule),
        )],
    );
    if let Some(p) = app.selected_plugin() {
        let mut head = vec![Span::styled(
            format!(
                "  {}",
                curated::widget_name(&p.id, None, &app.store.catalog)
            ),
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
                vec![Span::styled(format!("  {l}"), fg(app.pal.fg))],
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
                vec![Span::styled(truncate(&format!("  {text}"), w), fg(color))],
            );
        }
        let can_clone = p.first_party && app.store.catalog.clone_of(&p.id).is_none();
        let mut actions = vec![
            ('p', "󰆏", t("menu.p.clone"), can_clone),
            ('u', "󰚰", t("menu.p.update"), p.is_git),
            ('x', "󰆴", t("menu.p.remove"), !p.first_party),
        ];
        if app.advanced {
            actions.push(('e', "󰈔", t("menu.p.edit"), !p.first_party));
        }
        draw_chips(
            f,
            app,
            area.x + 1,
            area.bottom() - 1,
            w.saturating_sub(1),
            &actions,
        );
    }
}

/// Botones de texto de una línea para las acciones de plugins: (tecla
/// equivalente, —, texto, habilitado). La tecla se muestra tal cual
/// (`n`, `U`, `p`…). Se omiten los que no caben.
fn draw_chips(
    f: &mut Frame,
    app: &mut App,
    x: u16,
    y: u16,
    max_w: usize,
    chips: &[(char, &str, String, bool)],
) {
    let mut used = 0usize;
    for (key, _icon, label, enabled) in chips {
        let k = key.to_string();
        let cw = lizarbe_core::ui::button_width(&k, label);
        if used + cw > max_w {
            break;
        }
        let hit = Hit::PluginAction(*key);
        let lit = *enabled && hovered(app, hit);
        let cx = x + used as u16;
        put(
            f,
            cx,
            y,
            lizarbe_core::ui::button_spans(&app.pal, &k, label, *enabled, false, lit),
        );
        if *enabled {
            app.hits.push((Rect::new(cx, y, cw as u16, 1), hit));
        }
        used += cw + 3;
    }
}
