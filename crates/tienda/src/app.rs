//! Estado de la tienda y atención al teclado y al ratón.

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

use lizarbe_core::mouse::{self, Clicks, Mouse};
use lizarbe_core::term::{Command, TuiApp};
use lizarbe_core::theme::Palette;
use lizarbe_core::ui::fold;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;

use crate::catalogo::{Catalogo, Fuente};
use crate::i18n::{t, tf};
use crate::sistema::{self, Estado, Info};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Vista {
    Recomendadas,
    Cat(usize),
    Instaladas,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    Vista(usize),
    Fila(usize),
    Buscar,
    Instalar,
    Quitar,
    Abrir,
    Avanzado,
    Ok,
    Cancelar,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Foco {
    Vistas,
    Lista,
    Busqueda,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tarea {
    Instalar(Vec<usize>),
    Quitar(usize),
    Avanzado,
    Salir,
}

#[derive(Debug, Clone)]
pub struct Modal {
    pub tarea: Tarea,
    pub titulo: String,
    pub lineas: Vec<String>,
}

pub struct App {
    pub cat: Catalogo,
    pub fuente: Fuente,
    pub estado: Estado,
    pub pal: Palette,
    pub vistas: Vec<Vista>,
    pub vista: usize,
    pub fila: usize,
    pub scroll: usize,
    pub list_h: usize,
    pub foco: Foco,
    pub consulta: String,
    pub marcadas: BTreeSet<usize>,
    pub modal: Option<Modal>,
    pub aviso: Option<(String, bool, Instant)>,
    pub hits: Vec<(Rect, Hit)>,
    pub hover: Option<Hit>,
    clicks: Clicks<Hit>,
    infos: HashMap<String, Option<Info>>,
    pendiente: Option<(Command, Tarea)>,
    salir: bool,
}

impl App {
    pub fn new(cat: Catalogo, estado: Estado, pal: Palette, fuente: Fuente) -> App {
        let mut vistas = vec![Vista::Recomendadas];
        vistas.extend(
            (0..cat.categorias.len())
                .filter(|i| cat.categoria_con_apps(*i, fuente))
                .map(Vista::Cat),
        );
        vistas.push(Vista::Instaladas);
        App {
            cat,
            fuente,
            estado,
            pal,
            vistas,
            vista: 0,
            fila: 0,
            scroll: 0,
            list_h: 10,
            foco: Foco::Lista,
            consulta: String::new(),
            marcadas: BTreeSet::new(),
            modal: None,
            aviso: None,
            hits: vec![],
            hover: None,
            clicks: Clicks::default(),
            infos: HashMap::new(),
            pendiente: None,
            salir: false,
        }
    }

    /// Abre en la categoría `id` (o en «recomendadas» / «instaladas»).
    pub fn ir_a_categoria(&mut self, id: &str) {
        let v = match id {
            "recomendadas" => Some(Vista::Recomendadas),
            "instaladas" => Some(Vista::Instaladas),
            _ => self.cat.categoria(id).map(Vista::Cat),
        };
        if let Some(i) = v.and_then(|v| self.vistas.iter().position(|x| *x == v)) {
            self.vista = i;
            self.fila = 0;
        }
    }

    /// Abre seleccionando la app `id`.
    pub fn ir_a_app(&mut self, id: &str) {
        let Some(a) = self.cat.app(id) else { return };
        let cat = self.cat.apps[a].categoria.clone();
        self.ir_a_categoria(&cat);
        if let Some(p) = self.visibles().iter().position(|x| *x == a) {
            self.fila = p;
        }
    }

    pub fn buscar(&mut self, texto: &str) {
        self.consulta = texto.to_string();
        self.fila = 0;
    }

    /// Índices de las apps que se muestran ahora.
    pub fn visibles(&self) -> Vec<usize> {
        let q = fold(&self.consulta);
        let vista = self.vistas[self.vista];
        self.cat
            .apps
            .iter()
            .enumerate()
            .filter(|(_, a)| a.fuente == self.fuente)
            .filter(|(_, a)| self.estado.ofrecida(&a.paquetes, a.fuente))
            .filter(|(_, a)| {
                if !q.trim().is_empty() {
                    return a.coincide(&q);
                }
                match vista {
                    Vista::Recomendadas => a.recomendada,
                    Vista::Cat(i) => a.categoria == self.cat.categorias[i].id,
                    Vista::Instaladas => self.estado.instalada(&a.paquetes),
                }
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn actual(&self) -> Option<usize> {
        self.visibles().get(self.fila).copied()
    }

    pub fn instalada(&self, a: usize) -> bool {
        self.estado.instalada(&self.cat.apps[a].paquetes)
    }

    pub fn info(&mut self, a: usize) -> Option<Info> {
        let pkg = self.cat.apps[a].paquetes.first()?.clone();
        let fuente = self.fuente;
        self.infos
            .entry(pkg.clone())
            .or_insert_with(|| Info::consultar(&pkg, fuente))
            .clone()
    }

    fn avisar(&mut self, texto: String, ok: bool) {
        self.aviso = Some((texto, ok, Instant::now()));
    }

    fn mover(&mut self, delta: isize) {
        let n = self.visibles().len() as isize;
        if n > 0 {
            self.fila = (self.fila as isize + delta).clamp(0, n - 1) as usize;
        }
    }

    fn mover_vista(&mut self, delta: isize) {
        let n = self.vistas.len() as isize;
        self.vista = (self.vista as isize + delta).clamp(0, n - 1) as usize;
        self.fila = 0;
        self.scroll = 0;
    }

    fn marcar(&mut self) {
        let Some(a) = self.actual() else { return };
        if self.instalada(a) {
            self.avisar(t("msg.already"), false);
        } else if !self.marcadas.remove(&a) {
            self.marcadas.insert(a);
        }
    }

    #[cfg(test)]
    pub fn pedir_instalar_para_prueba(&mut self) {
        self.pedir_instalar();
    }

    fn pedir_instalar(&mut self) {
        let mut objetivos: Vec<usize> = self
            .marcadas
            .iter()
            .copied()
            .filter(|a| !self.instalada(*a))
            .collect();
        if objetivos.is_empty()
            && let Some(a) = self.actual()
        {
            if self.instalada(a) {
                self.avisar(t("msg.already"), false);
                return;
            }
            objetivos.push(a);
        }
        if objetivos.is_empty() {
            return;
        }
        let lineas = objetivos
            .clone()
            .into_iter()
            .map(|a| {
                let dl = self.info(a).map(|i| i.descarga).unwrap_or_default();
                let nombre = self.cat.apps[a].nombre.clone();
                if dl.is_empty() {
                    nombre
                } else {
                    format!("{nombre}  ·  {dl}")
                }
            })
            .collect::<Vec<String>>();
        let mut lineas = lineas;
        if self.fuente == Fuente::Aur {
            lineas.push(t("confirm.aur.note"));
        }
        self.modal = Some(Modal {
            titulo: tf(
                "confirm.install.title",
                &[("n", &objetivos.len().to_string())],
            ),
            lineas,
            tarea: Tarea::Instalar(objetivos),
        });
    }

    fn pedir_quitar(&mut self) {
        let Some(a) = self.actual() else { return };
        if !self.instalada(a) {
            return;
        }
        if !self.cat.apps[a].quitar {
            self.avisar(t("msg.protected"), false);
            return;
        }
        let nombre = self.cat.apps[a].nombre.clone();
        self.modal = Some(Modal {
            titulo: tf("confirm.remove.title", &[("name", &nombre)]),
            lineas: vec![t("confirm.remove.body")],
            tarea: Tarea::Quitar(a),
        });
    }

    fn abrir(&mut self) {
        let Some(a) = self.actual() else { return };
        let app = &self.cat.apps[a];
        match (&app.desktop, self.instalada(a)) {
            (Some(d), true) => match sistema::abrir(d) {
                Ok(()) => {
                    let m = tf("msg.opened", &[("name", &app.nombre)]);
                    self.avisar(m, true);
                }
                Err(_) => self.avisar(t("msg.cant_open"), false),
            },
            _ => self.avisar(t("msg.cant_open"), false),
        }
    }

    fn salir_o_confirmar(&mut self) {
        if self.marcadas.is_empty() {
            self.salir = true;
        } else {
            self.modal = Some(Modal {
                tarea: Tarea::Salir,
                titulo: t("quit.title"),
                lineas: vec![t("quit.body")],
            });
        }
    }

    fn confirmar(&mut self) {
        let Some(m) = self.modal.take() else { return };
        match m.tarea {
            Tarea::Instalar(ref apps) => {
                let mut pkgs: Vec<String> = vec![];
                for a in apps {
                    for p in &self.cat.apps[*a].paquetes {
                        if !pkgs.contains(p) {
                            pkgs.push(p.clone());
                        }
                    }
                }
                self.pendiente = Some((sistema::instalar(&pkgs, self.fuente), m.tarea));
            }
            Tarea::Quitar(a) => {
                let pkgs = self.cat.apps[a].paquetes.clone();
                self.pendiente = Some((sistema::quitar(&pkgs), m.tarea));
            }
            Tarea::Avanzado => {}
            Tarea::Salir => self.salir = true,
        }
    }

    fn avanzado(&mut self) {
        self.pendiente = Some((sistema::avanzado(self.fuente), Tarea::Avanzado));
    }

    fn key_modal(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('s') => self.confirmar(),
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => self.modal = None,
            _ => {}
        }
    }

    fn key_busqueda(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.consulta.clear();
                self.foco = Foco::Lista;
            }
            KeyCode::Enter | KeyCode::Down | KeyCode::Tab => self.foco = Foco::Lista,
            KeyCode::Backspace => {
                self.consulta.pop();
                self.fila = 0;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.consulta.clear();
                self.fila = 0;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.consulta.push(c);
                self.fila = 0;
            }
            _ => {}
        }
    }
}

impl TuiApp for App {
    fn draw(&mut self, f: &mut Frame) {
        self.dibujar(f);
    }

    fn on_key(&mut self, key: KeyEvent) {
        if self.modal.is_some() {
            return self.key_modal(key);
        }
        if self.foco == Foco::Busqueda {
            return self.key_busqueda(key);
        }
        let en_vistas = self.foco == Foco::Vistas;
        match key.code {
            KeyCode::Char('q') => self.salir_o_confirmar(),
            KeyCode::Esc => {
                if self.consulta.is_empty() {
                    self.salir_o_confirmar();
                } else {
                    self.consulta.clear();
                    self.fila = 0;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if en_vistas {
                    self.mover_vista(-1)
                } else {
                    self.mover(-1)
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if en_vistas {
                    self.mover_vista(1)
                } else {
                    self.mover(1)
                }
            }
            KeyCode::PageUp => self.mover(-(self.list_h as isize)),
            KeyCode::PageDown => self.mover(self.list_h as isize),
            KeyCode::Home | KeyCode::Char('g') => self.fila = 0,
            KeyCode::End | KeyCode::Char('G') => self.fila = usize::MAX / 2,
            KeyCode::Left | KeyCode::Char('h') => self.foco = Foco::Vistas,
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.foco = Foco::Lista,
            KeyCode::Char('/') => self.foco = Foco::Busqueda,
            KeyCode::Char(' ') => self.marcar(),
            KeyCode::Enter if en_vistas => self.foco = Foco::Lista,
            KeyCode::Enter | KeyCode::Char('i') => self.pedir_instalar(),
            KeyCode::Char('x') | KeyCode::Delete => self.pedir_quitar(),
            KeyCode::Char('o') => self.abrir(),
            KeyCode::Char('a') => self.avanzado(),
            _ => {}
        }
        let n = self.visibles().len();
        self.fila = self.fila.min(n.saturating_sub(1));
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        let ev = mouse::read(m, &self.hits, &mut self.clicks);
        match ev {
            Mouse::Hover(h) => self.hover = h,
            Mouse::Scroll { down, .. } if self.modal.is_none() => {
                self.mover(if down { 3 } else { -3 })
            }
            Mouse::Click {
                hit: Some(h),
                double,
                ..
            } => {
                if self.modal.is_some() {
                    match h {
                        Hit::Ok => self.confirmar(),
                        Hit::Cancelar => self.modal = None,
                        _ => {}
                    }
                    return;
                }
                match h {
                    Hit::Vista(i) => {
                        self.vista = i;
                        self.fila = 0;
                        self.scroll = 0;
                        self.foco = Foco::Vistas;
                    }
                    Hit::Fila(i) => {
                        self.fila = i;
                        self.foco = Foco::Lista;
                        if double {
                            match self.actual() {
                                Some(a) if self.instalada(a) => self.abrir(),
                                Some(_) => self.pedir_instalar(),
                                None => {}
                            }
                        }
                    }
                    Hit::Buscar => self.foco = Foco::Busqueda,
                    Hit::Instalar => self.pedir_instalar(),
                    Hit::Quitar => self.pedir_quitar(),
                    Hit::Abrir => self.abrir(),
                    Hit::Avanzado => self.avanzado(),
                    Hit::Ok | Hit::Cancelar => {}
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self) {
        if self
            .aviso
            .as_ref()
            .is_some_and(|(_, _, at)| at.elapsed() > Duration::from_secs(6))
        {
            self.aviso = None;
        }
        if let Some(a) = self.actual() {
            self.info(a);
        }
    }

    fn take_command(&mut self) -> Option<Command> {
        self.pendiente.as_ref().map(|(c, _)| c.clone())
    }

    fn after_command(&mut self, result: Result<bool, String>) {
        let Some((_, tarea)) = self.pendiente.take() else {
            return;
        };
        self.estado.recargar_instalados();
        self.infos.clear();
        let ok = matches!(result, Ok(true));
        match (tarea, result) {
            (_, Err(e)) => self.avisar(e, false),
            (Tarea::Instalar(apps), _) => {
                let hechas = apps.iter().filter(|a| self.instalada(**a)).count();
                self.marcadas
                    .retain(|a| !self.estado.instalada(&self.cat.apps[*a].paquetes));
                if ok {
                    self.avisar(tf("msg.installed", &[("n", &hechas.to_string())]), true)
                } else {
                    self.avisar(t("msg.failed"), false)
                }
            }
            (Tarea::Quitar(a), _) => {
                if ok {
                    let m = tf("msg.removed", &[("name", &self.cat.apps[a].nombre)]);
                    self.avisar(m, true)
                } else {
                    self.avisar(t("msg.failed"), false)
                }
            }
            (Tarea::Avanzado | Tarea::Salir, _) => {}
        }
    }

    fn should_quit(&self) -> bool {
        self.salir
    }
}
