//! Dibujo común de la Bienvenida: fondo, cabecera con el avance, pie, avisos
//! de logro y ventanas emergentes, más las piezas que usan todos los capítulos
//! (marcos, teclas, párrafos).

use lizarbe_core::ui::{put, spaced, truncate, wrap};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear};
use unicode_width::UnicodeWidthStr;

use crate::anim;
use crate::app::{App, Cap, Hit, Modal};
use crate::brand::*;
use crate::caps;
use crate::i18n::{t, tf};

pub const ANCHO_MIN: u16 = 80;
pub const ALTO_MIN: u16 = 24;
/// Ancho máximo del contenido (en pantallas muy anchas se centra).
const ANCHO_MAX: u16 = 116;
/// Alto máximo del contenido.
const ALTO_MAX: u16 = 32;

pub fn dibujar(app: &mut App, f: &mut Frame) {
    app.hits.clear();
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(bg()).fg(plata())), area);
    if area.width < ANCHO_MIN || area.height < ALTO_MIN {
        demasiado_pequena(f, area);
        return;
    }
    match app.capitulo() {
        Cap::Arranque => caps::arranque::dibujar(app, f, area),
        Cap::Final => caps::final_::dibujar(app, f, area),
        cap => {
            cabecera(app, f, area);
            pie(app, f, area);
            let w = (area.width - 4).min(ANCHO_MAX);
            // En pantallas altas el contenido se centra en vertical en vez de pegarse arriba.
            let disponible = area.height - 4;
            let alto = disponible.min(ALTO_MAX);
            let cuerpo = Rect::new(
                area.x + (area.width - w) / 2,
                area.y + 2 + (disponible - alto) / 2,
                w,
                alto,
            );
            match cap {
                Cap::Super => caps::superc::dibujar(app, f, cuerpo),
                Cap::Practica => caps::practica::dibujar(app, f, cuerpo),
                Cap::Terminal => caps::terminal::dibujar(app, f, cuerpo),
                Cap::Telefono => caps::telefono::dibujar(app, f, cuerpo),
                Cap::Escritorio => caps::escritorio::dibujar(app, f, cuerpo),
                Cap::Configurar => caps::configurar::dibujar(app, f, cuerpo),
                Cap::Conceptos => caps::conceptos::dibujar(app, f, cuerpo),
                Cap::Atajos => caps::atajos::dibujar(app, f, cuerpo),
                Cap::Actualizar => caps::actualizar::dibujar(app, f, cuerpo),
                Cap::Arranque | Cap::Final => {}
            }
        }
    }
    aviso_logro(app, f, area);
    match app.modal {
        Modal::Salir => modal_salir(app, f, area),
        Modal::Ayuda => modal_ayuda(f, area),
        Modal::Ninguno => {}
    }
}

fn demasiado_pequena(f: &mut Frame, area: Rect) {
    let lineas = [
        t("peq.1"),
        tf(
            "peq.2",
            &[("w", &ANCHO_MIN.to_string()), ("h", &ALTO_MIN.to_string())],
        ),
    ];
    for (i, l) in lineas.iter().enumerate() {
        let w = l.width() as u16;
        let x = area.x + area.width.saturating_sub(w) / 2;
        let y = area.y + area.height / 2 + i as u16;
        put(
            f,
            x,
            y,
            vec![Span::styled(
                l.clone(),
                if i == 0 { bold(rojo()) } else { fg(plata()) },
            )],
        );
    }
}

// ---------------------------------------------------------------- piezas

/// Texto en (x, y).
pub fn texto(f: &mut Frame, x: u16, y: u16, s: &str, st: Style) -> u16 {
    put(f, x, y, vec![Span::styled(s.to_string(), st)])
}

/// Escribe una línea de `spans` centrada en el ancho de `r`.
pub fn centrada(f: &mut Frame, r: Rect, y: u16, spans: Vec<Span<'static>>) {
    let w: usize = spans.iter().map(|s| s.content.width()).sum();
    let x = r.x + (r.width as usize).saturating_sub(w) as u16 / 2;
    put(f, x, y, spans);
}

/// Párrafo con ajuste de línea. Devuelve cuántas filas ocupó.
pub fn parrafo(f: &mut Frame, x: u16, y: u16, ancho: u16, s: &str, st: Style) -> u16 {
    let lineas = wrap(s, ancho as usize);
    for (i, l) in lineas.iter().enumerate() {
        texto(f, x, y + i as u16, l, st);
    }
    lineas.len() as u16
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Marco {
    Fino,
    Grueso,
    Doble,
}

/// Marco con las esquinas del sistema de diseño (`┌─┐`, `┏━┓`, `╔═╗`).
pub fn marco(f: &mut Frame, r: Rect, tipo: Marco, color: ratatui::style::Color, titulo: &str) {
    if r.width < 2 || r.height < 2 {
        return;
    }
    let (tl, tr, bl, br, h, v) = match tipo {
        Marco::Fino => ('┌', '┐', '└', '┘', '─', '│'),
        Marco::Grueso => ('┏', '┓', '┗', '┛', '━', '┃'),
        Marco::Doble => ('╔', '╗', '╚', '╝', '═', '║'),
    };
    let st = fg(color);
    let w = r.width as usize;
    let mut arriba = format!("{tl}{}{tr}", h.to_string().repeat(w - 2));
    // El título solo se dibuja si cabe con sus guiones (un recuadro estrecho, como
    // el aviso de logro mientras entra deslizándose, se queda sin título).
    let tit = format!(" {} ", truncate(titulo, w.saturating_sub(6)));
    let con_titulo = !titulo.is_empty() && w >= 8 && tit.width() + 3 <= w - 2;
    if con_titulo {
        let resto = w - 2 - 1 - tit.width();
        arriba = format!("{tl}{h}{tit}{}{tr}", h.to_string().repeat(resto));
    }
    texto(f, r.x, r.y, &arriba, st);
    if con_titulo {
        texto(f, r.x + 2, r.y, &tit, bold(fosforo()));
    }
    for y in 1..r.height - 1 {
        texto(f, r.x, r.y + y, &v.to_string(), st);
        texto(f, r.right() - 1, r.y + y, &v.to_string(), st);
    }
    texto(
        f,
        r.x,
        r.bottom() - 1,
        &format!("{bl}{}{br}", h.to_string().repeat(w - 2)),
        st,
    );
}

/// Una tecla dibujada como tecla física.
pub fn tecla(label: &str, activa: bool) -> Span<'static> {
    let st = if activa {
        Style::new()
            .fg(fosforo())
            .bg(rojo())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(fosforo()).bg(acero())
    };
    Span::styled(format!(" {label} "), st)
}

/// `[Super] + [Espacio]`; `separador` vacío para pegarlas con un hueco.
pub fn combo(teclas: &[&str], activa: bool, separador: &str) -> Vec<Span<'static>> {
    let mut v = vec![];
    for (i, k) in teclas.iter().enumerate() {
        if i > 0 {
            v.push(Span::styled(
                if separador.is_empty() {
                    " ".to_string()
                } else {
                    format!(" {separador} ")
                },
                fg(pizarra()),
            ));
        }
        v.push(tecla(k, activa));
    }
    v
}

pub fn ancho_spans(s: &[Span]) -> u16 {
    s.iter().map(|x| x.content.width() as u16).sum()
}

/// Rellena un rectángulo con el fondo.
pub fn limpiar(f: &mut Frame, r: Rect) {
    f.render_widget(Clear, r);
    f.render_widget(Block::new().style(Style::new().bg(bg()).fg(plata())), r);
}

/// Título del paso y, justo debajo, la frase que dice qué hacer ahora. Devuelve las filas usadas.
pub fn titulo_capitulo(app: &App, f: &mut Frame, r: Rect, clave: &str) -> u16 {
    texto(f, r.x, r.y, &t(clave), bold(fosforo()));
    let g = app.guia();
    let (marca, st_marca, st) = if g.hecho {
        ("✓", bold(verde()), bold(verde()))
    } else {
        let p = anim::pulso(app.cap_t(), 1.4);
        ("▸", bold(blend(rojo(), fosforo(), p * 0.5)), fg(fosforo()))
    };
    texto(f, r.x, r.y + 1, marca, st_marca);
    let filas = parrafo(f, r.x + 2, r.y + 1, r.width.saturating_sub(2), &g.texto, st);
    1 + filas
}

// ---------------------------------------------------------------- cabecera y pie

fn cabecera(app: &mut App, f: &mut Frame, area: Rect) {
    put(
        f,
        area.x + 2,
        area.y,
        vec![
            Span::styled(format!("{SPARK} "), bold(rojo())),
            Span::styled(spaced("LIZARBE"), bold(fosforo())),
        ],
    );
    let actual = app.capitulo().numero().unwrap_or(0);
    let total = Cap::NUMERADOS;
    let cuenta = if actual > 0 {
        format!("{actual} / {total}")
    } else {
        String::new()
    };
    let ayuda = format!("? {}", t("ayuda.titulo"));
    let x = area
        .right()
        .saturating_sub(2 + (cuenta.width() + 3 + ayuda.width()) as u16);
    put(
        f,
        x,
        area.y,
        vec![
            Span::styled(ayuda, fg(pizarra())),
            Span::raw("   "),
            Span::styled(cuenta, bold(fosforo())),
        ],
    );
    // Una raya fina dividida en un tramo por paso; los hechos, en rojo.
    let ancho = area.width.saturating_sub(4);
    let tramo = (ancho / total as u16).max(2);
    for i in 1..=total {
        let x = area.x + 2 + (i as u16 - 1) * tramo;
        let w = if i == total {
            (area.x + 2 + ancho).saturating_sub(x)
        } else {
            tramo - 1
        };
        let lit = app.hover == Some(Hit::Cap(i));
        let (c, st) = match i.cmp(&actual) {
            std::cmp::Ordering::Greater => ('─', fg(acero())),
            _ => ('━', fg(rojo())),
        };
        let st = if lit { bold(cadmio()) } else { st };
        texto(f, x, area.y + 1, &c.to_string().repeat(w as usize), st);
        app.hits
            .push((Rect::new(x, area.y + 1, w + 1, 1), Hit::Cap(i)));
    }
}

/// Pie común a todos los pasos: volver a la izquierda, lo propio del paso en
/// medio y, a la derecha y siempre en el mismo sitio, continuar.
fn pie(app: &mut App, f: &mut Frame, area: Rect) {
    let y = area.bottom() - 1;
    lizarbe_core::ui::rule_h(f, area.x + 2, y - 1, area.width.saturating_sub(4), acero());
    let lit = |app: &App, h: Hit, normal| {
        if app.hover == Some(h) {
            bold(cadmio())
        } else {
            normal
        }
    };

    // Continuar (a la derecha).
    let etiqueta = format!(" {} →", app.etiqueta_enter());
    let tecla_enter = tecla("Enter", true);
    let w_sig = tecla_enter.content.width() as u16 + etiqueta.width() as u16;
    let x_sig = area.right().saturating_sub(2 + w_sig);
    let st = lit(app, Hit::Siguiente, bold(fosforo()));
    put(f, x_sig, y, vec![tecla_enter, Span::styled(etiqueta, st)]);
    app.hits
        .push((Rect::new(x_sig, y, w_sig, 1), Hit::Siguiente));

    // Volver (a la izquierda).
    let mut x = area.x + 2;
    if app.cap > 0 {
        let atras = format!("← {}", t("nav.atras"));
        let w = atras.width() as u16;
        let st = lit(app, Hit::Anterior, fg(plata()));
        texto(f, x, y, &atras, st);
        app.hits.push((Rect::new(x, y, w, 1), Hit::Anterior));
        x += w + 4;
    }

    // Lo propio del paso.
    if let Some((k, etiqueta, accion)) = app.accion_secundaria() {
        let hit = Hit::Boton(accion);
        let spans = vec![
            tecla(k, false),
            Span::styled(format!(" {etiqueta}"), lit(app, hit, fg(fosforo()))),
        ];
        let w = ancho_spans(&spans);
        if x + w + 2 < x_sig {
            put(f, x, y, spans);
            app.hits.push((Rect::new(x, y, w, 1), hit));
        }
    }
}

// ---------------------------------------------------------------- avisos y ventanas

fn aviso_logro(app: &App, f: &mut Frame, area: Rect) {
    let Some((texto_logro, hasta)) = &app.toast else {
        return;
    };
    let w = (texto_logro.width() as u16 + 6)
        .max(26)
        .min(area.width.saturating_sub(4));
    let entra = anim::smooth((3.5 - (hasta - app.t)) / 0.3);
    let sale = anim::smooth((hasta - app.t) / 0.3);
    let visible = entra.min(sale);
    let desplazamiento = ((1.0 - visible) * (w as f32 + 4.0)) as u16;
    let x = (area.right().saturating_sub(w + 2)) + desplazamiento;
    if x >= area.right() {
        return;
    }
    let r = Rect::new(x, area.y + 2, w.min(area.right() - x), 4);
    limpiar(f, r);
    marco(
        f,
        r,
        Marco::Grueso,
        rojo(),
        &format!("{SPARK} {}", t("logro.titulo")),
    );
    texto(
        f,
        r.x + 2,
        r.y + 2,
        &truncate(texto_logro, r.width.saturating_sub(4) as usize),
        bold(fosforo()),
    );
}

fn modal_salir(app: &mut App, f: &mut Frame, area: Rect) {
    let r = lizarbe_core::ui::centered(area, 64, 11);
    limpiar(f, r);
    marco(f, r, Marco::Doble, rojo(), &t("salir.titulo"));
    let mut y = r.y + 2;
    y += parrafo(f, r.x + 3, y, r.width - 6, &t("salir.texto"), fg(fosforo()));
    y += 1;
    for (tecla_, clave) in [
        ("Enter", "salir.si"),
        ("N", "salir.no"),
        ("Esc", "salir.seguir"),
    ] {
        put(
            f,
            r.x + 3,
            y,
            vec![
                tecla(tecla_, tecla_ == "Enter"),
                Span::styled(format!("  {}", t(clave)), fg(plata())),
            ],
        );
        y += 1;
    }
    let _ = app;
}

fn modal_ayuda(f: &mut Frame, area: Rect) {
    let r = lizarbe_core::ui::centered(area, 70, 12);
    limpiar(f, r);
    marco(f, r, Marco::Doble, rojo(), &t("ayuda.titulo"));
    let filas = [
        ("Enter", "ayuda.continuar"),
        ("Espacio", "ayuda.espacio"),
        ("←", "ayuda.atras"),
        ("↑ ↓", "ayuda.elegir"),
        ("Esc", "ayuda.salir"),
    ];
    let mut y = r.y + 2;
    for (k, clave) in filas {
        put(
            f,
            r.x + 3,
            y,
            vec![
                tecla(k, k == "Enter"),
                Span::styled(format!("  {}", t(clave)), fg(plata())),
            ],
        );
        y += 1;
    }
    y += 1;
    parrafo(f, r.x + 3, y, r.width - 6, &t("ayuda.pie"), fg(pizarra()));
}

/// Recorta una línea de spans a `ancho` celdas.
pub fn recortar(spans: &[Span<'static>], ancho: usize) -> Vec<Span<'static>> {
    let mut out = vec![];
    let mut usado = 0;
    for s in spans {
        if usado >= ancho {
            break;
        }
        let restante = ancho - usado;
        if s.content.width() <= restante {
            usado += s.content.width();
            out.push(s.clone());
        } else {
            let mut txt = String::new();
            let mut w = 0;
            for c in s.content.chars() {
                let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                if w + cw > restante {
                    break;
                }
                txt.push(c);
                w += cw;
            }
            out.push(Span::styled(txt, s.style));
            break;
        }
    }
    out
}

/// Un botón clicable: tecla dibujada, etiqueta, la tecla que activa y si es el principal.
pub struct Boton<'a> {
    pub tecla: &'a str,
    pub etiqueta: &'a str,
    pub accion: char,
    pub principal: bool,
}

impl<'a> Boton<'a> {
    pub fn new(tecla: &'a str, etiqueta: &'a str, accion: char) -> Self {
        Boton {
            tecla,
            etiqueta,
            accion,
            principal: false,
        }
    }

    pub fn principal(mut self) -> Self {
        self.principal = true;
        self
    }
}

/// Dibuja el botón en (x, y) y lo registra para el ratón. Devuelve el ancho ocupado.
pub fn boton(app: &mut App, f: &mut Frame, x: u16, y: u16, b: Boton) -> u16 {
    let hit = Hit::Boton(b.accion);
    let lit = app.hover == Some(hit);
    let spans = vec![
        tecla(b.tecla, b.principal || lit),
        Span::styled(
            format!(" {}", b.etiqueta),
            if lit { bold(cadmio()) } else { fg(fosforo()) },
        ),
    ];
    let w = ancho_spans(&spans);
    put(f, x, y, spans);
    app.hits.push((Rect::new(x, y, w, 1), hit));
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn render(app: &mut App, w: u16, h: u16) -> Vec<String> {
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| dibujar(app, f)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect())
            .collect()
    }

    fn texto_de(app: &mut App, w: u16, h: u16) -> String {
        render(app, w, h).join("\n")
    }

    fn tecla_(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn app_en(cap: usize) -> App {
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        let mut a = App::new(true, cap);
        a.t = 30.0;
        a.cap_inicio = 0.0;
        a
    }

    #[test]
    fn every_chapter_draws_at_every_size_without_panicking() {
        for (w, h) in [(80, 24), (100, 30), (120, 36), (160, 45), (200, 60)] {
            for cap in 0..Cap::ALL.len() {
                let mut a = app_en(cap);
                // Con datos para que las listas no estén vacías.
                a.ir(cap);
                a.t = 30.0;
                let lineas = render(&mut a, w, h);
                assert_eq!(lineas.len(), h as usize);
                assert!(
                    lineas.iter().all(|l| l.chars().count() == w as usize),
                    "{cap} a {w}×{h}"
                );
            }
        }
    }

    #[test]
    fn too_small_windows_ask_to_be_enlarged() {
        let mut a = app_en(2);
        let t = texto_de(&mut a, 60, 20);
        assert!(t.contains("demasiado pequeña"), "{t}");
        assert!(!t.contains("LIZARBE"));
    }

    #[test]
    fn each_chapter_shows_its_title_the_header_progress_and_the_same_footer() {
        let titulos = [
            (1, "La tecla Super"),
            (2, "Práctica en vivo"),
            (3, "La terminal no muerde"),
            (4, "Conecte su teléfono"),
            (5, "Su escritorio, a su gusto"),
            (6, "Configurar sin editar archivos"),
            (7, "Conceptos clave"),
            (8, "Atajos imprescindibles"),
        ];
        for (n, titulo) in titulos {
            let mut a = app_en(Cap::de_numero(n));
            let t = texto_de(&mut a, 130, 40);
            assert!(t.contains(titulo), "paso {n}: falta «{titulo}»\n{t}");
            assert!(t.contains(&format!("{n} / 8")), "paso {n}: falta el avance");
            // Siempre se ve cómo continuar y cómo volver, en el mismo sitio.
            assert!(t.contains("Atrás"), "paso {n}: falta Atrás");
            let ultima = t.lines().last().unwrap();
            assert!(
                ultima.contains("Enter") && ultima.trim_end().ends_with("→"),
                "paso {n}: falta Continuar en la última fila\n{ultima}"
            );
            // Y una frase que dice qué hacer.
            assert!(
                t.contains('▸') || t.contains('✓'),
                "paso {n}: falta la guía"
            );
        }
    }

    #[test]
    fn update_step_lists_packages_and_has_no_step_number() {
        let mut a = app_en(Cap::Actualizar.indice());
        let t = texto_de(&mut a, 120, 36);
        for needle in [
            "Actualice su sistema",
            "2 actualizaciones disponibles",
            "lizarbe-ajustes",
            "1.0.1 → 1.1.0",
            "Espacio",
            "Actualizar ahora",
            "Omitir",
        ] {
            assert!(t.contains(needle), "falta «{needle}»\n{t}");
        }
        assert!(!t.contains(" / 8"), "{t}");
        a.act = crate::app::Act::AlDia;
        assert!(texto_de(&mut a, 120, 36).contains("al día"));
        a.act = crate::app::Act::SinRed;
        assert!(texto_de(&mut a, 120, 36).contains("sin conexión"));
    }

    #[test]
    fn colors_follow_the_theme_palette() {
        let p = lizarbe_core::theme::Palette {
            accent: ratatui::style::Color::Rgb(1, 2, 3),
            bg: ratatui::style::Color::Rgb(250, 250, 250),
            ..Default::default()
        };
        crate::brand::aplicar_paleta(&p);
        assert_eq!(crate::brand::rojo(), p.accent);
        assert_eq!(crate::brand::bg(), p.bg);
        // El isotipo toma el acento del tema.
        assert!(
            crate::brand::isotipo()
                .iter()
                .flat_map(|l| &l.spans)
                .any(|s| s.style.fg == Some(p.accent))
        );
        // Y la pantalla se pinta con el fondo del tema.
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        let mut a = app_en(Cap::Super.indice());
        term.draw(|f| dibujar(&mut a, f)).unwrap();
        assert_eq!(term.backend().buffer()[(0, 0)].bg, p.bg);
        crate::brand::paleta_de_marca();
    }

    #[test]
    fn the_footer_shows_the_step_action_only_where_there_is_one() {
        for (n, esperado) in [(4, "Vincular"), (5, "Aplicar"), (6, "Abrir ahora")] {
            let mut a = app_en(Cap::de_numero(n));
            let ultima = texto_de(&mut a, 130, 40)
                .lines()
                .last()
                .unwrap()
                .to_string();
            assert!(
                ultima.contains("Espacio") && ultima.contains(esperado),
                "paso {n}: {ultima}"
            );
        }
        for n in [1, 7, 8] {
            let mut a = app_en(Cap::de_numero(n));
            let ultima = texto_de(&mut a, 130, 40)
                .lines()
                .last()
                .unwrap()
                .to_string();
            assert!(!ultima.contains("Espacio"), "paso {n}: {ultima}");
        }
    }

    #[test]
    fn boot_screen_is_minimal_and_has_the_crt_logo() {
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        let mut a = App::new(true, 0);
        a.usuario = "Ana".into();
        a.t = 0.02;
        assert!(!texto_de(&mut a, 120, 36).contains('▀'));
        // Encendido: primero una línea que se ensancha…
        a.t = 0.3;
        let linea = texto_de(&mut a, 120, 36);
        assert!(
            linea.contains('━') && !linea.contains("Comenzar"),
            "{linea}"
        );
        // …luego el logo se aclara por tramas, sin texto de invitación aún.
        a.t = 1.0;
        let tramas = texto_de(&mut a, 120, 36);
        assert!(tramas.contains('░') && !tramas.contains('▀'), "{tramas}");
        assert!(!tramas.contains("Le damos la bienvenida"));
        a.t = 2.6;
        assert!(texto_de(&mut a, 120, 36).contains('▀'));
        a.t = 12.0;
        let completo = texto_de(&mut a, 120, 36);
        for needle in ["Le damos la bienvenida, Ana.", "Comenzar", "█"] {
            assert!(completo.contains(needle), "falta «{needle}»\n{completo}");
        }
        // Ya no hay lema ni datos técnicos: solo logo, nombre, saludo e invitación.
        for sobra in [
            "Una máquina con alma",
            "Núcleo",
            "Telemetría",
            "LZB // 2026",
        ] {
            assert!(!completo.contains(sobra), "sobra «{sobra}»\n{completo}");
        }
    }

    #[test]
    fn the_crt_logo_flickers_and_scans_over_time() {
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        let mut a = App::new(true, 0);
        // Con el haz que baja y los saltos de línea, la imagen no es estática.
        let mut term = Terminal::new(TestBackend::new(120, 36)).unwrap();
        let mut colores = std::collections::HashSet::new();
        for k in 0..40 {
            a.t = 6.0 + k as f32 * 0.07;
            term.draw(|fr| dibujar(&mut a, fr)).unwrap();
            let buf = term.backend().buffer().clone();
            let sig: Vec<_> = (0..36)
                .flat_map(|y| (0..60).map(move |x| (x, y)))
                .map(|(x, y)| format!("{:?}", buf[(x, y)].fg))
                .collect();
            colores.insert(sig.join(""));
        }
        assert!(colores.len() > 5, "el efecto CRT debe animarse");
    }

    #[test]
    fn super_chapter_draws_the_keyboard_and_examples() {
        let mut a = app_en(2);
        let t = texto_de(&mut a, 120, 36);
        for needle in [
            "Ctrl",
            "Espacio",
            "Super",
            "Esta es la tecla Super",
            "Abre una terminal",
            "Cierra la ventana activa",
        ] {
            assert!(t.contains(needle), "falta «{needle}»\n{t}");
        }
    }

    #[test]
    fn practice_shows_challenges_radar_and_marks_progress() {
        let mut a = app_en(3);
        let t = texto_de(&mut a, 130, 40);
        for needle in [
            "Abra el menú de Omarchy",
            "Abra una terminal",
            "Viaje al escritorio 2",
            "Haga esto ahora: abra el menú de Omarchy",
            "DEMOSTRACIÓN",
        ] {
            assert!(t.contains(needle), "falta «{needle}»\n{t}");
        }
        a.evento(&lizarbe_core::hypr_events::HyprEvent {
            name: "openlayer".into(),
            data: "omarchy-menu".into(),
        });
        let t = texto_de(&mut a, 130, 40);
        assert!(t.contains("Se abrió el menú"), "{t}");
        assert!(t.contains("Haga esto ahora: abra una terminal"), "{t}");
        // El aviso de logro entra deslizándose y a los 0,5 s ya se ve entero.
        a.t += 0.5;
        let t = texto_de(&mut a, 130, 40);
        assert!(
            t.contains("LOGRO") && t.contains("Menú de Omarchy abierto"),
            "{t}"
        );
    }

    #[test]
    fn toy_terminal_shows_prompt_history_and_ghost_suggestion() {
        let mut a = app_en(4);
        let t = texto_de(&mut a, 120, 36);
        assert!(t.contains("❯"), "{t}");
        assert!(t.contains("fastfetch"), "debe sugerir fastfetch\n{t}");
        a.on_key(tecla_(KeyCode::Tab));
        a.on_key(tecla_(KeyCode::Enter));
        let t = texto_de(&mut a, 120, 36);
        assert!(t.contains("Omarchy"), "debe mostrar la salida\n{t}");
        assert!(t.contains("Muy bien"), "{t}");
    }

    #[test]
    fn phone_chapter_shows_a_scannable_qr_steps_and_devices() {
        let mut a = app_en(5);
        let t = texto_de(&mut a, 130, 40);
        for needle in [
            "Instale KDE Connect",
            "Elija su teléfono abajo",
            "Teléfonos cercanos",
            "Teléfono de ejemplo",
            "Todas",
            "▀",
        ] {
            assert!(t.contains(needle), "falta «{needle}»\n{t}");
        }
        // Los códigos largos solo se dibujan si caben en la altura.
        a.on_key(tecla_(KeyCode::Tab));
        let pequena = texto_de(&mut a, 100, 30);
        assert!(pequena.contains("Amplíe la ventana"), "{pequena}");
        let grande = texto_de(&mut a, 130, 48);
        assert!(!grande.contains("Amplíe la ventana"));
        a.on_key(tecla_(KeyCode::Char(' ')));
        assert!(texto_de(&mut a, 130, 48).contains("Teléfono conectado"));
    }

    #[test]
    fn desktop_chapter_lists_themes_with_a_preview() {
        let mut a = app_en(6);
        let t = texto_de(&mut a, 130, 40);
        for needle in [
            "Lizarbe Light",
            "Tokyo Night",
            "Aplicar",
            "Otro fondo",
            "Volver al anterior",
        ] {
            assert!(t.contains(needle), "falta «{needle}»\n{t}");
        }
        a.on_key(tecla_(KeyCode::Char(' ')));
        assert!(texto_de(&mut a, 130, 40).contains("aplicado"));
    }

    #[test]
    fn tools_chapter_describes_the_selected_tool() {
        let mut a = app_en(7);
        for (i, nombre) in [
            "Escritorio",
            "Widgets y barra",
            "Notificaciones",
            "Capturas de pantalla",
            "Crear tema",
            "Tienda de aplicaciones",
            "Centro Lizarbe",
        ]
        .iter()
        .enumerate()
        {
            a.herramienta_sel = i;
            let t = texto_de(&mut a, 130, 40);
            assert!(t.contains(nombre), "falta «{nombre}»");
            assert!(t.contains("Abrir ahora") && t.contains("Menú ›"), "{t}");
        }
    }

    #[test]
    fn concepts_chapter_has_a_drawing_for_every_concept() {
        let mut a = app_en(8);
        for i in 0..crate::contenido::CONCEPTOS.len() {
            a.concepto_sel = i;
            for tt in [0.5, 2.5, 5.0] {
                a.t = tt;
                a.cap_inicio = 0.0;
                let t = texto_de(&mut a, 130, 40);
                assert!(
                    t.contains(
                        &crate::i18n::t(&format!("con.{}.1", crate::contenido::CONCEPTOS[i].id))
                            .split_whitespace()
                            .next()
                            .unwrap()
                            .to_string()
                    ),
                    "{i}"
                );
            }
        }
        a.concepto_sel = 0;
        let t = texto_de(&mut a, 130, 40);
        assert!(t.contains("Super + 1 … 5"), "{t}");
    }

    #[test]
    fn shortcuts_chapter_scrolls_to_keep_the_selection_visible() {
        let mut a = app_en(9);
        let t = texto_de(&mut a, 120, 30);
        assert!(t.contains("LO BÁSICO") && t.contains("Terminal"), "{t}");
        a.atajos_scroll = crate::contenido::total_atajos() - 1;
        let t = texto_de(&mut a, 120, 30);
        assert!(
            t.contains("Cambiar el tema"),
            "el último atajo debe verse\n{t}"
        );
        assert!(t.contains("Super") && t.contains("K"));
    }

    #[test]
    fn final_screen_lists_achievements_and_the_call_to_action() {
        let mut a = app_en(10);
        a.otorgar("menu");
        a.otorgar("telefono");
        a.usuario = "Ana".into();
        let t = texto_de(&mut a, 120, 36);
        for needle in [
            "Su equipo está listo",
            "2 de 9 logros",
            "Teléfono conectado",
            "Empezar a usar su Lizarbe",
            "Repetir el recorrido",
        ] {
            assert!(t.contains(needle), "falta «{needle}»\n{t}");
        }
    }

    #[test]
    fn modals_draw_on_top() {
        let mut a = app_en(3);
        a.on_key(tecla_(KeyCode::Char('q')));
        let t = texto_de(&mut a, 120, 36);
        assert!(
            t.contains("Salir de la Bienvenida") && t.contains("no volver a mostrarla"),
            "{t}"
        );
        a.on_key(tecla_(KeyCode::Esc));
        a.on_key(tecla_(KeyCode::Char('?')));
        assert!(texto_de(&mut a, 120, 36).contains("Ayuda"));
    }

    #[test]
    fn clicking_the_header_dots_and_footer_buttons_navigates() {
        use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let mut a = app_en(3);
        let _ = render(&mut a, 120, 36);
        let clic = |a: &mut App, x: u16, y: u16| {
            a.on_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            });
        };
        let (r, _) = *a.hits.iter().find(|(_, h)| *h == Hit::Cap(5)).unwrap();
        clic(&mut a, r.x, r.y);
        assert_eq!(a.capitulo(), Cap::Escritorio);
        let _ = render(&mut a, 120, 36);
        let (r, _) = *a.hits.iter().find(|(_, h)| *h == Hit::Siguiente).unwrap();
        clic(&mut a, r.x + 1, r.y);
        assert_eq!(a.capitulo(), Cap::Configurar);
        let _ = render(&mut a, 120, 36);
        let (r, _) = *a.hits.iter().find(|(_, h)| *h == Hit::Anterior).unwrap();
        clic(&mut a, r.x + 1, r.y);
        assert_eq!(a.capitulo(), Cap::Escritorio);
    }

    #[test]
    fn no_arithmetic_overflow_across_sizes_times_and_states() {
        // En la compilación de producción un desbordamiento no avisa: da la vuelta y
        // revienta más tarde. Las pruebas (que sí avisan) recorren muchos casos.
        for cap in 0..Cap::ALL.len() {
            for (w, h) in [
                (80, 24),
                (81, 25),
                (87, 27),
                (94, 31),
                (100, 24),
                (131, 41),
                (200, 61),
            ] {
                for tt in [
                    0.0, 0.05, 0.2, 0.31, 0.7, 1.1, 1.9, 2.4, 3.3, 5.0, 6.5, 9.0, 40.0,
                ] {
                    let mut a = app_en(cap);
                    a.t = tt;
                    a.cap_inicio = 0.0;
                    // Aviso de logro recién dado (entrando) y a mitad de camino.
                    a.otorgar("menu");
                    a.toast.as_mut().unwrap().1 = tt + 3.5 - (tt % 0.4);
                    a.evento(&lizarbe_core::hypr_events::HyprEvent {
                        name: "workspace".into(),
                        data: "2".into(),
                    });
                    let _ = render(&mut a, w, h);
                    if tt == 0.31 {
                        a.on_key(tecla_(KeyCode::Char('?')));
                        let _ = render(&mut a, w, h);
                        a.on_key(tecla_(KeyCode::Esc));
                        a.on_key(tecla_(KeyCode::Char('q')));
                        let _ = render(&mut a, w, h);
                    }
                }
            }
        }
    }

    #[test]
    fn the_achievement_toast_slides_in_without_breaking() {
        let mut a = app_en(3);
        a.t = 10.0;
        a.otorgar("menu");
        let hasta = a.toast.as_ref().unwrap().1;
        let mut visto = false;
        let mut dt = 0.0;
        while dt < 4.0 {
            a.t = 10.0 + dt;
            let t = texto_de(&mut a, 120, 36);
            visto |= t.contains("Menú de Omarchy abierto");
            dt += 0.03;
        }
        assert!(
            visto,
            "el aviso debe verse en algún momento (hasta {hasta})"
        );
    }

    /// `cargo test -p lizarbe-bienvenida dump_screens -- --ignored --nocapture` imprime las pantallas.
    #[test]
    #[ignore = "solo para revisar el aspecto a mano"]
    fn dump_screens() {
        let (w, h) = (130, 40);
        for cap in 0..Cap::ALL.len() {
            let mut a = app_en(cap);
            a.usuario = "Leonardo".into();
            if cap == 2 {
                a.evento(&lizarbe_core::hypr_events::HyprEvent {
                    name: "workspace".into(),
                    data: "2".into(),
                });
                a.otorgar("menu");
            }
            if cap == 3 {
                a.on_key(tecla_(KeyCode::Tab));
                a.on_key(tecla_(KeyCode::Enter));
            }
            println!("===== capítulo {cap} =====");
            for l in render(&mut a, w, h) {
                println!("{}", l.trim_end());
            }
        }
    }
}
