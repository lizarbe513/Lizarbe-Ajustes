//! Capítulo 7: conceptos clave, con un dibujo animado por concepto.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::Span;

use crate::anim;
use crate::app::{App, Hit};
use crate::brand::*;
use crate::contenido::{CONCEPTOS, Dibujo};
use crate::i18n::t;
use crate::ui::{Marco, marco, parrafo, texto, titulo_capitulo};
use lizarbe_core::ui::{center, put, truncate};

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.conceptos");
    let tt = app.cap_t();
    let y0 = r.y + h0 + 1;
    let lista_w = 30u16.min(r.width * 2 / 5);

    for (i, c) in CONCEPTOS.iter().enumerate() {
        let y = y0 + i as u16 * 2;
        if y >= r.bottom() {
            break;
        }
        let sel = i == app.concepto_sel;
        put(
            f,
            r.x,
            y,
            vec![
                Span::styled(if sel { "▍ " } else { "  " }, bold(rojo())),
                Span::styled(format!("{}  ", i + 1), fg(pizarra())),
                Span::styled(
                    truncate(
                        &t(&format!("con.{}", c.id)),
                        lista_w.saturating_sub(7) as usize,
                    ),
                    if sel { bold(fosforo()) } else { fg(plata()) },
                ),
            ],
        );
        app.hits.push((Rect::new(r.x, y, lista_w, 2), Hit::Fila(i)));
    }

    let c = &CONCEPTOS[app.concepto_sel.min(CONCEPTOS.len() - 1)];
    let x = r.x + lista_w + 3;
    let w = r.width.saturating_sub(lista_w + 3);
    if w < 36 {
        return;
    }
    texto(
        f,
        x,
        y0,
        &t(&format!("con.{}", c.id)).to_uppercase(),
        bold(fosforo()),
    );
    let mut y = y0 + 2;
    y += parrafo(f, x, y, w, &t(&format!("con.{}.1", c.id)), fg(plata())) + 1;
    y += parrafo(f, x, y, w, &t(&format!("con.{}.2", c.id)), fg(pizarra())) + 1;
    let alto = r.bottom().saturating_sub(y);
    if alto >= 7 {
        dibujo(f, c.dibujo, Rect::new(x, y, w.min(64), alto.min(10)), tt);
    }
}

fn etiqueta(f: &mut Frame, r: Rect, texto_: &str, color: Color, relleno: bool) {
    marco(f, r, Marco::Fino, color, "");
    let st = if relleno { bold(color) } else { fg(plata()) };
    texto(
        f,
        r.x + 1,
        r.y + r.height / 2,
        &center(texto_, r.width.saturating_sub(2) as usize),
        st,
    );
}

fn dibujo(f: &mut Frame, d: Dibujo, r: Rect, tt: f32) {
    match d {
        Dibujo::Escritorios => {
            let activo = (tt * 0.8) as usize % 5;
            for i in 0..5usize {
                let x = r.x + i as u16 * 9;
                if x + 8 > r.right() {
                    break;
                }
                let caja = Rect::new(x, r.y, 8, 4);
                let on = i == activo;
                marco(
                    f,
                    caja,
                    if on { Marco::Grueso } else { Marco::Fino },
                    if on { rojo() } else { pizarra() },
                    "",
                );
                texto(
                    f,
                    x + 3,
                    r.y + 1,
                    &(i + 1).to_string(),
                    if on { bold(fosforo()) } else { fg(pizarra()) },
                );
                if i % 2 == 0 {
                    texto(
                        f,
                        x + 2,
                        r.y + 2,
                        "▪▪",
                        fg(if on { fosforo() } else { acero() }),
                    );
                }
            }
            texto(
                f,
                r.x,
                r.y + 5,
                &format!("{}  {}", crate::brand::SPARK, "Super + 1 … 5"),
                fg(cadmio()),
            );
        }
        Dibujo::Mosaico => {
            let etapa = (tt / 1.8) as usize % 3;
            let (w, h) = (r.width.min(48), r.height.min(8));
            let area = Rect::new(r.x, r.y, w, h);
            marco(f, area, Marco::Fino, acero(), "");
            let i = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
            match etapa {
                0 => etiqueta(f, i, "Terminal", rojo(), true),
                1 => {
                    let mitad = i.width / 2;
                    etiqueta(
                        f,
                        Rect::new(i.x, i.y, mitad, i.height),
                        "Terminal",
                        pizarra(),
                        false,
                    );
                    etiqueta(
                        f,
                        Rect::new(i.x + mitad, i.y, i.width - mitad, i.height),
                        "Navegador",
                        rojo(),
                        true,
                    );
                }
                _ => {
                    let mitad = i.width / 2;
                    let alto = i.height / 2;
                    etiqueta(
                        f,
                        Rect::new(i.x, i.y, mitad, i.height),
                        "Terminal",
                        pizarra(),
                        false,
                    );
                    etiqueta(
                        f,
                        Rect::new(i.x + mitad, i.y, i.width - mitad, alto),
                        "Navegador",
                        pizarra(),
                        false,
                    );
                    etiqueta(
                        f,
                        Rect::new(i.x + mitad, i.y + alto, i.width - mitad, i.height - alto),
                        "Archivos",
                        rojo(),
                        true,
                    );
                }
            }
        }
        Dibujo::Menu => {
            let items = [
                "Apps", "Learn", "Trigger", "Style", "Setup", "Install", "Remove", "Update",
                "About", "System",
            ];
            let activo = (tt * 1.4) as usize % items.len();
            for (i, it) in items.iter().enumerate() {
                let (col, fila) = (i / 5, i % 5);
                let on = i == activo;
                put(
                    f,
                    r.x + col as u16 * 18,
                    r.y + fila as u16,
                    vec![
                        Span::styled(if on { "▍ " } else { "  " }, bold(rojo())),
                        Span::styled(
                            it.to_string(),
                            if on { bold(fosforo()) } else { fg(plata()) },
                        ),
                    ],
                );
            }
            texto(f, r.x, r.y + 6, "Super + Espacio", bold(cadmio()));
        }
        Dibujo::Instalar => {
            let items = ["Tienda", "Package", "AUR", "Web App"];
            let activo = (tt * 1.1) as usize % items.len();
            for (i, it) in items.iter().enumerate() {
                let caja = Rect::new(r.x + i as u16 * 14, r.y, 12, 3);
                if caja.right() > r.right() {
                    break;
                }
                etiqueta(
                    f,
                    caja,
                    it,
                    if i == activo { rojo() } else { pizarra() },
                    i == activo,
                );
            }
            texto(f, r.x, r.y + 4, "Super + Espacio › Install", bold(cadmio()));
        }
        Dibujo::Actualizar => {
            let ciclo = (tt % 8.0) / 8.0;
            let fase = if ciclo < 0.25 {
                "update.f1"
            } else if ciclo < 0.85 {
                "update.f2"
            } else {
                "update.f3"
            };
            texto(f, r.x, r.y, &t(fase), bold(fosforo()));
            put(
                f,
                r.x,
                r.y + 2,
                vec![
                    Span::styled(anim::barra(ciclo, 36), fg(rojo())),
                    Span::styled(format!("  {:>3}%", (ciclo * 100.0) as u32), fg(plata())),
                ],
            );
            texto(f, r.x, r.y + 4, "omarchy update", bold(cadmio()));
        }
        Dibujo::Privacidad => {
            let parpadeo = anim::pulso(tt, 2.0) > 0.5;
            put(
                f,
                r.x,
                r.y,
                vec![
                    Span::styled(format!("{} ", t("priv.telemetria")), fg(plata())),
                    Span::styled("0 B", bold(verde())),
                ],
            );
            put(
                f,
                r.x,
                r.y + 1,
                vec![
                    Span::styled(format!("{} ", t("priv.rastreadores")), fg(plata())),
                    Span::styled("0", bold(verde())),
                ],
            );
            put(
                f,
                r.x,
                r.y + 2,
                vec![
                    Span::styled(format!("{} ", t("priv.antivirus")), fg(plata())),
                    Span::styled(t("priv.no_hace_falta"), bold(verde())),
                ],
            );
            texto(
                f,
                r.x,
                r.y + 4,
                if parpadeo {
                    "▮ listening: nothing"
                } else {
                    "▯ listening: nothing"
                },
                fg(pizarra()),
            );
        }
        Dibujo::Modular => {
            let piezas = ["CPU", "RAM", "SSD", "GPU"];
            let activo = (tt * 0.9) as usize % piezas.len();
            for (i, p) in piezas.iter().enumerate() {
                let caja = Rect::new(r.x + i as u16 * 13, r.y, 11, 3);
                if caja.right() > r.right() {
                    break;
                }
                etiqueta(
                    f,
                    caja,
                    p,
                    if i == activo { rojo() } else { pizarra() },
                    i == activo,
                );
            }
            texto(
                f,
                r.x,
                r.y + 4,
                &format!("{SPARK} {}", t("modular.cambiable")),
                fg(cadmio()),
            );
        }
    }
}
