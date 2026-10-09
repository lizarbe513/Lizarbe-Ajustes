//! Estado de Recibir: los teléfonos que ve KDE Connect (se consultan en
//! segundo plano), los archivos que van llegando a la carpeta de destino y
//! las acciones de la ventana.

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command as Proc, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lizarbe_core::kdeconnect::{self, Estado, Telefono};
use lizarbe_core::mouse::{self, Clicks, Mouse};
use lizarbe_core::term::{Command, TuiApp};
use lizarbe_core::theme::{Palette, PaletteWatcher};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;

use crate::i18n::{t, tf};
use crate::ui;

/// Cada cuánto se consulta a KDE Connect.
const CONSULTA: Duration = Duration::from_secs(1);
/// Cada cuánto se mira la carpeta de destino.
const ESCANEO: Duration = Duration::from_secs(1);
/// Cuánto dura un aviso en el pie.
const AVISO: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcion {
    Avisar,
    Descargas,
    Kde,
    Cerrar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recibido {
    pub hora: String,
    pub nombre: String,
    pub tam: String,
}

/// Lo que cuenta la ventana sobre los teléfonos.
#[derive(Debug, Clone, PartialEq)]
pub enum Situacion {
    SinKde,
    Buscando,
    Ninguno,
    Conectado(Vec<String>),
    FueraDeAlcance(String),
    Solicitado(String),
    SinVincular(String),
}

pub enum Msg {
    Telefonos(Option<Vec<Telefono>>),
    Aviso(bool),
}

#[derive(Debug, Clone)]
pub struct Aviso {
    pub texto: String,
    pub bien: bool,
    desde: Instant,
}

pub struct App {
    rx: Receiver<Msg>,
    tx: Sender<Msg>,
    demo: bool,
    kde: bool,
    telefonos: Option<Vec<Telefono>>,
    pub equipo: String,
    pub destino: PathBuf,
    destino_de: Option<String>,
    inicio: i64,
    pendientes: HashMap<PathBuf, u64>,
    vistos: HashSet<PathBuf>,
    pub recibidos: Vec<Recibido>,
    ultimo_escaneo: Instant,
    pub sel: Opcion,
    pub hover: Option<Opcion>,
    pub hits: Vec<(Rect, Opcion)>,
    clicks: Clicks<Opcion>,
    pub aviso: Option<Aviso>,
    pub paleta: Palette,
    vigia: Option<PaletteWatcher>,
    t0: Instant,
    quit: bool,
}

impl App {
    /// Con `demo` no se toca nada del sistema: un teléfono y un archivo de ejemplo.
    pub fn new(demo: bool) -> App {
        let (tx, rx) = channel();
        let kde = demo || kdeconnect::instalado();
        let mut app = App {
            rx,
            tx,
            demo,
            kde,
            telefonos: None,
            equipo: "lizarbe".into(),
            destino: PathBuf::from("~/Downloads"),
            destino_de: None,
            inicio: ahora(),
            pendientes: HashMap::new(),
            vistos: HashSet::new(),
            recibidos: vec![],
            ultimo_escaneo: Instant::now(),
            sel: Opcion::Kde,
            hover: None,
            hits: vec![],
            clicks: Clicks::default(),
            aviso: None,
            paleta: Palette::default(),
            vigia: None,
            t0: Instant::now(),
            quit: false,
        };
        if demo {
            app.telefonos = Some(vec![Telefono {
                id: "demo".into(),
                nombre: "Galaxy A06".into(),
                estado: Estado::Vinculado,
                alcanzable: true,
            }]);
            app.recibidos.push(Recibido {
                hora: "12:41".into(),
                nombre: "foto_vacaciones.jpg".into(),
                tam: "2.4 MB".into(),
            });
        } else {
            app.equipo = kdeconnect::nombre_equipo();
            app.destino = kdeconnect::carpeta_destino(None);
            let colores = lizarbe_core::paths::current_theme().join("colors.toml");
            app.paleta = Palette::load(&colores);
            app.vigia = Some(PaletteWatcher::new(&colores));
            if kde {
                vigilar(app.tx.clone());
            }
        }
        app.sel = app.opciones()[0];
        app
    }

    pub fn situacion(&self) -> Situacion {
        if !self.kde {
            return Situacion::SinKde;
        }
        let Some(lista) = &self.telefonos else {
            return Situacion::Buscando;
        };
        let conectados: Vec<String> = lista
            .iter()
            .filter(|p| p.conectado())
            .map(|p| p.nombre.clone())
            .collect();
        if !conectados.is_empty() {
            return Situacion::Conectado(conectados);
        }
        let primero =
            |f: &dyn Fn(&Telefono) -> bool| lista.iter().find(|p| f(p)).map(|p| p.nombre.clone());
        if let Some(n) = primero(&|p| p.estado == Estado::Vinculado) {
            Situacion::FueraDeAlcance(n)
        } else if let Some(n) = primero(&|p| p.estado == Estado::Solicitado) {
            Situacion::Solicitado(n)
        } else if let Some(n) = primero(&|p| p.alcanzable) {
            Situacion::SinVincular(n)
        } else {
            Situacion::Ninguno
        }
    }

    pub fn conectado(&self) -> bool {
        matches!(self.situacion(), Situacion::Conectado(_))
    }

    pub fn opciones(&self) -> Vec<Opcion> {
        if self.conectado() {
            vec![
                Opcion::Avisar,
                Opcion::Descargas,
                Opcion::Kde,
                Opcion::Cerrar,
            ]
        } else {
            vec![Opcion::Kde, Opcion::Descargas, Opcion::Cerrar]
        }
    }

    /// Instante de la animación (décimas de segundo).
    pub fn pulso(&self) -> usize {
        (self.t0.elapsed().as_millis() / 100) as usize
    }

    fn mover(&mut self, delta: isize) {
        let ops = self.opciones();
        let i = ops.iter().position(|o| *o == self.sel).unwrap_or(0) as isize;
        self.sel = ops[(i + delta).rem_euclid(ops.len() as isize) as usize];
    }

    fn activar(&mut self, op: Opcion) {
        match op {
            Opcion::Avisar => {
                let ids: Vec<String> = self
                    .telefonos
                    .iter()
                    .flatten()
                    .filter(|p| p.conectado())
                    .map(|p| p.id.clone())
                    .collect();
                let tx = self.tx.clone();
                let mensaje = tf("aviso.ping", &[("equipo", &self.equipo)]);
                if self.demo {
                    let _ = tx.send(Msg::Aviso(true));
                } else {
                    std::thread::spawn(move || {
                        // A todos los teléfonos, aunque alguno falle.
                        let mut ok = false;
                        for id in &ids {
                            ok |= kdeconnect::avisar(id, &mensaje);
                        }
                        let _ = tx.send(Msg::Aviso(ok));
                    });
                }
            }
            Opcion::Descargas => {
                if !self.demo {
                    let _ = std::fs::create_dir_all(&self.destino);
                    lanzar("xdg-open", &[&self.destino.to_string_lossy()]);
                }
                self.quit = true;
            }
            Opcion::Kde => {
                if !self.demo {
                    lanzar("lizarbe-kdeconnect", &[]);
                }
                self.quit = true;
            }
            Opcion::Cerrar => self.quit = true,
        }
    }

    fn avisar(&mut self, texto: String, bien: bool) {
        self.aviso = Some(Aviso {
            texto,
            bien,
            desde: Instant::now(),
        });
    }

    fn recibir(&mut self, msg: Msg) {
        match msg {
            Msg::Telefonos(lista) => {
                let primera = self.telefonos.is_none();
                self.telefonos = Some(lista.unwrap_or_default());
                self.actualizar_destino();
                if primera {
                    self.sel = self.opciones()[0];
                }
            }
            Msg::Aviso(ok) => {
                self.avisar(t(if ok { "aviso.ok" } else { "aviso.error" }), ok);
            }
        }
    }

    /// La carpeta de destino es la del primer teléfono conectado.
    fn actualizar_destino(&mut self) {
        let id = self
            .telefonos
            .iter()
            .flatten()
            .find(|p| p.conectado())
            .map(|p| p.id.clone());
        if id.is_some() && id != self.destino_de && !self.demo {
            self.destino = kdeconnect::carpeta_destino(id.as_deref());
            self.destino_de = id;
        }
    }

    /// Anota los archivos nuevos de la carpeta de destino. Un archivo cuenta
    /// cuando su tamaño ya no cambia entre dos revisiones (así no se anuncia
    /// a medias) y nació después de abrir la ventana.
    pub fn escanear(&mut self) {
        let Ok(entradas) = std::fs::read_dir(&self.destino) else {
            return;
        };
        let mut listos = vec![];
        for e in entradas.flatten() {
            let ruta = e.path();
            let nombre = e.file_name().to_string_lossy().to_string();
            if self.vistos.contains(&ruta)
                || nombre.starts_with('.')
                || nombre.ends_with(".part")
                || nombre.ends_with(".crdownload")
            {
                continue;
            }
            let Ok(m) = e.metadata() else { continue };
            if !m.is_file() || nacimiento(&m) < self.inicio {
                continue;
            }
            if self.pendientes.get(&ruta) == Some(&m.len()) {
                listos.push((ruta, nombre, m.len()));
            } else {
                self.pendientes.insert(ruta, m.len());
            }
        }
        listos.sort_by(|a, b| a.1.cmp(&b.1));
        for (ruta, nombre, tam) in listos {
            self.pendientes.remove(&ruta);
            self.vistos.insert(ruta);
            self.avisar(tf("aviso.recibido", &[("nombre", &nombre)]), true);
            if !self.demo {
                notificar(t("notif.titulo"), nombre.clone());
            }
            self.recibidos.push(Recibido {
                hora: chrono::Local::now().format("%H:%M").to_string(),
                nombre,
                tam: tamano(tam),
            });
        }
    }

    pub fn dibujar(&mut self, f: &mut Frame) {
        ui::dibujar(self, f);
    }
}

fn ahora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Cuándo se creó el archivo (si el sistema de archivos no lo dice, su último cambio).
fn nacimiento(m: &std::fs::Metadata) -> i64 {
    m.created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(m.ctime(), |d| d.as_secs() as i64)
}

pub fn tamano(bytes: u64) -> String {
    const U: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64 / 1024.0;
    let mut u = 0;
    while v >= 1024.0 && u < U.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", U[u])
}

/// Consulta a KDE Connect cada pocos segundos (y de vez en cuando le pide
/// que vuelva a buscar en la red). Termina cuando se cierra la ventana.
fn vigilar(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let mut vuelta = 0u32;
        loop {
            let lista = kdeconnect::telefonos(vuelta > 0 && vuelta.is_multiple_of(5));
            if tx.send(Msg::Telefonos(lista)).is_err() {
                break;
            }
            vuelta += 1;
            std::thread::sleep(CONSULTA);
        }
    });
}

fn notificar(titulo: String, texto: String) {
    std::thread::spawn(move || {
        let _ = Proc::new("omarchy-notification-send")
            .args(["-g", "", &titulo, &texto])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    });
}

/// Lanza un programa desligado de esta ventana (sigue vivo al cerrarla), a
/// la manera de Omarchy (`uwsm-app`); si no existe, directamente.
fn lanzar(prog: &str, args: &[&str]) {
    let intento = |mut c: Proc| {
        c.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        c.process_group(0);
        c.spawn().ok()
    };
    let mut uwsm = Proc::new("uwsm-app");
    uwsm.arg("--").arg(prog).args(args);
    let hijo = intento(uwsm).or_else(|| {
        let mut directo = Proc::new(prog);
        directo.args(args);
        intento(directo)
    });
    if let Some(mut h) = hijo {
        std::thread::spawn(move || {
            let _ = h.wait();
        });
    }
}

impl TuiApp for App {
    fn draw(&mut self, f: &mut Frame) {
        self.dibujar(f);
    }

    fn on_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q' | 'Q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Up | KeyCode::Char('k') => self.mover(-1),
            KeyCode::Down | KeyCode::Char('j') => self.mover(1),
            KeyCode::Enter | KeyCode::Char(' ') => {
                let op = self.sel;
                self.activar(op);
            }
            _ => {}
        }
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        match mouse::read(m, &self.hits, &mut self.clicks) {
            Mouse::Hover(h) => self.hover = h,
            Mouse::Scroll { down, .. } => self.mover(if down { 1 } else { -1 }),
            Mouse::Click { hit: Some(op), .. } => {
                self.sel = op;
                self.activar(op);
            }
            _ => {}
        }
    }

    fn tick(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.recibir(msg);
        }
        if let Some(v) = &mut self.vigia {
            v.poll(&mut self.paleta);
        }
        if self
            .aviso
            .as_ref()
            .is_some_and(|a| a.desde.elapsed() > AVISO)
        {
            self.aviso = None;
        }
        if !self.opciones().contains(&self.sel) {
            self.sel = self.opciones()[0];
        }
        if !self.demo && self.ultimo_escaneo.elapsed() >= ESCANEO {
            self.ultimo_escaneo = Instant::now();
            self.escanear();
        }
    }

    fn take_command(&mut self) -> Option<Command> {
        None
    }

    fn after_command(&mut self, _result: Result<bool, String>) {}

    fn should_quit(&self) -> bool {
        self.quit
    }

    fn frame_interval(&self) -> Duration {
        if self.conectado() && self.recibidos.is_empty() {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(250)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tel(nombre: &str, estado: Estado, alcanzable: bool) -> Telefono {
        Telefono {
            id: nombre.to_lowercase(),
            nombre: nombre.into(),
            estado,
            alcanzable,
        }
    }

    fn app_con(telefonos: Option<Vec<Telefono>>) -> App {
        let mut a = App::new(true);
        a.telefonos = telefonos;
        a
    }

    #[test]
    fn situation_follows_the_phones() {
        use Estado::*;
        assert_eq!(app_con(None).situacion(), Situacion::Buscando);
        assert_eq!(app_con(Some(vec![])).situacion(), Situacion::Ninguno);
        assert_eq!(
            app_con(Some(vec![
                tel("A", Vinculado, true),
                tel("B", Vinculado, true)
            ]))
            .situacion(),
            Situacion::Conectado(vec!["A".into(), "B".into()])
        );
        assert_eq!(
            app_con(Some(vec![tel("A", Vinculado, false)])).situacion(),
            Situacion::FueraDeAlcance("A".into())
        );
        assert_eq!(
            app_con(Some(vec![tel("A", Solicitado, true)])).situacion(),
            Situacion::Solicitado("A".into())
        );
        assert_eq!(
            app_con(Some(vec![tel("A", Nuevo, true)])).situacion(),
            Situacion::SinVincular("A".into())
        );
    }

    #[test]
    fn notifying_the_phone_needs_one_connected() {
        let a = app_con(Some(vec![]));
        assert!(!a.opciones().contains(&Opcion::Avisar));
        let a = app_con(Some(vec![tel("A", Estado::Vinculado, true)]));
        assert_eq!(a.opciones()[0], Opcion::Avisar);
    }

    #[test]
    fn selection_wraps_and_survives_a_disconnect() {
        let mut a = app_con(Some(vec![tel("A", Estado::Vinculado, true)]));
        a.sel = Opcion::Cerrar;
        a.mover(1);
        assert_eq!(a.sel, Opcion::Avisar);
        a.telefonos = Some(vec![]);
        a.tick();
        assert_eq!(a.sel, Opcion::Kde);
    }

    #[test]
    fn sizes_are_readable() {
        assert_eq!(tamano(12), "12 B");
        assert_eq!(tamano(2048), "2.0 KB");
        assert_eq!(tamano(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn new_files_are_announced_once_they_stop_growing() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(true);
        a.recibidos.clear();
        a.destino = dir.path().to_path_buf();
        a.inicio = ahora() - 5;
        std::fs::write(dir.path().join("viejo.txt"), "x").unwrap();
        std::fs::write(dir.path().join(".oculto"), "x").unwrap();
        std::fs::write(dir.path().join("a medias.part"), "x").unwrap();
        a.escanear();
        assert!(
            a.recibidos.is_empty(),
            "la primera vez solo se anota el tamaño"
        );
        a.escanear();
        let nombres: Vec<_> = a.recibidos.iter().map(|r| r.nombre.as_str()).collect();
        assert_eq!(nombres, ["viejo.txt"]);
        assert!(a.aviso.as_ref().is_some_and(|x| x.bien));
        a.escanear();
        assert_eq!(a.recibidos.len(), 1, "no se repite");
    }

    #[test]
    fn files_that_were_already_there_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = App::new(true);
        a.recibidos.clear();
        a.destino = dir.path().to_path_buf();
        std::fs::write(dir.path().join("anterior.txt"), "x").unwrap();
        a.inicio = ahora() + 60;
        a.escanear();
        a.escanear();
        assert!(a.recibidos.is_empty());
    }
}
