//! Estado y lógica de la Bienvenida: capítulos, práctica en vivo, logros,
//! tareas en segundo plano y entrada de teclado y ratón.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use lizarbe_core::ansi;
use lizarbe_core::hypr_events::{HyprEvent, Listener};
use lizarbe_core::mouse::{self, Clicks, Mouse};
use lizarbe_core::term::{Command, TuiApp};
use lizarbe_core::theme::{Palette, PaletteWatcher};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::brand;
use crate::contenido::{CONCEPTOS, HERRAMIENTAS, LOGROS, TIENDAS, total_atajos};
use crate::i18n::{t, tf};
use crate::retos::{Estado as EstadoReto, Reto};
use crate::sistema::{self, Estado, Msg, Orden, Telefono};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    Arranque,
    Actualizar,
    Super,
    Practica,
    Terminal,
    Telefono,
    Escritorio,
    Configurar,
    Conceptos,
    Atajos,
    Final,
}

impl Cap {
    pub const ALL: [Cap; 11] = [
        Cap::Arranque,
        Cap::Actualizar,
        Cap::Super,
        Cap::Practica,
        Cap::Terminal,
        Cap::Telefono,
        Cap::Escritorio,
        Cap::Configurar,
        Cap::Conceptos,
        Cap::Atajos,
        Cap::Final,
    ];

    /// Pasos numerados del 1 al 8 (ni el arranque, ni «Actualizar», ni el final cuentan).
    pub const NUMERADOS: usize = 8;

    pub fn indice(self) -> usize {
        Cap::ALL.iter().position(|c| *c == self).unwrap()
    }

    /// Número de paso (1-8), si lo tiene.
    pub fn numero(self) -> Option<usize> {
        let i = self.indice();
        (2..=Cap::NUMERADOS + 1).contains(&i).then(|| i - 1)
    }

    /// Posición en `ALL` del paso numerado `n` (1-8).
    pub fn de_numero(n: usize) -> usize {
        n + 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Cap(usize),
    Anterior,
    Siguiente,
    /// Fila de la lista del capítulo actual.
    Fila(usize),
    /// Botón de acción del capítulo (la tecla que lo activa).
    Boton(char),
    Qr(usize),
}

/// Estado de la búsqueda de actualizaciones del sistema.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Buscando,
    AlDia,
    /// Líneas `paquete antes -> después`.
    Hay(Vec<String>),
    SinRed,
    Actualizado,
}

/// Comando externo en marcha (para saber qué hacer al volver).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tarea {
    Puertos,
    Actualizar,
}

/// La frase que dice qué hacer en el paso actual.
pub struct Guia {
    pub texto: String,
    /// El paso ya está cumplido y solo falta continuar.
    pub hecho: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    Ninguno,
    Salir,
    Ayuda,
}

pub struct App {
    pub cap: usize,
    pub cap_inicio: f32,
    /// Segundos desde que arrancó la aplicación (las pruebas lo fijan a mano).
    pub t: f32,
    t0: Instant,
    pub demo: bool,
    pub usuario: String,
    escuchador: Option<Listener>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pub logros: Vec<&'static str>,
    /// Aviso de logro: texto y segundo en que desaparece.
    pub toast: Option<(String, f32)>,
    pub retos: [EstadoReto; 6],
    /// Lo último que Hyprland ha contado (para enseñar que la detección es real).
    pub ultimos: Vec<(f32, String)>,
    // Terminal de juguete.
    pub entrada: String,
    pub historial: Vec<(String, Vec<Line<'static>>)>,
    pub term_ocupado: bool,
    pub term_scroll: usize,
    // Teléfono.
    pub kde_instalado: bool,
    pub telefonos: Vec<Telefono>,
    pub tel_sel: usize,
    pub qr_sel: usize,
    pub cortafuegos: bool,
    pub tel_aviso: String,
    activo_tel: Arc<AtomicBool>,
    // Escritorio (temas).
    pub temas: Vec<String>,
    pub tema_colores: Vec<Option<sistema::ColoresTema>>,
    pub tema_sel: usize,
    pub tema_inicial: Option<String>,
    pub tema_aplicado: Option<String>,
    pub tema_ocupado: bool,
    // Listas de otros capítulos.
    pub herramienta_sel: usize,
    pub concepto_sel: usize,
    pub atajos_scroll: usize,
    pub modal: Modal,
    pub quit: bool,
    /// Al salir, ¿hay que volver a mostrar la Bienvenida en el próximo inicio?
    pub volver_a_mostrar: bool,
    pub hits: Vec<(Rect, Hit)>,
    pub hover: Option<Hit>,
    clicks: Clicks<Hit>,
    comando: Option<Command>,
    en_curso: Option<Tarea>,
    pub act: Act,
    /// Tras actualizar hay una versión nueva de la Bienvenida: reabrirla en este paso.
    pub relanzar: Option<usize>,
    paleta: Palette,
    vigia: Option<PaletteWatcher>,
}

impl App {
    pub fn new(demo: bool, capitulo: usize) -> App {
        let (tx, rx) = channel();
        let kde = !demo && sistema::kdeconnect_instalado();
        let activo = Arc::new(AtomicBool::new(false));
        if kde {
            let (tx, activo) = (tx.clone(), activo.clone());
            std::thread::spawn(move || {
                loop {
                    if activo.load(Ordering::Relaxed)
                        && tx.send(Msg::Telefonos(sistema::telefonos())).is_err()
                    {
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(3));
                }
            });
        }
        let mut app = App {
            cap: 0,
            cap_inicio: 0.0,
            t: 0.0,
            t0: Instant::now(),
            demo,
            usuario: if demo {
                "Lizarbe".into()
            } else {
                sistema::nombre_usuario()
            },
            escuchador: if demo { None } else { Listener::start() },
            tx,
            rx,
            logros: vec![],
            toast: None,
            retos: [EstadoReto::Pendiente; 6],
            ultimos: vec![],
            entrada: String::new(),
            historial: vec![],
            term_ocupado: false,
            term_scroll: 0,
            kde_instalado: kde || demo,
            telefonos: vec![],
            tel_sel: 0,
            qr_sel: 0,
            cortafuegos: true,
            tel_aviso: String::new(),
            activo_tel: activo,
            temas: vec![],
            tema_colores: vec![],
            tema_sel: 0,
            tema_inicial: None,
            tema_aplicado: None,
            tema_ocupado: false,
            herramienta_sel: 0,
            concepto_sel: 0,
            atajos_scroll: 0,
            modal: Modal::Ninguno,
            quit: false,
            volver_a_mostrar: false,
            hits: vec![],
            hover: None,
            clicks: Clicks::default(),
            comando: None,
            en_curso: None,
            act: Act::Buscando,
            relanzar: None,
            paleta: Palette::default(),
            vigia: None,
        };
        if !demo {
            let colores = lizarbe_core::paths::current_theme().join("colors.toml");
            app.paleta = Palette::load(&colores);
            brand::aplicar_paleta(&app.paleta);
            app.vigia = Some(PaletteWatcher::new(&colores));
        }
        app.buscar_actualizaciones();
        app.ir(capitulo.min(Cap::ALL.len() - 1));
        app
    }

    pub fn capitulo(&self) -> Cap {
        Cap::ALL[self.cap]
    }

    /// Segundos en el capítulo actual.
    pub fn cap_t(&self) -> f32 {
        (self.t - self.cap_inicio).max(0.0)
    }

    /// ¿Hay sesión de Hyprland a la que escuchar?
    pub fn en_vivo(&self) -> bool {
        self.escuchador.is_some()
    }

    // ------------------------------------------------------------ actualizar

    fn buscar_actualizaciones(&mut self) {
        if self.demo {
            self.act = Act::Hay(vec![
                "lizarbe-ajustes 1.0.1 -> 1.1.0".into(),
                "linux 7.2.5 -> 7.2.6".into(),
            ]);
            return;
        }
        self.act = Act::Buscando;
        sistema::en_segundo_plano(
            &self.tx,
            || Msg::Actualizaciones(sistema::actualizaciones()),
        );
    }

    fn actualizar(&mut self) {
        if !matches!(self.act, Act::Hay(_)) {
            return;
        }
        if self.demo {
            self.act = Act::Actualizado;
            return;
        }
        self.en_curso = Some(Tarea::Actualizar);
        self.comando = Some(Command {
            program: "omarchy-update".into(),
            args: vec!["-y".into()],
        });
    }

    /// Termina una actualización: si trajo una Bienvenida nueva, se reabre.
    fn actualizacion_terminada(&mut self, ok: bool, instalada: Option<String>) {
        if !ok {
            self.buscar_actualizaciones();
            return;
        }
        self.act = Act::Actualizado;
        if instalada.is_some_and(|v| v != env!("CARGO_PKG_VERSION")) {
            self.relanzar = Some(Cap::Super.indice());
            self.quit = true;
        }
    }

    // ------------------------------------------------------------ guía

    /// Lo que el usuario tiene que hacer ahora en este paso, en una frase.
    /// `hecho` pasa a `true` cuando ya lo logró y solo falta continuar.
    pub fn guia(&self) -> Guia {
        let (texto, hecho) = match self.capitulo() {
            Cap::Practica => match self.reto_actual() {
                None => (t("guia.practica.hecho"), true),
                Some(i) => {
                    let reto = t(&format!("reto.{}", Reto::TODOS[i].id()));
                    let clave = if self.en_vivo() {
                        "guia.practica"
                    } else {
                        "guia.practica.demo"
                    };
                    // «Abra el menú…» → «abra el menú…» (sin tocar «Omarchy»).
                    let mut letras = reto.chars();
                    let reto: String = letras
                        .next()
                        .into_iter()
                        .flat_map(char::to_lowercase)
                        .chain(letras)
                        .collect();
                    (tf(clave, &[("reto", &reto)]), false)
                }
            },
            Cap::Actualizar => match self.act {
                Act::Buscando => (t("guia.actualizar.buscando"), false),
                Act::Hay(_) => (t("guia.actualizar"), false),
                Act::AlDia => (t("guia.actualizar.aldia"), true),
                Act::SinRed => (t("guia.actualizar.sinred"), false),
                Act::Actualizado => (t("guia.actualizar.hecho"), true),
            },
            Cap::Terminal if self.tiene("fastfetch") => (t("guia.terminal.hecho"), true),
            Cap::Terminal => (t("guia.terminal"), false),
            Cap::Telefono => self.guia_telefono(),
            Cap::Escritorio => match &self.tema_aplicado {
                Some(tema) => (tf("guia.escritorio.hecho", &[("tema", tema)]), true),
                None => (t("guia.escritorio"), false),
            },
            Cap::Configurar if self.tiene("ajustes") => (t("guia.configurar.hecho"), true),
            Cap::Configurar => (t("guia.configurar"), false),
            Cap::Conceptos | Cap::Atajos => (t("guia.lista"), false),
            _ => (t("guia.leer"), false),
        };
        Guia { texto, hecho }
    }

    fn guia_telefono(&self) -> (String, bool) {
        if self.tiene("telefono") {
            return (t("guia.telefono.hecho"), true);
        }
        if !self.kde_instalado {
            return (t("guia.telefono.sin_kde"), false);
        }
        if !self.cortafuegos {
            return (t("guia.telefono.puertos"), false);
        }
        let texto = match self.telefono_elegido() {
            Some(p) if p.estado == Estado::Nuevo => {
                tf("guia.telefono.nuevo", &[("nombre", &p.nombre)])
            }
            Some(p) if p.estado == Estado::Solicitado => t("guia.telefono.solicitado"),
            _ => t("guia.telefono"),
        };
        (texto, false)
    }

    /// Lo que hace Enter en este paso, para el botón de abajo a la derecha.
    pub fn etiqueta_enter(&self) -> String {
        match self.capitulo() {
            Cap::Terminal if !self.entrada.is_empty() => t("nav.ejecutar"),
            Cap::Terminal if !self.tiene("fastfetch") => t("nav.probar"),
            Cap::Actualizar if matches!(self.act, Act::Hay(_)) => t("nav.omitir"),
            _ => t("nav.continuar"),
        }
    }

    /// La acción propia del paso (tecla, etiqueta, tecla que la activa), si la hay.
    pub fn accion_secundaria(&self) -> Option<(&'static str, String, char)> {
        match self.capitulo() {
            Cap::Practica if self.reto_actual().is_none() => None,
            Cap::Practica if self.en_vivo() && !self.demo => Some(("S", t("acc.saltar"), 's')),
            Cap::Practica => Some(("Espacio", t("acc.hecho"), ' ')),
            Cap::Actualizar if matches!(self.act, Act::Hay(_)) => {
                Some(("Espacio", t("btn.actualizar"), ' '))
            }
            Cap::Telefono if self.telefono_elegido().is_some() => {
                Some(("Espacio", t("btn.vincular"), ' '))
            }
            Cap::Escritorio if !self.temas.is_empty() => Some(("Espacio", t("btn.aplicar"), ' ')),
            Cap::Configurar => Some(("Espacio", t("herr.abrir"), ' ')),
            _ => None,
        }
    }

    // ------------------------------------------------------------ navegación

    pub fn ir(&mut self, i: usize) {
        let i = i.min(Cap::ALL.len() - 1);
        self.activo_tel.store(false, Ordering::Relaxed);
        self.cap = i;
        self.cap_inicio = self.t;
        self.hits.clear();
        match self.capitulo() {
            Cap::Telefono => self.entrar_telefono(),
            Cap::Escritorio => self.entrar_escritorio(),
            _ => {}
        }
    }

    pub fn siguiente(&mut self) {
        if self.cap + 1 < Cap::ALL.len() {
            self.ir(self.cap + 1);
        }
    }

    pub fn anterior(&mut self) {
        if self.cap > 0 {
            self.ir(self.cap - 1);
        }
    }

    // ------------------------------------------------------------ logros

    pub fn otorgar(&mut self, id: &'static str) {
        if self.logros.contains(&id) {
            return;
        }
        self.logros.push(id);
        if let Some((_, clave)) = LOGROS.iter().find(|(l, _)| *l == id) {
            self.toast = Some((t(clave), self.t + 3.5));
        }
    }

    pub fn tiene(&self, id: &str) -> bool {
        self.logros.contains(&id)
    }

    // ------------------------------------------------------------ práctica

    /// Índice del reto que toca ahora (el primero sin cumplir ni saltar).
    pub fn reto_actual(&self) -> Option<usize> {
        self.retos.iter().position(|e| *e == EstadoReto::Pendiente)
    }

    #[cfg(test)]
    pub fn retos_cumplidos(&self) -> usize {
        self.retos
            .iter()
            .filter(|e| **e == EstadoReto::Hecho)
            .count()
    }

    fn cumplir_reto(&mut self, i: usize) {
        self.retos[i] = EstadoReto::Hecho;
        match Reto::TODOS[i] {
            Reto::Escritorio2 => {}
            Reto::Escritorio1 => self.otorgar("escritorios"),
            r => self.otorgar(r.id_logro()),
        }
    }

    /// Procesa un evento de Hyprland: cumple el reto actual si corresponde.
    pub fn evento(&mut self, ev: &HyprEvent) {
        if let Some(texto) = crate::retos::describir(ev) {
            self.ultimos.push((self.t, texto));
            if self.ultimos.len() > 6 {
                self.ultimos.remove(0);
            }
        }
        if let Some(i) = self.reto_actual()
            && Reto::TODOS[i].cumple(ev)
        {
            self.cumplir_reto(i);
        }
    }

    pub fn saltar_reto(&mut self) {
        if let Some(i) = self.reto_actual() {
            self.retos[i] = EstadoReto::Saltado;
        }
    }

    /// Sin Hyprland (o en demostración) el reto se da por hecho con la barra espaciadora.
    pub fn simular_reto(&mut self) {
        if let Some(i) = self.reto_actual() {
            self.cumplir_reto(i);
        }
    }

    // ------------------------------------------------------------ terminal de juguete

    /// Lo que se sugiere escribir (texto fantasma).
    pub fn sugerencia(&self) -> String {
        if self.entrada.is_empty() {
            for o in ["fastfetch", "lizarbe status", "date"] {
                if !self.historial.iter().any(|(h, _)| h == o) {
                    return o.to_string();
                }
            }
            return String::new();
        }
        sistema::ORDENES
            .iter()
            .find(|o| o.starts_with(self.entrada.as_str()) && **o != self.entrada)
            .map(|o| o.to_string())
            .unwrap_or_default()
    }

    fn poner_salida(&mut self, orden: &str, salida: &str) {
        let mut lineas = ansi::parse(salida);
        lineas.truncate(60);
        self.historial.push((orden.to_string(), lineas));
        self.term_scroll = 0;
        if orden == "fastfetch" {
            self.otorgar("fastfetch");
        }
    }

    fn ejecutar_terminal(&mut self) {
        if self.term_ocupado {
            return;
        }
        let linea = std::mem::take(&mut self.entrada);
        let texto = linea.trim().to_string();
        match sistema::interpretar(&texto) {
            Orden::Vacia => {}
            Orden::Limpiar => self.historial.clear(),
            Orden::Ayuda => {
                let ayuda = format!("{}\n  {}", t("term.ayuda"), sistema::ORDENES.join("\n  "));
                self.poner_salida(&texto, &ayuda);
            }
            Orden::Eco(s) => self.poner_salida(&texto, &s),
            Orden::NoPermitida => {
                let aviso = t("term.no_permitida");
                self.poner_salida(&texto, &aviso);
            }
            Orden::Ejecutar { prog, args } => {
                if self.demo {
                    let s = sistema::salida_demo(&texto);
                    self.poner_salida(&texto, &s);
                } else {
                    self.term_ocupado = true;
                    let orden = texto.clone();
                    sistema::en_segundo_plano(&self.tx, move || {
                        let a: Vec<&str> = args.iter().map(String::as_str).collect();
                        let salida = sistema::salida(prog, &a).unwrap_or_else(|| t("term.fallo"));
                        Msg::Terminal { orden, salida }
                    });
                }
            }
        }
    }

    // ------------------------------------------------------------ teléfono

    fn entrar_telefono(&mut self) {
        self.cortafuegos = if self.demo {
            true
        } else {
            sistema::cortafuegos_ok()
        };
        if self.demo {
            if self.telefonos.is_empty() {
                self.telefonos = vec![Telefono {
                    id: "demo".into(),
                    nombre: t("tel.demo_nombre"),
                    estado: Estado::Nuevo,
                    alcanzable: true,
                }];
            }
            return;
        }
        if self.kde_instalado {
            self.activo_tel.store(true, Ordering::Relaxed);
            sistema::en_segundo_plano(&self.tx, || Msg::Telefonos(sistema::telefonos()));
        }
    }

    pub fn telefono_elegido(&self) -> Option<&Telefono> {
        self.telefonos.get(self.tel_sel)
    }

    fn revisar_telefonos(&mut self) {
        if self
            .telefonos
            .iter()
            .any(|p| p.estado == Estado::Vinculado && p.alcanzable)
        {
            self.otorgar("telefono");
        }
    }

    fn vincular(&mut self) {
        let Some(p) = self.telefono_elegido().cloned() else {
            return;
        };
        if p.estado == Estado::Vinculado {
            self.tel_aviso = t("tel.ya_vinculado");
            return;
        }
        if !p.alcanzable {
            self.tel_aviso = t("tel.no_alcanzable");
            return;
        }
        if self.demo {
            self.telefonos[self.tel_sel].estado = Estado::Vinculado;
            self.revisar_telefonos();
            return;
        }
        self.tel_aviso = t("tel.aceptar_en_telefono");
        let id = p.id.clone();
        sistema::en_segundo_plano(&self.tx, move || {
            Msg::Vincular(sistema::salida("kdeconnect-cli", &["--pair", "-d", &id]).is_some())
        });
    }

    fn sonar(&mut self) {
        let Some(p) = self.telefono_elegido().cloned() else {
            return;
        };
        if p.estado != Estado::Vinculado || !p.alcanzable {
            self.tel_aviso = t("tel.solo_vinculado");
            return;
        }
        if self.demo {
            self.tel_aviso = t("tel.sonando");
            return;
        }
        sistema::en_segundo_plano(&self.tx, move || {
            Msg::Sonar(sistema::salida("kdeconnect-cli", &["--ring", "-d", &p.id]).is_some())
        });
    }

    fn abrir_puertos(&mut self) {
        if self.demo || self.cortafuegos {
            return;
        }
        self.en_curso = Some(Tarea::Puertos);
        self.comando = Some(Command {
            program: "bash".into(),
            args: vec![
                "-c".into(),
                "sudo ufw allow 1714:1764/udp && sudo ufw allow 1714:1764/tcp && sudo ufw reload"
                    .into(),
            ],
        });
    }

    fn abrir_kdeconnect(&mut self) {
        if !self.demo {
            sistema::lanzar("lizarbe-kdeconnect", &[]);
        }
    }

    // ------------------------------------------------------------ temas

    fn entrar_escritorio(&mut self) {
        if !self.temas.is_empty() {
            return;
        }
        if self.demo {
            self.temas = ["Lizarbe", "Lizarbe Light", "Lizarbe Arena", "Tokyo Night"]
                .map(String::from)
                .to_vec();
            self.tema_inicial = Some("Lizarbe".into());
            self.tema_colores = self
                .temas
                .iter()
                .map(|n| sistema::colores_tema(n))
                .collect();
            return;
        }
        self.temas = sistema::temas();
        self.tema_colores = self
            .temas
            .iter()
            .map(|n| sistema::colores_tema(n))
            .collect();
        self.tema_inicial = sistema::tema_actual();
        if let Some(i) = self
            .tema_inicial
            .as_ref()
            .and_then(|a| self.temas.iter().position(|t| t == a))
        {
            self.tema_sel = i;
        }
    }

    fn aplicar_tema_por_nombre(&mut self, nombre: String) {
        if self.tema_ocupado {
            return;
        }
        if self.demo {
            self.tema_aplicado = Some(nombre);
            self.otorgar("tema");
            return;
        }
        self.tema_ocupado = true;
        sistema::en_segundo_plano(&self.tx, move || {
            let ok = sistema::salida("omarchy-theme-set", &[&nombre]).is_some();
            Msg::Tema { nombre, ok }
        });
    }

    fn aplicar_tema(&mut self) {
        if let Some(n) = self.temas.get(self.tema_sel).cloned() {
            self.aplicar_tema_por_nombre(n);
        }
    }

    fn volver_tema(&mut self) {
        if let Some(n) = self.tema_inicial.clone() {
            self.aplicar_tema_por_nombre(n);
        }
    }

    fn siguiente_fondo(&mut self) {
        if self.demo || self.tema_ocupado {
            return;
        }
        self.tema_ocupado = true;
        sistema::en_segundo_plano(&self.tx, || {
            let _ = sistema::salida("omarchy-theme-bg-next", &[]);
            Msg::Fondo
        });
    }

    // ------------------------------------------------------------ herramientas

    fn abrir_herramienta(&mut self, i: usize) {
        let Some(h) = HERRAMIENTAS.get(i) else {
            return;
        };
        if !self.demo {
            let orden = h.orden();
            let args: Vec<&str> = orden.iter().map(String::as_str).collect();
            sistema::lanzar("omarchy-launch-tui", &args);
        }
        self.otorgar("ajustes");
    }

    // ------------------------------------------------------------ salida

    /// Cierra. `mostrar_otra_vez` deja la marca de «pendiente» para el próximo inicio.
    pub fn terminar(&mut self, mostrar_otra_vez: bool) {
        self.volver_a_mostrar = mostrar_otra_vez;
        self.quit = true;
    }

    /// Cuántos elementos tiene la lista del capítulo actual.
    fn largo_lista(&self) -> usize {
        match self.capitulo() {
            Cap::Telefono => self.telefonos.len(),
            Cap::Escritorio => self.temas.len(),
            Cap::Configurar => HERRAMIENTAS.len(),
            Cap::Conceptos => CONCEPTOS.len(),
            Cap::Atajos => total_atajos(),
            _ => 0,
        }
    }

    fn mover_seleccion(&mut self, delta: i32) {
        let n = self.largo_lista();
        if n == 0 {
            return;
        }
        let mover = |v: &mut usize| *v = (*v as i32 + delta).clamp(0, n as i32 - 1) as usize;
        match self.capitulo() {
            Cap::Telefono => mover(&mut self.tel_sel),
            Cap::Escritorio => mover(&mut self.tema_sel),
            Cap::Configurar => mover(&mut self.herramienta_sel),
            Cap::Conceptos => mover(&mut self.concepto_sel),
            Cap::Atajos => mover(&mut self.atajos_scroll),
            _ => {}
        }
    }

    /// Acción principal del capítulo con Enter. Devuelve `false` si el capítulo no tiene y hay que avanzar.
    fn accion_principal(&mut self) -> bool {
        match self.capitulo() {
            Cap::Actualizar => {
                self.actualizar();
                true
            }
            Cap::Telefono => {
                self.vincular();
                true
            }
            Cap::Escritorio => {
                self.aplicar_tema();
                true
            }
            Cap::Configurar => {
                self.abrir_herramienta(self.herramienta_sel);
                true
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------ teclado

    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.modal = Modal::Salir;
            return;
        }
        match self.modal {
            Modal::Salir => return self.tecla_salir(key),
            Modal::Ayuda => {
                self.modal = Modal::Ninguno;
                return;
            }
            Modal::Ninguno => {}
        }
        let cap = self.capitulo();
        if cap == Cap::Terminal && self.tecla_terminal(key) {
            return;
        }
        if cap == Cap::Arranque
            && self.cap_t() < crate::caps::arranque::LISTO
            && matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
        {
            // Un primer Enter salta la animación de arranque.
            self.cap_inicio = self.t - crate::caps::arranque::LISTO - 1.0;
            return;
        }
        match key.code {
            KeyCode::Right | KeyCode::Char('l') => self.siguiente(),
            KeyCode::Left | KeyCode::Char('h') => self.anterior(),
            KeyCode::Char(c @ '1'..='8') => self.ir(Cap::de_numero(c as usize - '0' as usize)),
            KeyCode::Char('0') => self.ir(0),
            KeyCode::Char('?') => self.modal = Modal::Ayuda,
            KeyCode::Char('q') | KeyCode::Esc => self.modal = Modal::Salir,
            KeyCode::Up | KeyCode::Char('k') => self.mover_seleccion(-1),
            KeyCode::Down | KeyCode::Char('j') => self.mover_seleccion(1),
            KeyCode::PageUp => self.mover_seleccion(-5),
            KeyCode::PageDown => self.mover_seleccion(5),
            // Enter siempre continúa; Espacio hace lo propio del paso.
            KeyCode::Enter => {
                if cap == Cap::Final {
                    self.terminar(false);
                } else {
                    self.siguiente();
                }
            }
            KeyCode::Char(' ') => {
                if cap == Cap::Practica {
                    self.simular_reto_si_corresponde();
                } else {
                    self.accion_principal();
                }
            }
            KeyCode::Char('s') if cap == Cap::Practica => self.saltar_reto(),
            KeyCode::Tab if cap == Cap::Telefono => self.qr_sel = (self.qr_sel + 1) % TIENDAS.len(),
            KeyCode::Char('p') if cap == Cap::Telefono => self.vincular(),
            KeyCode::Char('r') if cap == Cap::Telefono => self.sonar(),
            KeyCode::Char('f') if cap == Cap::Telefono => self.abrir_puertos(),
            KeyCode::Char('a') if cap == Cap::Telefono => self.abrir_kdeconnect(),
            KeyCode::Char('b') if cap == Cap::Escritorio => self.siguiente_fondo(),
            KeyCode::Char('z') if cap == Cap::Escritorio => self.volver_tema(),
            KeyCode::Char('r') if cap == Cap::Final => self.ir(0),
            KeyCode::Char('n') if cap == Cap::Final => self.terminar(false),
            _ => {}
        }
    }

    fn simular_reto_si_corresponde(&mut self) {
        if !self.en_vivo() || self.demo {
            self.simular_reto();
        }
    }

    fn tecla_salir(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Char('s') => self.terminar(true),
            KeyCode::Char('n') => self.terminar(false),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('c') => self.modal = Modal::Ninguno,
            _ => {}
        }
    }

    /// Teclas del capítulo de la terminal. Devuelve `true` si las consumió.
    fn tecla_terminal(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('l') if ctrl => {
                self.historial.clear();
                true
            }
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                if self.entrada.chars().count() < 60 {
                    self.entrada.push(c);
                }
                true
            }
            KeyCode::Backspace => {
                self.entrada.pop();
                true
            }
            KeyCode::Tab => {
                let s = self.sugerencia();
                if !s.is_empty() {
                    self.entrada = s;
                }
                true
            }
            KeyCode::Enter => {
                if self.entrada.is_empty() {
                    if self.tiene("fastfetch") {
                        // Ya probó: con la línea vacía, Enter continúa.
                        return false;
                    }
                    // Aún no probó: Enter ejecuta la sugerencia.
                    self.entrada = self.sugerencia();
                }
                self.ejecutar_terminal();
                true
            }
            KeyCode::Esc if !self.entrada.is_empty() => {
                self.entrada.clear();
                true
            }
            KeyCode::Up => {
                self.term_scroll = self.term_scroll.saturating_add(1);
                true
            }
            KeyCode::Down => {
                self.term_scroll = self.term_scroll.saturating_sub(1);
                true
            }
            // Con la línea vacía, las flechas y Esc navegan como en los demás capítulos.
            KeyCode::Left | KeyCode::Right if !self.entrada.is_empty() => true,
            _ => false,
        }
    }

    // ------------------------------------------------------------ ratón

    pub fn on_mouse(&mut self, m: MouseEvent) {
        match mouse::read(m, &self.hits, &mut self.clicks) {
            Mouse::Hover(h) => self.hover = h,
            Mouse::Scroll { down, .. } => {
                if self.modal == Modal::Ninguno {
                    self.mover_seleccion(if down { 1 } else { -1 });
                }
            }
            Mouse::Click {
                hit: Some(h),
                double,
                ..
            } => self.clic(h, double),
            _ => {}
        }
    }

    fn clic(&mut self, h: Hit, double: bool) {
        if self.modal != Modal::Ninguno {
            return;
        }
        match h {
            Hit::Cap(i) => self.ir(Cap::de_numero(i)),
            Hit::Anterior => self.anterior(),
            Hit::Siguiente => self.siguiente(),
            Hit::Fila(i) => {
                match self.capitulo() {
                    Cap::Telefono => self.tel_sel = i,
                    Cap::Escritorio => self.tema_sel = i,
                    Cap::Configurar => self.herramienta_sel = i,
                    Cap::Conceptos => self.concepto_sel = i,
                    _ => {}
                }
                if double {
                    self.accion_principal();
                }
            }
            Hit::Qr(i) => self.qr_sel = i,
            Hit::Boton(c) => {
                let code = if c == '\n' {
                    KeyCode::Enter
                } else {
                    KeyCode::Char(c)
                };
                self.on_key(KeyEvent::new(code, KeyModifiers::NONE));
            }
        }
    }

    // ------------------------------------------------------------ tareas periódicas

    pub fn tick(&mut self) {
        self.t = self.t0.elapsed().as_secs_f32();
        if let Some(v) = &mut self.vigia
            && v.poll(&mut self.paleta)
        {
            brand::aplicar_paleta(&self.paleta);
        }
        if let Some(l) = &self.escuchador {
            for ev in l.drain() {
                self.evento(&ev);
            }
        }
        while let Ok(msg) = self.rx.try_recv() {
            self.recibir(msg);
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, hasta)| self.t > *hasta)
        {
            self.toast = None;
        }
    }

    pub fn recibir(&mut self, msg: Msg) {
        match msg {
            Msg::Telefonos(lista) => {
                // Se conserva el teléfono elegido aunque cambie el orden.
                let id = self.telefono_elegido().map(|p| p.id.clone());
                self.telefonos = lista;
                if let Some(i) = id.and_then(|id| self.telefonos.iter().position(|p| p.id == id)) {
                    self.tel_sel = i;
                }
                self.tel_sel = self.tel_sel.min(self.telefonos.len().saturating_sub(1));
                self.revisar_telefonos();
            }
            Msg::Terminal { orden, salida } => {
                self.term_ocupado = false;
                self.poner_salida(&orden, &salida);
            }
            Msg::Tema { nombre, ok } => {
                self.tema_ocupado = false;
                if ok {
                    self.tema_aplicado = Some(nombre);
                    self.otorgar("tema");
                }
            }
            Msg::Fondo => self.tema_ocupado = false,
            Msg::Actualizaciones(lista) => {
                // Si ya se actualizó en esta sesión, el aviso no vuelve a ofrecerlo.
                if self.act != Act::Actualizado {
                    self.act = match lista {
                        None => Act::SinRed,
                        Some(v) if v.is_empty() => Act::AlDia,
                        Some(v) => Act::Hay(v),
                    };
                }
            }
            Msg::Vincular(ok) => {
                self.tel_aviso = if ok {
                    t("tel.aceptar_en_telefono")
                } else {
                    t("tel.no_alcanzable")
                };
                sistema::en_segundo_plano(&self.tx, || Msg::Telefonos(sistema::telefonos()));
            }
            Msg::Sonar(ok) => {
                self.tel_aviso = if ok {
                    t("tel.sonando")
                } else {
                    t("tel.solo_vinculado")
                };
            }
        }
    }
}

impl TuiApp for App {
    fn draw(&mut self, f: &mut Frame) {
        crate::ui::dibujar(self, f);
    }

    fn on_key(&mut self, key: KeyEvent) {
        App::on_key(self, key);
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        App::on_mouse(self, m);
    }

    fn tick(&mut self) {
        App::tick(self);
    }

    fn take_command(&mut self) -> Option<Command> {
        self.comando.take()
    }

    fn after_command(&mut self, result: Result<bool, String>) {
        match self.en_curso.take() {
            Some(Tarea::Actualizar) => self.actualizacion_terminada(
                result == Ok(true),
                sistema::version_instalada("lizarbe-ajustes"),
            ),
            _ => self.cortafuegos = sistema::cortafuegos_ok(),
        }
    }

    fn should_quit(&self) -> bool {
        self.quit
    }

    fn frame_interval(&self) -> Duration {
        match self.capitulo() {
            Cap::Arranque | Cap::Final => Duration::from_millis(33),
            _ => Duration::from_millis(66),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tecla(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn ev(name: &str, data: &str) -> HyprEvent {
        HyprEvent {
            name: name.into(),
            data: data.into(),
        }
    }

    fn demo() -> App {
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        App::new(true, 0)
    }

    #[test]
    fn chapters_are_numbered_one_to_eight() {
        assert_eq!(Cap::Arranque.numero(), None);
        assert_eq!(Cap::Final.numero(), None);
        assert_eq!(Cap::Super.numero(), Some(1));
        assert_eq!(Cap::Atajos.numero(), Some(Cap::NUMERADOS));
    }

    #[test]
    fn arrows_and_numbers_navigate() {
        let mut a = demo();
        a.t = 10.0;
        a.on_key(tecla(KeyCode::Right));
        assert_eq!(a.capitulo(), Cap::Actualizar);
        a.on_key(tecla(KeyCode::Right));
        assert_eq!(a.capitulo(), Cap::Super);
        a.on_key(tecla(KeyCode::Char('5')));
        assert_eq!(a.capitulo(), Cap::Escritorio);
        a.on_key(tecla(KeyCode::Left));
        assert_eq!(a.capitulo(), Cap::Telefono);
        // No se sale por los extremos.
        a.ir(0);
        a.anterior();
        assert_eq!(a.cap, 0);
        a.ir(99);
        a.siguiente();
        assert_eq!(a.capitulo(), Cap::Final);
    }

    #[test]
    fn first_enter_skips_the_boot_animation_then_advances() {
        let mut a = demo();
        a.t = 1.0;
        a.on_key(tecla(KeyCode::Enter));
        assert_eq!(a.capitulo(), Cap::Arranque);
        assert!(a.cap_t() >= crate::caps::arranque::LISTO);
        a.on_key(tecla(KeyCode::Enter));
        assert_eq!(a.capitulo(), Cap::Actualizar);
    }

    #[test]
    fn live_challenges_complete_in_order_and_grant_achievements() {
        let mut a = demo();
        a.ir(3);
        assert_eq!(a.reto_actual(), Some(0));
        // Un evento que no toca no cuenta.
        a.evento(&ev("workspace", "2"));
        assert_eq!(a.reto_actual(), Some(0));
        a.evento(&ev("openlayer", "omarchy-menu"));
        assert!(a.tiene("menu"));
        assert!(a.toast.is_some());
        a.evento(&ev("openwindow", "1,1,kitty,~"));
        a.evento(&ev("closewindow", "1"));
        a.evento(&ev("workspace", "2"));
        assert!(!a.tiene("escritorios"), "falta volver al 1");
        a.evento(&ev("workspace", "1"));
        assert!(a.tiene("escritorios"));
        a.evento(&ev("fullscreen", "1"));
        assert_eq!(a.retos_cumplidos(), 6);
        assert_eq!(a.reto_actual(), None);
        assert!(a.tiene("terminal") && a.tiene("cerrar") && a.tiene("pantalla"));
    }

    #[test]
    fn skipping_and_simulating_challenges() {
        let mut a = demo();
        a.ir(3);
        a.on_key(tecla(KeyCode::Char('s')));
        assert_eq!(a.retos[0], EstadoReto::Saltado);
        assert!(!a.tiene("menu"));
        a.on_key(tecla(KeyCode::Char(' ')));
        assert_eq!(a.retos[1], EstadoReto::Hecho);
        assert!(a.tiene("terminal"));
    }

    #[test]
    fn achievements_are_granted_once() {
        let mut a = demo();
        a.otorgar("menu");
        a.toast = None;
        a.otorgar("menu");
        assert_eq!(a.logros, vec!["menu"]);
        assert!(a.toast.is_none());
    }

    #[test]
    fn toy_terminal_types_runs_and_scores_fastfetch() {
        let mut a = demo();
        a.ir(4);
        assert_eq!(a.sugerencia(), "fastfetch");
        a.on_key(tecla(KeyCode::Tab));
        assert_eq!(a.entrada, "fastfetch");
        a.on_key(tecla(KeyCode::Enter));
        assert!(a.entrada.is_empty());
        assert_eq!(a.historial.len(), 1);
        assert!(a.tiene("fastfetch"));
        assert_eq!(a.sugerencia(), "lizarbe status");
        // Escribir no navega aunque sean teclas de navegación.
        for c in "q5h".chars() {
            a.on_key(tecla(KeyCode::Char(c)));
        }
        assert_eq!(a.capitulo(), Cap::Terminal);
        assert_eq!(a.entrada, "q5h");
        a.on_key(tecla(KeyCode::Enter));
        assert_eq!(a.historial.len(), 2);
        // Con la línea vacía, las flechas navegan.
        a.on_key(tecla(KeyCode::Right));
        assert_eq!(a.capitulo(), Cap::Telefono);
    }

    #[test]
    fn enter_always_continues_except_where_it_has_a_job() {
        let mut a = demo();
        // Terminal: antes de probar, Enter con la línea vacía ejecuta la sugerencia…
        a.ir(4);
        assert_eq!(a.etiqueta_enter(), t("nav.probar"));
        a.on_key(tecla(KeyCode::Enter));
        assert!(a.tiene("fastfetch"));
        assert_eq!(a.capitulo(), Cap::Terminal);
        // …y una vez hecho, Enter continúa.
        assert_eq!(a.etiqueta_enter(), t("nav.continuar"));
        a.on_key(tecla(KeyCode::Enter));
        assert_eq!(a.capitulo(), Cap::Telefono);
        // En el resto de pasos, Enter solo continúa y Espacio hace lo propio del paso.
        a.ir(6);
        a.on_key(tecla(KeyCode::Enter));
        assert_eq!(a.capitulo(), Cap::Configurar);
        assert!(a.tema_aplicado.is_none() && !a.tiene("ajustes"));
        a.on_key(tecla(KeyCode::Char(' ')));
        assert_eq!(a.capitulo(), Cap::Configurar);
        assert!(a.tiene("ajustes"));
    }

    #[test]
    fn update_step_offers_updating_and_can_be_skipped() {
        let mut a = demo();
        a.ir(Cap::Actualizar.indice());
        assert!(matches!(a.act, Act::Hay(_)));
        assert_eq!(a.etiqueta_enter(), t("nav.omitir"));
        assert_eq!(a.accion_secundaria().unwrap().2, ' ');
        // Enter omite sin actualizar.
        a.on_key(tecla(KeyCode::Enter));
        assert_eq!(a.capitulo(), Cap::Super);
        assert!(matches!(a.act, Act::Hay(_)));
        // Espacio actualiza (en demostración no toca el sistema).
        a.ir(Cap::Actualizar.indice());
        a.on_key(tecla(KeyCode::Char(' ')));
        assert_eq!(a.act, Act::Actualizado);
        assert!(a.guia().hecho);
        assert_eq!(a.etiqueta_enter(), t("nav.continuar"));
        assert!(a.accion_secundaria().is_none());
    }

    #[test]
    fn search_results_become_states() {
        let mut a = demo();
        a.recibir(Msg::Actualizaciones(None));
        assert_eq!(a.act, Act::SinRed);
        a.recibir(Msg::Actualizaciones(Some(vec![])));
        assert_eq!(a.act, Act::AlDia);
        a.recibir(Msg::Actualizaciones(Some(vec!["x 1 -> 2".into()])));
        assert!(matches!(a.act, Act::Hay(_)));
    }

    #[test]
    fn a_new_version_after_updating_reopens_the_welcome() {
        let mut a = demo();
        a.ir(Cap::Actualizar.indice());
        // Misma versión: sigue la guía.
        a.actualizacion_terminada(true, Some(env!("CARGO_PKG_VERSION").into()));
        assert_eq!(a.act, Act::Actualizado);
        assert!(a.relanzar.is_none() && !a.quit);
        // Versión distinta: se reabre en el primer paso numerado.
        a.actualizacion_terminada(true, Some("99.0.0".into()));
        assert_eq!(a.relanzar, Some(Cap::Super.indice()));
        assert!(a.quit);
        // Si falló, no se reabre y se vuelve a buscar.
        let mut b = demo();
        b.actualizacion_terminada(false, Some("99.0.0".into()));
        assert!(b.relanzar.is_none() && !b.quit);
    }

    #[test]
    fn the_guide_tells_what_to_do_and_flips_when_done() {
        let mut a = demo();
        a.ir(4);
        let g = a.guia();
        assert!(!g.hecho && g.texto.contains("fastfetch"));
        a.on_key(tecla(KeyCode::Enter));
        let g = a.guia();
        assert!(g.hecho && g.texto.contains("Enter"));
        // Cada paso numerado tiene una frase que dice qué hacer.
        for i in 1..=Cap::NUMERADOS {
            a.ir(Cap::de_numero(i));
            assert!(!a.guia().texto.is_empty(), "paso {i}");
        }
    }

    #[test]
    fn dangerous_commands_are_refused_in_the_toy_terminal() {
        let mut a = demo();
        a.ir(4);
        a.entrada = "rm -rf /".into();
        a.on_key(tecla(KeyCode::Enter));
        let (orden, salida) = a.historial.last().unwrap();
        assert_eq!(orden, "rm -rf /");
        assert!(salida[0].to_string().contains(&t("term.no_permitida")));
    }

    #[test]
    fn phone_pairing_flow_in_demo() {
        let mut a = demo();
        a.ir(5);
        assert_eq!(a.telefonos.len(), 1);
        assert!(!a.tiene("telefono"));
        a.on_key(tecla(KeyCode::Char('p')));
        assert_eq!(a.telefonos[0].estado, Estado::Vinculado);
        assert!(a.tiene("telefono"));
        a.on_key(tecla(KeyCode::Tab));
        assert_eq!(a.qr_sel, 1);
    }

    #[test]
    fn real_phone_list_updates_and_keeps_selection() {
        let mut a = demo();
        a.ir(5);
        let tel = |id: &str, est| Telefono {
            id: id.into(),
            nombre: id.into(),
            estado: est,
            alcanzable: true,
        };
        a.recibir(Msg::Telefonos(vec![
            tel("a", Estado::Nuevo),
            tel("b", Estado::Nuevo),
        ]));
        a.tel_sel = 1;
        a.recibir(Msg::Telefonos(vec![
            tel("b", Estado::Vinculado),
            tel("a", Estado::Nuevo),
        ]));
        assert_eq!(a.telefono_elegido().unwrap().id, "b");
        assert!(a.tiene("telefono"));
    }

    #[test]
    fn theme_chapter_applies_and_restores_in_demo() {
        let mut a = demo();
        a.ir(6);
        assert_eq!(a.temas[0], "Lizarbe");
        a.on_key(tecla(KeyCode::Down));
        a.on_key(tecla(KeyCode::Char(' ')));
        assert_eq!(a.tema_aplicado.as_deref(), Some("Lizarbe Light"));
        assert!(a.tiene("tema"));
        a.on_key(tecla(KeyCode::Char('z')));
        assert_eq!(a.tema_aplicado.as_deref(), Some("Lizarbe"));
    }

    #[test]
    fn tools_open_and_grant_the_settings_achievement() {
        let mut a = demo();
        a.ir(7);
        a.on_key(tecla(KeyCode::Down));
        a.on_key(tecla(KeyCode::Char(' ')));
        assert!(a.tiene("ajustes"));
        // Los límites de la lista se respetan.
        a.mover_seleccion(100);
        assert_eq!(a.herramienta_sel, HERRAMIENTAS.len() - 1);
        a.mover_seleccion(-100);
        assert_eq!(a.herramienta_sel, 0);
    }

    #[test]
    fn quitting_asks_and_remembers_the_choice() {
        let mut a = demo();
        a.t = 10.0;
        a.ir(2);
        a.on_key(tecla(KeyCode::Char('q')));
        assert_eq!(a.modal, Modal::Salir);
        a.on_key(tecla(KeyCode::Esc));
        assert_eq!(a.modal, Modal::Ninguno);
        assert!(!a.quit);
        a.on_key(tecla(KeyCode::Char('q')));
        a.on_key(tecla(KeyCode::Char('n')));
        assert!(a.quit && !a.volver_a_mostrar);
        let mut b = demo();
        b.on_key(tecla(KeyCode::Char('q')));
        b.on_key(tecla(KeyCode::Enter));
        assert!(b.quit && b.volver_a_mostrar);
    }

    #[test]
    fn finishing_from_the_last_screen_does_not_ask_again() {
        let mut a = demo();
        a.ir(10);
        a.on_key(tecla(KeyCode::Enter));
        assert!(a.quit && !a.volver_a_mostrar);
    }

    #[test]
    fn toast_expires() {
        let mut a = demo();
        a.otorgar("menu");
        assert!(a.toast.is_some());
        let hasta = a.toast.as_ref().unwrap().1;
        a.toast.as_mut().unwrap().1 = a.t - 1.0;
        a.tick();
        assert!(a.toast.is_none(), "{hasta}");
    }
}
