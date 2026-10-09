//! Dibujo de Recibir: a la izquierda el celular con KDE Connect y a la
//! derecha el texto, sobre el fondo del tema y sin marcos. En ventanas
//! pequeñas se queda solo el texto.

use lizarbe_core::ui::{pad, put, truncate};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Block;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Opcion, Situacion};
use crate::dibujo;
use crate::i18n::{t, tf};

/// Ancho de la columna de texto.
const ANCHO_TEXTO: u16 = 52;
/// Espacio entre el celular y el texto.
const SEPARACION: u16 = 5;
/// Alto de la columna de texto con la lista completa.
const ALTO_TEXTO: u16 = 23;
/// Tamaño mínimo para dibujar algo.
const MINIMO: (u16, u16) = (46, 16);

pub fn dibujar(app: &mut App, f: &mut Frame) {
    app.hits.clear();
    let area = f.area();
    let p = app.paleta.clone();
    f.render_widget(Block::new().style(Style::new().bg(p.bg).fg(p.fg)), area);
    if area.width < MINIMO.0 || area.height < MINIMO.1 {
        let msg = truncate(&t("peq"), area.width as usize);
        put(
            f,
            area.x,
            area.y,
            vec![Span::styled(msg, Style::new().fg(p.muted))],
        );
        return;
    }

    let ancho_celular = dibujo::ANCHO + 1;
    let con_celular = area.width >= ancho_celular + SEPARACION + ANCHO_TEXTO + 4
        && area.height >= dibujo::ALTO + 4;
    let ancho = ANCHO_TEXTO
        + if con_celular {
            ancho_celular + SEPARACION
        } else {
            0
        };
    let alto = if con_celular {
        dibujo::ALTO + 1
    } else {
        ALTO_TEXTO.min(area.height)
    };
    let x0 = area.x + area.width.saturating_sub(ancho) / 2;
    let y0 = area.y + area.height.saturating_sub(alto) / 2;

    let mut x_texto = x0;
    if con_celular {
        celular(app, f, x0, y0);
        x_texto = x0 + ancho_celular + SEPARACION;
    }
    let y_texto = y0 + alto.saturating_sub(ALTO_TEXTO.min(alto)) / 2;
    texto(
        app,
        f,
        Rect::new(x_texto, y_texto, ANCHO_TEXTO, ALTO_TEXTO.min(alto)),
    );
}

/// El celular: bordes tenues, el mosaico «Enviar archivos» en acento y una
/// sombra a la derecha y abajo.
fn celular(app: &App, f: &mut Frame, x: u16, y: u16) {
    let p = &app.paleta;
    let linea = Style::new().fg(p.muted);
    let hora = chrono::Local::now().format("%H:%M").to_string();
    for (i, l) in dibujo::CELULAR.iter().enumerate() {
        let mut l = l.replace(dibujo::HORA, &hora);
        if i == 3 {
            // Cabecera de la app: el nombre de este equipo.
            let nombre = pad(&truncate(&app.equipo, 24), 24);
            l = format!("│ ≡  {nombre}⋮ │");
        }
        let fila = y + i as u16;
        put(f, x, fila, vec![Span::styled(l, linea)]);
        if i > 0 {
            put(
                f,
                x + dibujo::ANCHO,
                fila,
                vec![Span::styled("▒", Style::new().fg(p.dim))],
            );
        }
    }
    put(
        f,
        x + 1,
        y + dibujo::ALTO,
        vec![Span::styled(
            "▀".repeat(dibujo::ANCHO as usize),
            Style::new().fg(p.dim),
        )],
    );
    let (fila0, filas, col, ancho) = dibujo::MOSAICO;
    for i in 0..filas {
        let l = dibujo::CELULAR[fila0 + i];
        let tramo: String = l.chars().skip(col).take(ancho).collect();
        put(
            f,
            x + col as u16,
            y + (fila0 + i) as u16,
            vec![Span::styled(
                tramo,
                Style::new().fg(p.accent).add_modifier(Modifier::BOLD),
            )],
        );
    }
}

/// Parte `texto` en tramos y pone en `fuerte` las `palabras` que aparecen.
fn resaltar<'a>(texto: &str, palabras: &[&str], base: Style, fuerte: Style) -> Vec<Span<'a>> {
    let mut marcas: Vec<(usize, usize)> = palabras
        .iter()
        .filter(|w| !w.is_empty())
        .filter_map(|w| texto.find(w).map(|i| (i, i + w.len())))
        .collect();
    marcas.sort();
    let mut out = vec![];
    let mut desde = 0;
    for (a, b) in marcas {
        if a < desde {
            continue;
        }
        out.push(Span::styled(texto[desde..a].to_string(), base));
        out.push(Span::styled(texto[a..b].to_string(), fuerte));
        desde = b;
    }
    out.push(Span::styled(texto[desde..].to_string(), base));
    out
}

fn texto(app: &mut App, f: &mut Frame, r: Rect) {
    let p = app.paleta.clone();
    let fg = Style::new().fg(p.fg);
    let tenue = Style::new().fg(p.muted);
    let fuerte = Style::new().fg(p.bright).add_modifier(Modifier::BOLD);
    let w = r.width as usize;
    let alto = r.height;
    let mut y = r.y;
    let fila = |f: &mut Frame, y: &mut u16, spans: Vec<Span>| {
        if *y < r.y + alto {
            put(f, r.x, *y, spans);
        }
        *y += 1;
    };

    fila(f, &mut y, vec![Span::styled(t("titulo"), fuerte)]);
    fila(f, &mut y, vec![Span::styled(t("subtitulo"), tenue)]);
    y += 1;

    let (punto, color, msg) = match app.situacion() {
        Situacion::SinKde => ("○", p.warn, t("estado.sin_kde")),
        Situacion::Buscando => ("○", p.muted, t("estado.buscando")),
        Situacion::Ninguno => ("○", p.warn, t("estado.ninguno")),
        Situacion::Conectado(n) => (
            "●",
            p.ok,
            tf("estado.conectado", &[("nombres", &n.join(", "))]),
        ),
        Situacion::FueraDeAlcance(n) => ("○", p.warn, tf("estado.fuera", &[("nombre", &n)])),
        Situacion::Solicitado(n) => ("○", p.warn, tf("estado.solicitado", &[("nombre", &n)])),
        Situacion::SinVincular(n) => ("○", p.warn, tf("estado.nuevo", &[("nombre", &n)])),
    };
    fila(
        f,
        &mut y,
        vec![
            Span::styled(format!("{punto} "), Style::new().fg(color)),
            Span::styled(
                truncate(&msg, w - 2),
                if punto == "●" { fuerte } else { fg },
            ),
        ],
    );
    y += 1;

    // Los tres pasos.
    let kde = t("paso.kde");
    let accion = t("paso.accion");
    let pasos = [
        (tf("paso.1", &[("kde", &kde)]), vec![kde.clone()]),
        (
            tf("paso.2", &[("equipo", &app.equipo)]),
            vec![app.equipo.clone()],
        ),
        (tf("paso.3", &[("accion", &accion)]), vec![accion.clone()]),
    ];
    for (n, (txt, marcar)) in pasos.iter().enumerate() {
        let palabras: Vec<&str> = marcar.iter().map(String::as_str).collect();
        let mut spans = vec![Span::styled(
            format!("{}  ", n + 1),
            Style::new().fg(p.accent),
        )];
        spans.extend(resaltar(txt, &palabras, fg, fuerte));
        fila(f, &mut y, spans);
    }
    let carpeta = app.destino.to_string_lossy().replace(
        &lizarbe_core::paths::home().to_string_lossy().to_string(),
        "~",
    );
    let llegan = truncate(&tf("llegan", &[("carpeta", &carpeta)]), w);
    fila(f, &mut y, resaltar(&llegan, &[&carpeta], tenue, fg));
    y += 1;

    // Lo recibido (o la espera).
    if app.conectado() || !app.recibidos.is_empty() {
        fila(
            f,
            &mut y,
            vec![
                Span::styled(t("recibidos"), tenue),
                Span::styled(format!("  {}", app.recibidos.len()), Style::new().fg(p.dim)),
            ],
        );
        let sitio = if alto >= 22 { 5 } else { 3 };
        if app.recibidos.is_empty() {
            const GIRO: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            fila(
                f,
                &mut y,
                vec![
                    Span::styled(
                        format!("{} ", GIRO[app.pulso() % GIRO.len()]),
                        Style::new().fg(p.accent),
                    ),
                    Span::styled(t("esperando"), tenue),
                ],
            );
            y += sitio as u16 - 1;
        } else {
            let desde = app.recibidos.len().saturating_sub(sitio);
            for rec in app.recibidos[desde..].iter().rev() {
                let tam = format!("  {}", rec.tam);
                let hueco = w.saturating_sub(rec.hora.width() + 4 + tam.width());
                fila(
                    f,
                    &mut y,
                    vec![
                        Span::styled(format!("{}  ", rec.hora), Style::new().fg(p.dim)),
                        Span::styled("↓ ", Style::new().fg(p.ok)),
                        Span::styled(truncate(&rec.nombre, hueco), fuerte),
                        Span::styled(tam, tenue),
                    ],
                );
            }
            y += (sitio - app.recibidos.len().min(sitio)) as u16;
        }
        y += 1;
    }

    // Opciones.
    for op in app.opciones() {
        let etiqueta = t(match op {
            Opcion::Avisar => "op.avisar",
            Opcion::Descargas => "op.descargas",
            Opcion::Kde => "op.kde",
            Opcion::Cerrar => "op.cerrar",
        });
        let activa = app.sel == op;
        let encima = app.hover == Some(op);
        if y < r.y + alto {
            app.hits.push((Rect::new(r.x, y, r.width, 1), op));
        }
        let (marca, estilo) = if activa {
            ("› ", Style::new().fg(p.accent).add_modifier(Modifier::BOLD))
        } else if encima {
            ("  ", Style::new().fg(p.bright))
        } else {
            ("  ", fg)
        };
        fila(
            f,
            &mut y,
            vec![
                Span::styled(marca, Style::new().fg(p.accent)),
                Span::styled(etiqueta, estilo),
            ],
        );
    }
    y += 1;

    // Pie: el último aviso o la ayuda.
    let pie = match &app.aviso {
        Some(a) if a.bien => vec![
            Span::styled("✓ ", Style::new().fg(p.ok)),
            Span::styled(truncate(&a.texto, w - 2), fg),
        ],
        Some(a) => vec![
            Span::styled("! ", Style::new().fg(p.warn)),
            Span::styled(truncate(&a.texto, w - 2), fg),
        ],
        None => vec![Span::styled(
            truncate(&t("ayuda"), w),
            Style::new().fg(p.dim),
        )],
    };
    fila(f, &mut y, pie);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn pantalla(app: &mut App, w: u16, h: u16) -> Vec<String> {
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| dibujar(app, f)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn full_window_shows_phone_status_files_and_options() {
        let mut app = App::new(true);
        let txt = pantalla(&mut app, 110, 40).join("\n");
        for esperado in [
            "lizarbe",
            "Enviar",
            "Conectado · Galaxy A06",
            "foto_vacaciones.jpg",
            "Avisar al teléfono",
            "Abrir Descargas",
            "q salir",
        ] {
            assert!(txt.contains(esperado), "falta «{esperado}»:\n{txt}");
        }
        assert_eq!(app.hits.len(), 4);
    }

    #[test]
    fn small_window_drops_the_phone_but_keeps_the_text() {
        let mut app = App::new(true);
        let txt = pantalla(&mut app, 60, 26).join("\n");
        assert!(!txt.contains("Algunos complementos"));
        assert!(txt.contains("Recibir archivos") && txt.contains("Cerrar"));
    }

    #[test]
    fn tiny_window_only_says_so() {
        let mut app = App::new(true);
        let txt = pantalla(&mut app, 30, 8).join("\n");
        assert!(txt.contains("demasiado pequeña"));
    }
}
