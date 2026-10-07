//! Maqueta en vivo: un mini escritorio dibujado con los colores del borrador
//! (barra, ventana activa e inactiva con sus bordes, terminal con los 16
//! colores, código con resaltado y una tarjeta de menú).

use lizarbe_core::color::{mix, parse_hex};
use lizarbe_core::themes::{Border, ThemeSpec};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

type Rgb = (u8, u8, u8);

struct Pal<'a>(&'a ThemeSpec);

impl Pal<'_> {
    fn rgb(&self, key: &str, fallback: Rgb) -> Rgb {
        self.0
            .colors
            .get(key)
            .and_then(|v| parse_hex(v))
            .unwrap_or(fallback)
    }

    fn c(&self, key: &str, fallback: Rgb) -> Color {
        col(self.rgb(key, fallback))
    }
}

fn col(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

fn hex(s: &str, fallback: Rgb) -> Rgb {
    parse_hex(s).unwrap_or(fallback)
}

/// Color de la celda `i` de `n` de un borde (con degradado si lo tiene).
fn border_color(b: &Border, i: usize, n: usize) -> Rgb {
    let from = hex(&b.from, (128, 128, 128));
    match &b.to {
        None => from,
        Some(to) => {
            let to = hex(to, from);
            let mut t = i as f32 / n.max(1) as f32;
            if b.angle.is_some_and(|a| (90..270).contains(&a)) {
                t = 1.0 - t;
            }
            mix(from, to, t)
        }
    }
}

/// Dibuja una ventana: marco de una línea con el color del borde y el título
/// en el marco; devuelve el área interior.
fn window(
    f: &mut Frame,
    r: Rect,
    title: &str,
    border: &Border,
    bg: Color,
    title_fg: Color,
) -> Rect {
    if r.width < 4 || r.height < 3 {
        return Rect::default();
    }
    let (w, h) = (r.width as usize, r.height as usize);
    let perim = 2 * (w + h) - 4;
    let at = |i: usize| col(border_color(border, i, perim));
    let mut lines: Vec<Line> = vec![];
    // arriba (con título)
    let mut top = vec![Span::styled("╭", Style::new().fg(at(0)).bg(bg))];
    let title = format!(" {title} ");
    let tw = title.chars().count().min(w.saturating_sub(4));
    top.push(Span::styled("─", Style::new().fg(at(1)).bg(bg)));
    top.push(Span::styled(
        title.chars().take(tw).collect::<String>(),
        Style::new()
            .fg(title_fg)
            .bg(bg)
            .add_modifier(Modifier::BOLD),
    ));
    for k in (2 + tw)..(w - 1) {
        top.push(Span::styled("─", Style::new().fg(at(k)).bg(bg)));
    }
    top.push(Span::styled("╮", Style::new().fg(at(w - 1)).bg(bg)));
    lines.push(Line::from(top));
    for row in 1..h - 1 {
        lines.push(Line::from(vec![
            Span::styled("│", Style::new().fg(at(perim - row)).bg(bg)),
            Span::styled(" ".repeat(w - 2), Style::new().bg(bg)),
            Span::styled("│", Style::new().fg(at(w - 1 + row)).bg(bg)),
        ]));
    }
    let mut bottom = vec![Span::styled("╰", Style::new().fg(at(perim - h + 1)).bg(bg))];
    for k in 1..w - 1 {
        bottom.push(Span::styled(
            "─",
            Style::new().fg(at(perim - h + 1 - k)).bg(bg),
        ));
    }
    bottom.push(Span::styled("╯", Style::new().fg(at(w + h - 2)).bg(bg)));
    lines.push(Line::from(bottom));
    f.render_widget(Paragraph::new(lines), r);
    Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 2)
}

fn text(f: &mut Frame, r: Rect, lines: Vec<Line<'_>>, bg: Color) {
    if r.width > 0 && r.height > 0 {
        f.render_widget(Paragraph::new(lines).style(Style::new().bg(bg)), r);
    }
}

/// Dibuja la maqueta del tema `spec` en `area`.
pub fn draw(f: &mut Frame, area: Rect, spec: &ThemeSpec) {
    if area.width < 24 || area.height < 8 {
        return;
    }
    let p = Pal(spec);
    let bg = p.c("background", (26, 27, 38));
    let fg = p.c("foreground", (169, 177, 214));
    let muted = p.c("muted", (65, 72, 104));
    let accent = p.c("accent", (122, 162, 247));
    let sel_bg = p.c("selection", (41, 46, 66));
    f.render_widget(Paragraph::new("").style(Style::new().bg(bg)), area);

    // --- barra
    let (bar_bg, bar_fg, bar_active) = match &spec.surfaces {
        Some(s) => (
            col(hex(&s.bar_bg, (26, 27, 38))),
            col(hex(&s.bar_text, (169, 177, 214))),
            col(hex(&s.bar_active, (247, 118, 142))),
        ),
        None => (bg, fg, p.c("red", (247, 118, 142))),
    };
    let bar = Rect::new(area.x, area.y, area.width, 1);
    let ws = |n: u8, on: bool| {
        if on {
            Span::styled(
                format!(" {n} "),
                Style::new()
                    .fg(bar_bg)
                    .bg(accent)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(format!(" {n} "), Style::new().fg(bar_fg).bg(bar_bg))
        }
    };
    let left = vec![ws(1, false), ws(2, true), ws(3, false), ws(4, false)];
    let right = "  ▮▮▮▯   ● ";
    let clock = "mar 7 oct  12:45";
    let used: usize = left
        .iter()
        .map(|s| s.content.chars().count())
        .sum::<usize>()
        + right.chars().count();
    let mid_pad = (area.width as usize).saturating_sub(used + clock.chars().count());
    let mut spans = left;
    spans.push(Span::styled(
        " ".repeat(mid_pad / 2),
        Style::new().bg(bar_bg),
    ));
    spans.push(Span::styled(clock, Style::new().fg(bar_fg).bg(bar_bg)));
    spans.push(Span::styled(
        " ".repeat(mid_pad - mid_pad / 2),
        Style::new().bg(bar_bg),
    ));
    spans.push(Span::styled(right, Style::new().fg(bar_fg).bg(bar_bg)));
    spans.push(Span::styled("", Style::new().fg(bar_active).bg(bar_bg)));
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::new().bg(bar_bg)),
        bar,
    );
    // un guion bajo la barra que se ve como el borde del escritorio
    let body = Rect::new(
        area.x + 1,
        area.y + 2,
        area.width.saturating_sub(2),
        area.height.saturating_sub(3),
    );

    // --- ventanas: terminal (activa) a la izquierda; editor (inactiva) y menú a la derecha
    let two = body.width >= 56;
    let lw = if two {
        body.width * 11 / 20
    } else {
        body.width
    };
    let term = Rect::new(body.x, body.y, lw, body.height.min(14));
    let inner = window(f, term, "terminal", &spec.active_border, bg, accent);
    let ansi = |k: &str, fb: Rgb| p.c(k, fb);
    let prompt = |cmd: &str| {
        Line::from(vec![
            Span::styled("~ ", Style::new().fg(ansi("blue", (122, 162, 247))).bg(bg)),
            Span::styled(
                "❯ ",
                Style::new().fg(accent).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(cmd.to_string(), Style::new().fg(fg).bg(bg)),
        ])
    };
    let mut lines = vec![
        prompt("ls"),
        Line::from(vec![
            Span::styled(
                "src  ",
                Style::new()
                    .fg(ansi("blue", (122, 162, 247)))
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Cargo.toml  README.md", Style::new().fg(fg).bg(bg)),
        ]),
        prompt("git status"),
        Line::from(vec![
            Span::styled(
                " M ",
                Style::new().fg(ansi("yellow", (224, 175, 104))).bg(bg),
            ),
            Span::styled("README.md", Style::new().fg(fg).bg(bg)),
        ]),
        Line::from(vec![
            Span::styled(
                " + ",
                Style::new().fg(ansi("green", (158, 206, 106))).bg(bg),
            ),
            Span::styled("tema.toml  ", Style::new().fg(fg).bg(bg)),
            Span::styled(" - ", Style::new().fg(ansi("red", (247, 118, 142))).bg(bg)),
            Span::styled("viejo.toml", Style::new().fg(fg).bg(bg)),
        ]),
        prompt("█"),
        Line::from(""),
    ];
    let row = |keys: [&str; 8]| {
        Line::from(
            keys.iter()
                .map(|k| Span::styled("██ ", Style::new().fg(ansi(k, (128, 128, 128))).bg(bg)))
                .collect::<Vec<_>>(),
        )
    };
    lines.push(row([
        "dark_background",
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "foreground",
    ]));
    lines.push(row([
        "muted",
        "bright_red",
        "bright_green",
        "bright_yellow",
        "bright_blue",
        "bright_magenta",
        "bright_cyan",
        "bright_foreground",
    ]));
    text(f, inner, lines, bg);

    if !two {
        return;
    }
    let rx = body.x + lw + 1;
    let rw = body.width - lw - 1;
    let code_h = 8u16.min(body.height.saturating_sub(5)).max(5);
    let editor = Rect::new(rx, body.y, rw, code_h);
    let inner = window(f, editor, "editor", &spec.inactive_border, bg, muted);
    let kw = |s: &str| {
        Span::styled(
            s.to_string(),
            Style::new().fg(ansi("magenta", (173, 142, 230))).bg(bg),
        )
    };
    let plain = |s: &str| Span::styled(s.to_string(), Style::new().fg(fg).bg(bg));
    let num = |n: u8| Span::styled(format!("{n:>2} "), Style::new().fg(muted).bg(bg));
    let code = vec![
        Line::from(vec![
            num(1),
            kw("fn "),
            Span::styled(
                "main",
                Style::new().fg(ansi("blue", (122, 162, 247))).bg(bg),
            ),
            plain("() {"),
        ]),
        Line::from(vec![
            num(2),
            plain("    "),
            kw("let "),
            plain("x = "),
            Span::styled(
                "\"tema\"",
                Style::new().fg(ansi("green", (158, 206, 106))).bg(bg),
            ),
            plain(";"),
        ]),
        Line::from(vec![
            num(3),
            plain("    "),
            Span::styled(
                "// colores",
                Style::new().fg(muted).bg(bg).add_modifier(Modifier::ITALIC),
            ),
        ]),
        Line::from(vec![
            num(4),
            plain("    "),
            Span::styled(
                "print!",
                Style::new().fg(ansi("cyan", (68, 157, 171))).bg(bg),
            ),
            plain("(x, "),
            Span::styled(
                "42",
                Style::new().fg(ansi("orange", (235, 146, 123))).bg(bg),
            ),
            plain(");"),
        ]),
        Line::from(vec![num(5), plain("}")]),
    ];
    text(f, inner, code, bg);

    // tarjeta de menú
    let card_y = body.y + code_h + 1;
    let card_h = body.height.saturating_sub(code_h + 1).min(7);
    if card_h >= 4 {
        let (cbg, cfg, cborder, csel) = match &spec.surfaces {
            Some(s) => (
                hex(&s.card_bg, (36, 40, 59)),
                hex(&s.card_text, (169, 177, 214)),
                hex(&s.card_border, (122, 162, 247)),
                hex(&s.card_selected, (122, 162, 247)),
            ),
            None => (
                p.rgb("lighter_background", (36, 40, 59)),
                p.rgb("foreground", (169, 177, 214)),
                p.rgb("accent", (122, 162, 247)),
                p.rgb("accent", (122, 162, 247)),
            ),
        };
        let card = Rect::new(rx + rw.saturating_sub(24), card_y, 24.min(rw), card_h);
        let inner = window(
            f,
            card,
            "menú",
            &Border::solid(&lizarbe_core::color::to_hex(cborder)),
            col(cbg),
            col(cborder),
        );
        let sel = col(mix(cbg, csel, 0.18));
        let item = |label: &str, on: bool| {
            if on {
                Line::from(vec![
                    Span::styled(
                        "▍",
                        Style::new()
                            .fg(col(csel))
                            .bg(sel)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{label:<w$}", w = inner.width as usize - 1),
                        Style::new()
                            .fg(col(cfg))
                            .bg(sel)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])
            } else {
                Line::from(Span::styled(
                    format!(" {label}"),
                    Style::new().fg(col(cfg)).bg(col(cbg)),
                ))
            }
        };
        text(
            f,
            inner,
            vec![
                item("Tema", true),
                item("Fondo", false),
                item("Fuente", false),
            ],
            col(cbg),
        );
    }
    let _ = sel_bg;
}

#[cfg(test)]
mod tests {
    use super::*;
    use lizarbe_core::themes::Surfaces;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::BTreeMap;

    fn spec(gradient: bool) -> ThemeSpec {
        let mut colors = BTreeMap::new();
        for (k, v) in [
            ("background", "#1a1b26"),
            ("foreground", "#a9b1d6"),
            ("accent", "#7aa2f7"),
            ("red", "#f7768e"),
            ("green", "#9ece6a"),
        ] {
            colors.insert(k.to_string(), v.to_string());
        }
        let mut active = Border::solid("#26a269");
        if gradient {
            active.to = Some("#2ec27e".into());
            active.angle = Some(45);
        }
        ThemeSpec {
            mode: "dark".into(),
            surfaces: Some(Surfaces::from_palette(&colors)),
            colors,
            active_border: active,
            inactive_border: Border::solid("#414868"),
            icons: String::new(),
            neovim: None,
            vscode: None,
            keyboard: None,
            backgrounds: vec![],
            unlock: None,
            preview: None,
        }
    }

    fn render(spec: &ThemeSpec, w: u16, h: u16) -> Vec<String> {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, f.area(), spec)).unwrap();
        let b = t.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| b[(x, y)].symbol().to_string()).collect())
            .collect()
    }

    #[test]
    fn draws_the_scene_at_several_sizes_without_panicking() {
        for (w, h) in [(24, 8), (40, 12), (60, 20), (90, 30), (140, 40)] {
            for g in [false, true] {
                let lines = render(&spec(g), w, h);
                assert_eq!(lines.len(), h as usize);
            }
        }
    }

    #[test]
    fn shows_the_main_pieces() {
        let text = render(&spec(false), 90, 28).join("\n");
        for needle in ["terminal", "editor", "menú", "❯", "fn ", "12:45"] {
            assert!(text.contains(needle), "falta {needle}\n{text}");
        }
    }

    #[test]
    fn gradient_border_changes_color_along_the_edge() {
        let b = Border {
            from: "#000000".into(),
            to: Some("#ffffff".into()),
            angle: Some(45),
        };
        let a = border_color(&b, 0, 100);
        let z = border_color(&b, 100, 100);
        assert_ne!(a, z);
        let rev = Border {
            angle: Some(180),
            ..b
        };
        assert_eq!(border_color(&rev, 0, 100), z);
    }
}
