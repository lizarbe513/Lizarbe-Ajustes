//! Capítulo 4: conectar el teléfono. QR para descargar KDE Connect, pasos,
//! estado del cortafuegos y teléfonos detectados en vivo.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::app::{App, Hit};
use crate::brand::*;
use crate::contenido::TIENDAS;
use crate::i18n::t;
use crate::qr;
use crate::sistema::Estado;
use crate::ui::{Boton, Marco, boton, marco, parrafo, texto, titulo_capitulo};
use lizarbe_core::ui::{put, truncate};

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.telefono");
    let tt = app.cap_t();
    let y0 = r.y + h0 + 1;

    // Columna del QR: la página de descargas siempre; los códigos largos solo si caben.
    let alto_disponible = r.bottom().saturating_sub(y0 + 2);
    let (_, url) = TIENDAS[app.qr_sel.min(TIENDAS.len() - 1)];
    let lineas = qr::lineas(url, INK, PHOSPHOR);
    let cabe = lineas.len() as u16 + 3 <= alto_disponible;
    let qr_w = if cabe { qr::ancho(url) as u16 } else { 0 };
    let col_qr = if cabe { qr_w.max(36) } else { 36 };
    if cabe {
        let x = r.x + (col_qr - qr_w) / 2;
        for (i, l) in lineas.iter().enumerate() {
            put(f, x, y0 + i as u16, l.spans.clone());
        }
        // Selector de tienda.
        let ys = y0 + lineas.len() as u16;
        let mut xs = r.x;
        for (i, (clave, _)) in TIENDAS.iter().enumerate() {
            let etiqueta = format!(" {} ", t(clave));
            let st = if i == app.qr_sel {
                bold(PHOSPHOR)
            } else if app.hover == Some(Hit::Qr(i)) {
                bold(CADMIUM)
            } else {
                fg(SLATE)
            };
            let w = texto(f, xs, ys, &etiqueta, st);
            app.hits.push((Rect::new(xs, ys, w, 1), Hit::Qr(i)));
            xs += w + 1;
        }
        texto(f, r.x, ys + 1, &truncate(url, col_qr as usize), fg(STEEL));
    } else {
        let caja = Rect::new(r.x, y0, col_qr, 8);
        marco(f, caja, Marco::Fino, STEEL, "");
        parrafo(
            f,
            caja.x + 2,
            caja.y + 2,
            caja.width - 4,
            &t("tel.ampliar"),
            fg(SLATE),
        );
    }

    // Columna de pasos y teléfonos.
    let x = r.x + col_qr + 3;
    let w = r.width.saturating_sub(col_qr + 3);
    if w < 30 {
        return;
    }
    let mut y = y0;
    for n in 1..=3 {
        put(
            f,
            x,
            y,
            vec![
                Span::styled(format!("{n}  "), bold(RED)),
                Span::styled(t(&format!("tel.paso{n}")), fg(PHOSPHOR)),
            ],
        );
        let filas = parrafo(
            f,
            x + 3,
            y + 1,
            w.saturating_sub(3),
            &t(&format!("tel.paso{n}.d")),
            fg(SLATE),
        );
        y += 1 + filas + 1;
    }

    // Avisos del sistema.
    if !app.kde_instalado {
        y += parrafo(
            f,
            x,
            y,
            w,
            &format!("▲ {}", t("tel.no_instalado")),
            bold(CADMIUM),
        );
    } else if !app.cortafuegos {
        y += parrafo(
            f,
            x,
            y,
            w,
            &format!("▲ {}", t("tel.cortafuegos")),
            bold(CADMIUM),
        );
        texto(f, x, y, &t("tel.abrir_puertos"), fg(PHOSPHOR));
        y += 1;
    }
    y += 1;

    // Teléfonos cercanos.
    let alto_caja = r.bottom().saturating_sub(y + 2).clamp(4, 7);
    let caja = Rect::new(x, y, w, alto_caja);
    marco(f, caja, Marco::Fino, STEEL, &t("tel.cercanos"));
    if app.telefonos.is_empty() {
        let giro = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(tt * 10.0) as usize % 10];
        texto(
            f,
            caja.x + 2,
            caja.y + 1,
            &format!("{giro} {}", t("tel.buscando")),
            fg(SLATE),
        );
    }
    let n_max = alto_caja.saturating_sub(2) as usize;
    let telefonos = app.telefonos.clone();
    for (i, p) in telefonos.iter().take(n_max).enumerate() {
        let sel = i == app.tel_sel;
        let (marca, color) = match (&p.estado, p.alcanzable) {
            (Estado::Vinculado, true) => (SPARK, GREEN),
            (Estado::Vinculado, false) => ("○", SLATE),
            (Estado::Solicitado, _) => ("◌", CADMIUM),
            (Estado::Nuevo, _) => ("●", COBALT),
        };
        let estado = match (&p.estado, p.alcanzable) {
            (Estado::Vinculado, true) => t("tel.est.conectado"),
            (Estado::Vinculado, false) => t("tel.est.lejos"),
            (Estado::Solicitado, _) => t("tel.est.solicitado"),
            (Estado::Nuevo, _) => t("tel.est.disponible"),
        };
        let yy = caja.y + 1 + i as u16;
        put(
            f,
            caja.x + 2,
            yy,
            vec![
                Span::styled(if sel { "▍" } else { " " }, bold(RED)),
                Span::styled(format!("{marca} "), bold(color)),
                Span::styled(
                    truncate(&p.nombre, 24),
                    if sel { bold(PHOSPHOR) } else { fg(SILVER) },
                ),
                Span::styled(format!("  {estado}"), fg(SLATE)),
            ],
        );
        app.hits
            .push((Rect::new(caja.x + 1, yy, caja.width - 2, 1), Hit::Fila(i)));
    }

    // Acciones y avisos.
    let ya = r.bottom().saturating_sub(2);
    if !app.tel_aviso.is_empty() {
        texto(
            f,
            x,
            ya,
            &truncate(&app.tel_aviso, w as usize),
            bold(CADMIUM),
        );
    } else if app.tiene("telefono") {
        texto(
            f,
            x,
            ya,
            &format!("{SPARK} {}", t("tel.logrado")),
            bold(GREEN),
        );
    }
    let mut xb = x;
    for (tecla_, clave, accion) in [
        ("P", "btn.vincular", 'p'),
        ("R", "btn.sonar", 'r'),
        ("A", "btn.abrir_app", 'a'),
    ] {
        let etiqueta = t(clave);
        if xb + etiqueta.chars().count() as u16 + 8 > r.right() {
            break;
        }
        xb += boton(app, f, xb, ya + 1, Boton::new(tecla_, &etiqueta, accion)) + 3;
    }
}
