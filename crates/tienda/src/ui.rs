//! Dibujo de la tienda: categorías, lista de apps, detalle y ventana de confirmación.

use lizarbe_core::ui::{
    Hint, button_spans, button_width, centered, fg, frame, list_row, pad, put, row_style, rule_h,
    rule_v, status_bar, truncate, wrap,
};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Span;

use crate::app::{App, Foco, Hit, Vista};
use crate::catalogo::Fuente;
use crate::i18n::t;

const ANCHO_VISTAS: u16 = 24;
const ANCHO_DETALLE: u16 = 40;
const ALTO_DETALLE_ESTRECHO: u16 = 9;

impl App {
    pub fn dibujar(&mut self, f: &mut Frame) {
        self.hits.clear();
        let pal = self.pal.clone();
        let titulo = match self.fuente {
            Fuente::Repos => t("title.repos"),
            Fuente::Aur => t("title.aur"),
        };
        let inner = frame(f, &pal, f.area(), &titulo);
        if inner.width < 30 || inner.height < 8 {
            return;
        }
        self.dibujar_busqueda(f, inner);
        rule_h(f, inner.x, inner.y + 1, inner.width, pal.rule);

        let cuerpo = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 3);
        let ancho_detalle = if inner.width >= 100 { ANCHO_DETALLE } else { 0 };
        let vistas_r = Rect::new(cuerpo.x, cuerpo.y, ANCHO_VISTAS, cuerpo.height);
        let resto_x = cuerpo.x + ANCHO_VISTAS + 1;
        let resto_w = cuerpo.width.saturating_sub(ANCHO_VISTAS + 1);
        rule_v(
            f,
            cuerpo.x + ANCHO_VISTAS,
            cuerpo.y,
            cuerpo.height,
            pal.rule,
        );

        let (lista_r, detalle_r) = if ancho_detalle > 0 {
            let lw = resto_w - ancho_detalle - 1;
            rule_v(f, resto_x + lw, cuerpo.y, cuerpo.height, pal.rule);
            (
                Rect::new(resto_x, cuerpo.y, lw, cuerpo.height),
                Rect::new(resto_x + lw + 2, cuerpo.y, ancho_detalle - 1, cuerpo.height),
            )
        } else {
            let dh = ALTO_DETALLE_ESTRECHO.min(cuerpo.height / 2);
            rule_h(f, resto_x, cuerpo.bottom() - dh - 1, resto_w, pal.rule);
            (
                Rect::new(resto_x, cuerpo.y, resto_w, cuerpo.height - dh - 1),
                Rect::new(resto_x + 1, cuerpo.bottom() - dh, resto_w - 1, dh),
            )
        };

        self.dibujar_vistas(f, vistas_r);
        self.dibujar_lista(f, lista_r);
        self.dibujar_detalle(f, detalle_r);
        self.dibujar_barra(f, Rect::new(inner.x, inner.bottom() - 1, inner.width, 1));
        if self.modal.is_some() {
            self.dibujar_modal(f);
        }
    }

    fn dibujar_busqueda(&mut self, f: &mut Frame, inner: Rect) {
        let pal = &self.pal;
        let activo = self.foco == Foco::Busqueda;
        let mut spans = vec![Span::styled(
            " 󰍉  ",
            fg(if activo { pal.accent } else { pal.muted }),
        )];
        if self.consulta.is_empty() && !activo {
            spans.push(Span::styled(t("search.hint"), fg(pal.dim)));
        } else {
            spans.push(Span::styled(self.consulta.clone(), fg(pal.bright)));
            if activo {
                spans.push(Span::styled("▏", fg(pal.accent)));
            }
        }
        put(f, inner.x, inner.y, spans);
        self.hits
            .push((Rect::new(inner.x, inner.y, inner.width, 1), Hit::Buscar));
    }

    fn nombre_vista(&self, v: Vista) -> (String, String) {
        match v {
            Vista::Recomendadas => ("󰓎".into(), t("cat.recomendadas")),
            Vista::Instaladas => ("󰄬".into(), t("cat.instaladas")),
            Vista::Cat(i) => {
                let c = &self.cat.categorias[i];
                (c.icono.clone(), c.nombre.get().to_string())
            }
        }
    }

    fn dibujar_vistas(&mut self, f: &mut Frame, r: Rect) {
        let pal = self.pal.clone();
        let buscando = !self.consulta.trim().is_empty();
        for (i, v) in self.vistas.clone().into_iter().enumerate() {
            let y = r.y + i as u16;
            if y >= r.bottom() {
                break;
            }
            let (icono, nombre) = self.nombre_vista(v);
            let sel = i == self.vista && !buscando;
            let hover = self.hover == Some(Hit::Vista(i));
            let texto = format!("{icono}  {nombre}");
            let mut spans = list_row(&pal, &texto, r.width as usize, sel, hover, true);
            if sel && self.foco != Foco::Vistas {
                spans[1].style = spans[1].style.remove_modifier(Modifier::BOLD).fg(pal.muted);
            }
            put(f, r.x, y, spans);
            self.hits
                .push((Rect::new(r.x, y, r.width, 1), Hit::Vista(i)));
        }
    }

    fn dibujar_lista(&mut self, f: &mut Frame, r: Rect) {
        let pal = self.pal.clone();
        let visibles = self.visibles();
        self.list_h = r.height as usize;
        let h = self.list_h.max(1);
        self.fila = self.fila.min(visibles.len().saturating_sub(1));
        if self.fila < self.scroll {
            self.scroll = self.fila;
        } else if self.fila >= self.scroll + h {
            self.scroll = self.fila + 1 - h;
        }
        if visibles.is_empty() {
            let msg = if self.consulta.trim().is_empty() {
                t("list.empty")
            } else {
                crate::i18n::tf("list.no_results", &[("q", self.consulta.trim())])
            };
            put(
                f,
                r.x + 2,
                r.y + 1,
                vec![Span::styled(
                    truncate(&msg, r.width as usize - 2),
                    fg(pal.muted),
                )],
            );
            if !self.consulta.trim().is_empty() {
                put(
                    f,
                    r.x + 2,
                    r.y + 3,
                    vec![Span::styled(
                        truncate(&t("list.try_advanced"), r.width as usize - 2),
                        fg(pal.dim),
                    )],
                );
            }
            return;
        }
        let ancho_nombre = (r.width as usize / 3).clamp(14, 24);
        for (pos, a) in visibles.iter().enumerate().skip(self.scroll).take(h) {
            let y = r.y + (pos - self.scroll) as u16;
            let app = &self.cat.apps[*a];
            let instalada = self.instalada(*a);
            let marcada = self.marcadas.contains(a);
            let sel = pos == self.fila;
            let hover = self.hover == Some(Hit::Fila(pos));
            let estilo = row_style(&pal, hover, sel && self.foco != Foco::Vistas, true);
            let (glifo, color) = if instalada {
                ("✓", pal.ok)
            } else if marcada {
                ("●", pal.accent)
            } else {
                (" ", pal.dim)
            };
            let resto = r.width as usize - ancho_nombre - 5;
            let spans = vec![
                Span::styled(
                    if sel { "▍" } else { " " },
                    fg(pal.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{glifo} "), fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(pad(&app.nombre, ancho_nombre), estilo),
                Span::styled(
                    format!("  {}", pad(app.resumen.get(), resto)),
                    fg(pal.muted),
                ),
            ];
            put(f, r.x, y, spans);
            self.hits
                .push((Rect::new(r.x, y, r.width, 1), Hit::Fila(pos)));
        }
        if visibles.len() > h {
            let n = format!("{}/{}", self.fila + 1, visibles.len());
            put(
                f,
                r.right().saturating_sub(n.len() as u16 + 1),
                r.bottom() - 1,
                vec![Span::styled(n, fg(pal.dim))],
            );
        }
    }

    fn dibujar_detalle(&mut self, f: &mut Frame, r: Rect) {
        let pal = self.pal.clone();
        let Some(a) = self.actual() else { return };
        let app = self.cat.apps[a].clone();
        let instalada = self.instalada(a);
        let info = self.info(a);
        let w = r.width as usize;
        let y = std::cell::Cell::new(r.y);
        let linea = |f: &mut Frame, spans: Vec<Span<'static>>| {
            if y.get() < r.bottom().saturating_sub(2) {
                put(f, r.x, y.get(), spans);
            }
            y.set(y.get() + 1);
        };
        linea(
            f,
            vec![Span::styled(
                truncate(&app.nombre, w),
                fg(pal.bright).add_modifier(Modifier::BOLD),
            )],
        );
        let estado = if instalada {
            ("✓ ", t("detail.installed"), pal.ok)
        } else {
            ("  ", t("detail.not_installed"), pal.muted)
        };
        linea(
            f,
            vec![Span::styled(
                format!("{}{}", estado.0, estado.1),
                fg(estado.2),
            )],
        );
        y.set(y.get() + 1);
        for l in wrap(app.descripcion.get(), w) {
            linea(f, vec![Span::styled(l, fg(pal.fg))]);
        }
        y.set(y.get() + 1);
        let dato = |f: &mut Frame, k: &str, v: &str| {
            if !v.is_empty() {
                linea(
                    f,
                    vec![
                        Span::styled(pad(&t(k), 12), fg(pal.muted)),
                        Span::styled(truncate(v, w.saturating_sub(12)), fg(pal.fg)),
                    ],
                );
            }
        };
        if app.fuente == Fuente::Aur {
            dato(f, "detail.repo", &t("source.aur"));
        }
        if let Some(i) = &info {
            dato(f, "detail.repo", &i.repo);
            dato(f, "detail.version", &i.version);
            dato(f, "detail.download", &i.descarga);
            dato(f, "detail.size", &i.instalado);
        }
        dato(f, "detail.packages", &app.paquetes.join(", "));
        if instalada && !app.quitar {
            y.set(y.get() + 1);
            linea(
                f,
                vec![Span::styled(truncate(&t("msg.protected"), w), fg(pal.warn))],
            );
        }

        // Botones en la última fila del panel.
        let by = r.bottom() - 1;
        let mut bx = r.x;
        let mut boton = |f: &mut Frame,
                         hits: &mut Vec<(Rect, Hit)>,
                         hover: Option<Hit>,
                         key: &str,
                         label: String,
                         hit: Hit,
                         primary: bool| {
            let bw = button_width(key, &label) as u16;
            if bx + bw > r.right() {
                return;
            }
            put(
                f,
                bx,
                by,
                button_spans(&pal, key, &label, true, primary, hover == Some(hit)),
            );
            hits.push((Rect::new(bx, by, bw, 1), hit));
            bx += bw + 2;
        };
        let hover = self.hover;
        if instalada {
            if app.desktop.is_some() {
                boton(
                    f,
                    &mut self.hits,
                    hover,
                    "o",
                    t("btn.open"),
                    Hit::Abrir,
                    true,
                );
            }
            if app.quitar {
                boton(
                    f,
                    &mut self.hits,
                    hover,
                    "x",
                    t("btn.remove"),
                    Hit::Quitar,
                    false,
                );
            }
        } else {
            let n = self.marcadas.len();
            let label = if n > 1 {
                crate::i18n::tf("btn.install_n", &[("n", &n.to_string())])
            } else {
                t("btn.install")
            };
            boton(f, &mut self.hits, hover, "⏎", label, Hit::Instalar, true);
        }
        boton(
            f,
            &mut self.hits,
            hover,
            "a",
            t("btn.advanced"),
            Hit::Avanzado,
            false,
        );
    }

    fn dibujar_barra(&mut self, f: &mut Frame, r: Rect) {
        let (texto, negrita) = match (&self.aviso, self.fuente) {
            (Some((m, ok, _)), _) => (m.clone(), *ok),
            (None, Fuente::Aur) => (t("status.aur"), false),
            (None, Fuente::Repos) => (String::new(), false),
        };
        let h = |k: &str, d: &str, p: u8| -> Hint { (k.into(), t(d), p) };
        let hints = vec![
            h("↑↓", "hint.move", 3),
            h("␣", "hint.mark", 2),
            h("⏎", "hint.install", 3),
            h("x", "hint.remove", 1),
            h("/", "hint.search", 2),
            h("a", "hint.advanced", 0),
            h("q", "hint.quit", 3),
        ];
        let pal = self.pal.clone();
        status_bar(f, &pal, r, &texto, negrita, &hints);
    }

    fn dibujar_modal(&mut self, f: &mut Frame) {
        let pal = self.pal.clone();
        let Some(m) = self.modal.clone() else { return };
        let w = 62u16;
        let mut lineas: Vec<String> = vec![];
        for l in &m.lineas {
            lineas.extend(wrap(l, w as usize - 4));
        }
        let h = (lineas.len() as u16 + 5).min(f.area().height.saturating_sub(2));
        let r = centered(f.area(), w, h);
        let inner = frame(f, &pal, r, &m.titulo);
        for (i, l) in lineas.iter().enumerate() {
            if (i as u16) < inner.height.saturating_sub(2) {
                put(
                    f,
                    inner.x + 1,
                    inner.y + 1 + i as u16,
                    vec![Span::styled(l.clone(), fg(pal.fg))],
                );
            }
        }
        let by = inner.bottom() - 1;
        let ok = t("modal.ok");
        let no = t("modal.cancel");
        let total = (button_width("⏎", &ok) + 3 + button_width("Esc", &no)) as u16;
        let mut x = inner.right().saturating_sub(total + 1);
        for (key, label, hit, primary) in
            [("⏎", ok, Hit::Ok, true), ("Esc", no, Hit::Cancelar, false)]
        {
            let bw = button_width(key, &label) as u16;
            put(
                f,
                x,
                by,
                button_spans(&pal, key, &label, true, primary, self.hover == Some(hit)),
            );
            self.hits.push((Rect::new(x, by, bw, 1), hit));
            x += bw + 3;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogo::Catalogo;
    use crate::sistema::Estado;
    use lizarbe_core::theme::Palette;
    use ratatui::{Terminal, backend::TestBackend};

    fn app() -> App {
        let cat = Catalogo::embebido();
        let mut estado = Estado::default();
        for a in &cat.apps {
            estado.disponibles.extend(a.paquetes.iter().cloned());
        }
        estado.instalados.insert("firefox".into());
        App::new(
            cat,
            estado,
            Palette::load(std::path::Path::new("/nonexistent")),
            Fuente::Repos,
        )
    }

    fn app_aur() -> App {
        let mut a = app();
        a.fuente = Fuente::Aur;
        App::new(a.cat, a.estado, a.pal, Fuente::Aur)
    }

    fn pantalla(app: &mut App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| app.dibujar(f)).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn draws_wide_narrow_tiny_and_modal() {
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        let mut a = app();
        let ancha = pantalla(&mut a, 130, 30);
        assert!(ancha.contains("Firefox") && ancha.contains("Recomendadas"));
        println!("{ancha}");
        assert!(pantalla(&mut a, 80, 24).contains("Firefox"));
        pantalla(&mut a, 20, 5);
        a.fila = 2;
        a.pedir_instalar_para_prueba();
        let m = pantalla(&mut a, 130, 30);
        assert!(m.contains("INSTALAR 1 APLICACI"));
        println!("{m}");
    }

    #[test]
    fn aur_mode_shows_only_aur_apps_and_its_note() {
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        let mut a = app_aur();
        let nombres: Vec<String> = a
            .visibles()
            .iter()
            .map(|i| a.cat.apps[*i].id.clone())
            .collect();
        assert!(nombres.contains(&"onlyoffice".to_string()));
        assert!(!nombres.contains(&"firefox".to_string()));
        assert!(
            a.vistas.len() < app().vistas.len(),
            "categorías vacías ocultas"
        );
        let p = pantalla(&mut a, 130, 30);
        assert!(p.contains("AUR") && p.contains("ONLYOFFICE") || p.contains("OnlyOffice"));
        println!("{p}");
        a.pedir_instalar_para_prueba();
        assert!(pantalla(&mut a, 130, 30).contains("compil"));
        let mut r = app();
        let repos: Vec<String> = r
            .visibles()
            .iter()
            .map(|i| r.cat.apps[*i].id.clone())
            .collect();
        assert!(!repos.contains(&"onlyoffice".to_string()));
        pantalla(&mut r, 100, 24);
    }
}
