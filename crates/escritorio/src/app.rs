//! Estado de Escritorio y manejo de teclado y ratón. Sigue las mismas
//! convenciones que Widgets: barra lateral, formularios, botones Restaurar ·
//! Cancelar · Aplicar, menú contextual y ayuda.

mod lists;

use std::cell::OnceCell;
use std::time::{Duration, Instant};

use lizarbe_core::field::{self, Activation};
use lizarbe_core::flow;
use lizarbe_core::form::{FieldStore, FormState, NoteKind, nudge};
use lizarbe_core::hints;
use lizarbe_core::mouse::{self, Mouse};
use lizarbe_core::popup::{self as cp, HelpContent, PickTarget, PopupTypes};
use lizarbe_core::prefs::Prefs;
use lizarbe_core::schema::{Kind, value_label};
use lizarbe_core::search;
use lizarbe_core::term::{Command, TuiApp};
use lizarbe_core::theme::Palette;
use lizarbe_core::view::CoreHit as _;
pub use lizarbe_core::view::Sub;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use serde_json::Value;

use crate::apps::App as DesktopApp;
pub use crate::catalog::Section;
use crate::catalog::{self, groups};
use crate::i18n::{self, t, tf};
use crate::record::Recorder;
use crate::store::{Bind, Store};
use lizarbe_core::schema::FieldDef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Content,
    /// Botones Restaurar / Cancelar / Aplicar.
    Buttons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Apply,
    Cancel,
    Restore,
}

impl Button {
    /// En el orden en que se dibujan: el principal a la derecha.
    pub const ALL: [Button; 3] = [Button::Apply, Button::Cancel, Button::Restore];

    pub fn label(self) -> String {
        match self {
            Button::Apply => t("btn.apply"),
            Button::Cancel => t("btn.cancel"),
            Button::Restore => t("btn.restore"),
        }
    }

    /// Tecla que lo activa (la que se muestra entre corchetes).
    pub fn key(self) -> &'static str {
        match self {
            Button::Apply => "A",
            Button::Cancel => "C",
            Button::Restore => "R",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Sidebar(usize),
    ModeToggle,
    LangToggle,
    /// Fila de un formulario (zona del nombre).
    Row(usize),
    /// Control de la fila `usize`.
    Ctrl(usize, Sub),
    PopupItem(usize),
    ModalButton(usize),
    MenuItem(usize),
    Button(usize),
    Close,
    Back,
    Search,
}

lizarbe_core::core_hit!(Hit {
    sidebar: Sidebar,
    search: Search,
    mode: Hit::ModeToggle,
    lang: Hit::LangToggle,
});

/// Acciones de filas tipo botón.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    AddBind,
    SearchBinds,
    ClearSearch,
    AddAutostart,
    AddCompose,
    /// Cambiar la tecla de captura de pantalla (graba la combinación).
    CapShortcut,
    /// Ir a Atajos mostrando solo los que contienen este texto.
    OpenBinds(&'static str),
    /// Sistema: elegir de una lista (navegador, energía…).
    SysPick(crate::system::Pick),
    /// Sistema: abrir un panel del shell de Omarchy.
    SysPanel(&'static str),
    /// Sistema: ejecutar una tarea en la terminal.
    SysRun(crate::system::Task),
    /// Sistema: elegir un cambio de Escritorio para deshacerlo.
    UndoPick,
    /// Sistema: abrir un programa gráfico aparte (programa, argumentos).
    SysOpen(&'static str, &'static [&'static str]),
    /// Ir a otra sección.
    Goto(Section),
}

/// Qué recibe la combinación que se está grabando.
#[derive(Debug, Clone, PartialEq)]
pub enum RecTarget {
    /// Un atajo existente (por sus teclas originales).
    Move(String),
    /// Un atajo añadido desde Escritorio (por su posición).
    Custom(usize),
    /// Uno nuevo: después se elige qué hace.
    NewCustom,
}

pub type Row = lizarbe_core::form::Row<Bind, Act>;
pub type FieldRow = lizarbe_core::form::FieldRow<Bind>;

/// Qué hacer cuando el usuario confirma.
#[derive(Debug, Clone, PartialEq)]
pub enum Confirm {
    Apply,
    Discard,
    Quit,
    Restore(Vec<String>),
    /// Asignar teclas que ya usa otro atajo (que se desactivará).
    BindAssign {
        target: RecTarget,
        keys: String,
    },
    RestoreAutostart,
    /// Importar los ajustes de Meca y retirarlo.
    MigrateMeca,
    /// Ejecutar una tarea de Sistema en la terminal.
    SysRun(crate::system::Task),
    /// Deshacer un cambio aplicado (por su fecha).
    Undo(String),
}

/// Cuadros de texto propios de Escritorio.
#[derive(Debug, Clone, PartialEq)]
pub enum EInput {
    BindFilter,
    ComposeKeys,
    ComposeText(String),
    BindCommand(String),
    BindWeb(String),
    AutostartCmd,
}

/// Selectores propios de Escritorio.
#[derive(Debug, Clone, PartialEq)]
pub enum EPick {
    /// Qué hace un atajo nuevo (con sus teclas ya grabadas).
    BindKind(String),
    BindApp(String),
    AutostartApp,
    /// Búsqueda de opciones: salta a la elegida.
    Search,
    /// Sistema: el valor elegido se aplica al momento.
    Sys(crate::system::Pick),
    /// Cambio de Escritorio que se va a deshacer.
    Undo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAct {
    Activate(usize),
    ResetField(usize),
    BindRecord(usize),
    BindDisable(usize),
    CustomRemove(usize),
    AutostartRemove(usize),
    ComposeRemove(usize),
    Apply,
    Cancel,
    Restore,
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub struct E;

impl PopupTypes for E {
    type Bind = Bind;
    type Confirm = Confirm;
    type Input = EInput;
    type Pick = EPick;
    type Menu = MenuAct;
}

pub type Popup = cp::Popup<E>;
type Outcome = cp::Outcome<E>;
type MenuItem = cp::MenuItem<E>;

pub struct Toast {
    pub text: String,
    pub kind: NoteKind,
    until: Instant,
}

pub struct App {
    pub store: Store,
    prefs: Prefs,
    pub pal: Palette,
    pub section: Section,
    pub focus: Focus,
    /// Botón resaltado cuando el foco está en la fila de botones.
    pub button: usize,
    pub advanced: bool,
    pub forms: Vec<FormState>,
    pub popup: Option<Popup>,
    pub toast: Option<Toast>,
    pub hits: Vec<(Rect, Hit)>,
    /// Elemento bajo el ratón.
    pub hover: Option<Hit>,
    pub quit: bool,
    /// Tema cuyos colores se están editando.
    /// Filtro de la lista de atajos.
    pub bind_filter: String,
    /// Grabación de una combinación en curso.
    recording: Option<(Recorder, RecTarget)>,
    /// Aplicaciones instaladas (se leen la primera vez que hacen falta).
    apps: OnceCell<Vec<DesktopApp>>,
    last_check: Instant,
    palette_watch: lizarbe_core::theme::PaletteWatcher,
    clicks: mouse::Clicks<Hit>,
    /// Estado de Sistema (se lee al entrar en la sección).
    pub sys: Option<crate::system::State>,
    /// Comando pendiente de ejecutar en la terminal.
    command: Option<Command>,
}

impl App {
    pub fn new(store: Store, prefs: Prefs) -> App {
        App {
            pal: Palette::load(&store.paths.theme_colors()),
            palette_watch: lizarbe_core::theme::PaletteWatcher::new(&store.paths.theme_colors()),
            advanced: prefs.advanced,
            store,
            prefs,
            section: Section::Appearance,
            focus: Focus::Content,
            button: 0,
            forms: vec![FormState::default(); Section::ALL.len()],
            popup: None,
            toast: None,
            hits: vec![],
            hover: None,
            quit: false,
            bind_filter: String::new(),
            recording: None,
            apps: OnceCell::new(),
            last_check: Instant::now(),
            clicks: mouse::Clicks::default(),
            sys: None,
            command: None,
        }
    }

    fn save_prefs(&self) {
        if !self.store.paths.sandbox {
            self.prefs.save();
        }
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: NoteKind) {
        self.toast = Some(Toast {
            text: text.into(),
            kind,
            until: Instant::now() + Duration::from_secs(5),
        });
    }

    fn message(&mut self, title: String, lines: Vec<String>) {
        self.popup = Some(Popup::Message { title, lines });
    }

    /// Avisos que caducan y recarga si `escritorio.lua` cambió por fuera.
    pub fn tick(&mut self) {
        self.poll_record();
        self.palette_watch.poll(&mut self.pal);
        if self.section == Section::System && self.sys.is_none() {
            self.sys = Some(crate::system::State::load());
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|t| Instant::now() > t.until)
        {
            self.toast = None;
        }
        if self.last_check.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.last_check = Instant::now();
        if self.popup.is_none() && !self.store.dirty() && self.store.external_change() {
            self.store.reload();
            self.toast(t("msg.reloaded_external"), NoteKind::Info);
        }
    }

    /// Ofrece importar los ajustes de Meca si siguen en uso.
    pub fn offer_migration(&mut self) {
        if self.store.meca_pending() {
            self.popup = Some(Popup::Confirm {
                title: t("mig.title"),
                lines: t("mig.body").split('\n').map(String::from).collect(),
                action: Confirm::MigrateMeca,
            });
        }
    }

    pub fn pending(&self) -> usize {
        self.store.changes().len()
    }

    // ------------------------------------------------------------ filas

    pub fn rows(&self) -> Vec<Row> {
        match self.section {
            Section::Changes => self.changes_rows(),
            Section::Keybinds => self.keybind_rows(),
            Section::Autostart => self.autostart_rows(),
            Section::Compose => self.compose_rows(),
            Section::Screenshots => self.capture_rows(),
            Section::System => self.system_rows(),
            s => self.settings_rows(s),
        }
    }

    /// Capturas: los ajustes de siempre más el atajo y los enlaces a otros.
    fn capture_rows(&self) -> Vec<Row> {
        let mut rows = self.settings_rows(Section::Screenshots);
        let keys = match self
            .store
            .values
            .get(&format!("x:bind:{}", crate::capture::DEFAULT_KEYS))
        {
            Some(Value::String(k)) => k.clone(),
            Some(_) => t("cap.disabled"),
            None => crate::capture::DEFAULT_KEYS.to_string(),
        };
        rows.push(Row::Header(t("g.cap_keys")));
        rows.push(Row::Action(
            tf("cap.shortcut", &[("keys", &keys)]),
            Act::CapShortcut,
        ));
        for (label, filter) in [
            ("cap.link.rec", "Screenrecording"),
            ("cap.link.color", "Color picker"),
            ("cap.link.ocr", "OCR"),
        ] {
            rows.push(Row::Action(t(label), Act::OpenBinds(filter)));
        }
        rows
    }

    /// Sistema: estado de Omarchy, con cada valor como botón.
    fn system_rows(&self) -> Vec<Row> {
        use crate::system::{GAMING_MENU, PANELS, PRINTERS_APP, Pick, Printing, Snapper, Task};
        let Some(sys) = &self.sys else {
            return vec![Row::Note(t("sys.loading"), NoteKind::Info)];
        };
        let row = |p: Pick| {
            let value = sys
                .values
                .get(&p)
                .map(|v| p.label(v))
                .unwrap_or_else(|| t("sys.unknown"));
            Row::Action(
                tf(
                    "sys.row",
                    &[
                        ("name", &t(&format!("sys.pick.{}", p.id()))),
                        ("value", &value),
                    ],
                ),
                Act::SysPick(p),
            )
        };
        let mut rows = vec![
            Row::Note(t("sys.instant"), NoteKind::Info),
            Row::Header(t("g.sys_defaults")),
            row(Pick::Browser),
            row(Pick::Terminal),
            row(Pick::Editor),
            Row::Header(t("g.sys_power")),
            row(Pick::Power),
            Row::Header(t("g.sys_font")),
            row(Pick::Font),
            Row::Header(t("g.sys_panels")),
        ];
        for (label, panel, keys) in PANELS {
            rows.push(Row::Action(
                tf(label, &[("keys", keys)]),
                Act::SysPanel(panel),
            ));
        }
        rows.push(Row::Header(t("g.sys_access")));
        rows.push(row(Pick::TextSize));
        rows.push(Row::Action(t("access.zoom"), Act::Goto(Section::Cursor)));
        rows.push(Row::Header(t("g.sys_print")));
        match sys.printing {
            Printing::Missing => rows.push(Row::Note(t("print.missing"), NoteKind::Info)),
            Printing::Disabled => {
                rows.push(Row::Note(t("print.disabled"), NoteKind::Warn));
                rows.push(Row::Action(
                    t(Task::EnablePrinting.label()),
                    Act::SysRun(Task::EnablePrinting),
                ));
            }
            Printing::Ready => {
                let (label, program, args) = PRINTERS_APP;
                if lizarbe_core::ipc::command_exists(program) {
                    rows.push(Row::Action(t(label), Act::SysOpen(program, args)));
                } else {
                    rows.push(Row::Note(t("print.no_app"), NoteKind::Info));
                }
            }
        }
        rows.push(Row::Header(t("g.sys_games")));
        rows.push(Row::Note(t("games.d"), NoteKind::Info));
        let (label, program, args) = GAMING_MENU;
        rows.push(Row::Action(t(label), Act::SysOpen(program, args)));
        rows.push(Row::Header(t("g.sys_repair")));
        rows.push(Row::Note(t("sys.repair.d"), NoteKind::Info));
        for task in Task::REPAIRS {
            rows.push(Row::Action(t(task.label()), Act::SysRun(task)));
        }
        rows.push(Row::Header(t("g.sys_snapshots")));
        match sys.snapper {
            Snapper::Missing => rows.push(Row::Note(t("snap.missing"), NoteKind::Info)),
            Snapper::Unconfigured => {
                rows.push(Row::Note(t("snap.unconfigured"), NoteKind::Warn));
                rows.push(Row::Action(
                    t(Task::SnapperSetup.label()),
                    Act::SysRun(Task::SnapperSetup),
                ));
            }
            Snapper::Ready if sys.in_snapshot => {
                rows.push(Row::Note(t("snap.in_snapshot"), NoteKind::Warn));
                rows.push(Row::Action(
                    t(Task::SnapshotRestore.label()),
                    Act::SysRun(Task::SnapshotRestore),
                ));
            }
            Snapper::Ready => {
                rows.push(Row::Note(t("snap.ready"), NoteKind::Info));
                rows.push(Row::Action(
                    t(Task::SnapshotCreate.label()),
                    Act::SysRun(Task::SnapshotCreate),
                ));
                rows.push(Row::Note(t("snap.how_restore"), NoteKind::Info));
            }
        }
        rows.push(Row::Header(t("g.sys_undo")));
        rows.push(Row::Note(t("undo.d"), NoteKind::Info));
        rows.push(Row::Action(t("undo.pick"), Act::UndoPick));
        rows
    }

    fn settings_rows(&self, section: Section) -> Vec<Row> {
        let mut rows = vec![];
        if !self.store.live && !self.store.paths.sandbox {
            rows.push(Row::Note(t("note.offline"), NoteKind::Warn));
        }
        if self.store.meca_active {
            rows.push(Row::Note(t("note.meca"), NoteKind::Warn));
        }
        if self.store.foreign_file {
            rows.push(Row::Note(t("note.foreign"), NoteKind::Warn));
        }
        for g in groups(section, &self.store.ctx) {
            rows.push(Row::Header(g.title.clone()));
            if g.id == "spacing" && lizarbe_core::hypr::toggle_enabled("window-no-gaps") {
                rows.push(Row::Note(t("note.no_gaps"), NoteKind::Warn));
            }
            if g.id == "monitors_all" {
                if !self.store.global_scale_editable() {
                    rows.push(Row::Note(t("note.no_global_scale"), NoteKind::Info));
                }
                if self.store.gdk_change().is_some() {
                    rows.push(Row::Note(t("note.gdk_mismatch"), NoteKind::Warn));
                }
                if self.store.ctx.monitors.is_empty() {
                    rows.push(Row::Note(t("note.no_monitors"), NoteKind::Info));
                }
            }
            for mut def in g.fields {
                if def.advanced && !self.advanced {
                    continue;
                }
                if def.key == "m:scale" && !self.store.global_scale_editable() {
                    continue;
                }
                def.default = self.store.base(&def);
                let bind = Bind(def.key.clone());
                rows.push(Row::Field(FieldRow::new(def, bind, &self.store)));
            }
        }
        rows
    }

    /// Definición de un ajuste para mostrar su nombre y valores.
    fn def_of(&self, key: &str) -> Option<FieldDef> {
        let mut def = catalog::find(key, &self.store.ctx)?;
        if let Some((name, _)) = catalog::monitor_key(key)
            && !def.label.starts_with(name)
        {
            def.label = tf("mon.label", &[("name", name), ("field", &def.label)]);
        }
        Some(def)
    }

    fn changes_rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::Header(t("ch.header"))];
        let changes = self.store.changes();
        if changes.is_empty() {
            rows.push(Row::Note(t("ch.none"), NoteKind::Info));
        } else {
            for c in changes {
                if let Some((name, old, new)) = self.describe_list_change(&c) {
                    rows.push(Row::Note(
                        tf("ch.key", &[("key", &name), ("old", &old), ("new", &new)]),
                        NoteKind::Info,
                    ));
                    continue;
                }
                let def = self.def_of(&c.key);
                let base = def.as_ref().and_then(|d| self.store.base(d));
                let show = |v: Option<Value>| match (v.or_else(|| base.clone()), &def) {
                    (None, _) => t("ch.omarchy"),
                    (Some(v), Some(d)) => match &d.kind {
                        Kind::Enum(opts) => opts
                            .iter()
                            .find(|o| o.value == v)
                            .map(|o| o.label.clone())
                            .unwrap_or_else(|| value_label(&v)),
                        Kind::Bool => t(if v.as_bool() == Some(true) {
                            "val.on"
                        } else {
                            "val.off"
                        }),
                        _ => value_label(&v),
                    },
                    (Some(v), None) => value_label(&v),
                };
                let name = def
                    .as_ref()
                    .map(|d| d.label.clone())
                    .unwrap_or(c.key.clone());
                rows.push(Row::Note(
                    tf(
                        "ch.key",
                        &[("key", &name), ("old", &show(c.old)), ("new", &show(c.new))],
                    ),
                    NoteKind::Info,
                ));
            }
            rows.push(Row::Note(t("ch.use_buttons"), NoteKind::Info));
        }
        rows.push(Row::Note(
            tf(
                "ch.file",
                &[(
                    "path",
                    &self.store.paths.escritorio_lua().display().to_string(),
                )],
            ),
            NoteKind::Info,
        ));
        rows.push(Row::Note(t("note.base"), NoteKind::Info));
        rows.push(Row::Note(
            tf(
                "ch.backups",
                &[("dir", &self.store.paths.backup_dir.display().to_string())],
            ),
            NoteKind::Info,
        ));
        rows
    }

    fn form_state(&mut self) -> &mut FormState {
        let i = self.section.index();
        &mut self.forms[i]
    }

    // ------------------------------------------------------------ teclado

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('w'))
        {
            return self.request_quit();
        }
        // Mientras se graba, las teclas son para Hyprland.
        if self.recording.is_some() {
            return;
        }
        if let Some(mut popup) = self.popup.take() {
            let outcome = popup.on_key(key);
            self.popup = Some(popup);
            return self.handle_outcome(outcome);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('f') {
            return self.open_search();
        }
        match key.code {
            // En Atajos, `/` sigue buscando atajos (Ctrl+F busca opciones).
            KeyCode::Char('/')
                if !(self.section == Section::Keybinds && self.focus == Focus::Content) =>
            {
                return self.open_search();
            }
            KeyCode::Char('q') => return self.request_quit(),
            KeyCode::Char('?') => return self.open_help(),
            KeyCode::Char('a') => return self.request_apply(),
            KeyCode::Char('c') => return self.request_discard(),
            KeyCode::Char('R') => return self.request_restore(),
            KeyCode::Char('o') => return self.open_menu_for_selection(),
            KeyCode::Char('m') => {
                self.advanced = !self.advanced;
                self.prefs.advanced = self.advanced;
                self.save_prefs();
                let msg = if self.advanced {
                    t("mode.advanced_on")
                } else {
                    t("mode.simple_on")
                };
                return self.toast(msg, NoteKind::Info);
            }
            KeyCode::Char('i') => {
                let l = i18n::lang().toggle();
                i18n::set_lang(l);
                self.prefs.lang = l.code().into();
                return self.save_prefs();
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Content,
                    Focus::Content => {
                        self.button = 0;
                        Focus::Buttons
                    }
                    Focus::Buttons => Focus::Sidebar,
                };
                return;
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Buttons,
                    Focus::Content => Focus::Sidebar,
                    Focus::Buttons => Focus::Content,
                };
                return;
            }
            KeyCode::Char(c @ '1'..='9') => {
                if let Some(s) = Section::ALL.get(c as usize - '1' as usize) {
                    self.go_section(*s);
                }
                return;
            }
            _ => {}
        }
        match self.focus {
            Focus::Sidebar => self.sidebar_key(key),
            Focus::Buttons => self.buttons_key(key),
            Focus::Content => self.form_key(key),
        }
    }

    /// Todas las opciones que se pueden buscar, tal como se ven ahora.
    /// Todas las opciones que se pueden buscar, tal como se ven ahora: las
    /// secciones y las filas de las secciones de ajustes (las listas propias,
    /// como los atajos, tienen su propio buscador).
    pub fn search_entries(&self) -> Vec<search::Entry> {
        let mut out = vec![];
        for (si, s) in Section::ALL.iter().enumerate() {
            let title = s.title();
            out.push(search::section_entry(si, &title));
            let rows = match s {
                Section::Screenshots => self.capture_rows(),
                s if !groups(*s, &self.store.ctx).is_empty() => self.settings_rows(*s),
                _ => continue,
            };
            out.extend(search::entries_from_rows(si, &title, &rows));
        }
        out
    }

    pub fn open_search(&mut self) {
        let target = PickTarget::App(EPick::Search);
        self.popup = Some(Popup::Picker(search::picker(
            &self.search_entries(),
            target,
        )));
    }

    /// Salta a la sección y a la fila de la opción buscada.
    fn jump_to(&mut self, section: usize, key: &str) {
        let Some(s) = Section::ALL.get(section) else {
            return;
        };
        self.go_section(*s);
        if let Some(i) = search::row_of(key) {
            self.form_state().sel = i;
        }
    }

    pub fn go_section(&mut self, s: Section) {
        self.section = s;
        self.focus = Focus::Content;
    }

    fn sidebar_key(&mut self, key: KeyEvent) {
        let i = self.section.index();
        let last = Section::ALL.len() - 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.section = Section::ALL[i.saturating_sub(1)],
            KeyCode::Down | KeyCode::Char('j') => self.section = Section::ALL[(i + 1).min(last)],
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
                self.focus = Focus::Content
            }
            KeyCode::Esc => self.request_quit(),
            _ => {}
        }
    }

    pub fn button_enabled(&self, b: Button) -> bool {
        match b {
            Button::Apply | Button::Cancel => self.store.dirty(),
            Button::Restore => true,
        }
    }

    fn buttons_key(&mut self, key: KeyEvent) {
        let n = Button::ALL.len();
        match key.code {
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {
                let step = if matches!(key.code, KeyCode::Left | KeyCode::Char('h')) {
                    n - 1
                } else {
                    1
                };
                for _ in 0..n {
                    self.button = (self.button + step) % n;
                    if self.button_enabled(Button::ALL[self.button]) {
                        break;
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => self.press(Button::ALL[self.button]),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Esc => self.focus = Focus::Content,
            _ => {}
        }
    }

    pub fn press(&mut self, b: Button) {
        match b {
            Button::Apply => self.request_apply(),
            Button::Cancel => self.request_discard(),
            Button::Restore => self.request_restore(),
        }
    }

    fn form_key(&mut self, key: KeyEvent) {
        let rows = self.rows();
        self.form_state().clamp(&rows);
        let sel = self.form_state().sel;
        match key.code {
            KeyCode::Esc => self.focus = Focus::Sidebar,
            KeyCode::Up | KeyCode::Char('k') => self.form_state().step(&rows, false),
            KeyCode::Down | KeyCode::Char('j') => self.form_state().step(&rows, true),
            KeyCode::PageUp => (0..8).for_each(|_| self.form_state().step(&rows, false)),
            KeyCode::PageDown => (0..8).for_each(|_| self.form_state().step(&rows, true)),
            KeyCode::Home | KeyCode::Char('g') => self.form_state().first(&rows),
            KeyCode::End | KeyCode::Char('G') => self.form_state().last(&rows),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {
                let forward = matches!(key.code, KeyCode::Right | KeyCode::Char('l'));
                if let Some(Row::Field(f)) = rows.get(sel) {
                    nudge(&mut self.store, f, forward);
                } else if !forward {
                    self.focus = Focus::Sidebar;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => match rows.get(sel).cloned() {
                Some(Row::Field(f)) => self.activate_field(f),
                Some(Row::Action(_, act)) => self.run_act(act),
                _ => {}
            },
            KeyCode::Char('/') if self.section == Section::Keybinds => {
                self.run_act(Act::SearchBinds)
            }
            KeyCode::Char('n') if self.section == Section::Keybinds => self.run_act(Act::AddBind),
            KeyCode::Char('n') if self.section == Section::Autostart => {
                self.run_act(Act::AddAutostart)
            }
            KeyCode::Char('n') if self.section == Section::Compose => self.run_act(Act::AddCompose),
            KeyCode::Char('d') | KeyCode::Delete
                if self.section == Section::Compose
                    && matches!(rows.get(sel), Some(Row::Field(_))) =>
            {
                if let Some(Row::Field(f)) = rows.get(sel)
                    && let Some(n) = f.bind.0.strip_prefix("xc:").and_then(|n| n.parse().ok())
                {
                    self.remove_compose(n);
                }
            }
            KeyCode::Char('d') | KeyCode::Delete
                if self.section == Section::Autostart
                    && matches!(rows.get(sel), Some(Row::Field(_))) =>
            {
                if let Some(Row::Field(f)) = rows.get(sel)
                    && let Some(n) = f.bind.0.strip_prefix("as:").and_then(|n| n.parse().ok())
                {
                    self.remove_autostart(n);
                }
            }
            KeyCode::Char('r') | KeyCode::Delete | KeyCode::Backspace => {
                if let Some(Row::Field(f)) = rows.get(sel) {
                    self.store.unset(&f.bind);
                }
            }
            _ => {}
        }
    }

    fn activate_field(&mut self, f: FieldRow) {
        let sel = self.form_state().sel;
        // Los atajos abren su menú (cambiar teclas, desactivar…).
        if Self::is_list_row(&f.bind.0) {
            let at = self
                .hit_rect(Hit::Ctrl(sel, Sub::Main))
                .map(|r| (r.x, r.y + 2))
                .unwrap_or((10, 5));
            return self.open_menu(Some(Hit::Ctrl(sel, Sub::Main)), at);
        }
        let anchor = self.hit_rect(Hit::Ctrl(sel, Sub::Main));
        match field::activate(&f, anchor, None) {
            Activation::Set(v) => self.store.set_field(&f.bind, v, f.def.default.as_ref()),
            Activation::Open(popup) => self.popup = Some(popup),
        }
    }

    // ------------------------------------------------- aplicar / salir

    /// Ajustes que restaura el botón "Restaurar" en la pantalla actual.
    pub fn restore_scope(&self) -> (Vec<String>, &'static str) {
        match self.section {
            Section::Changes => (self.store.values.keys().cloned().collect(), "restore.all"),
            Section::Keybinds => (
                self.store
                    .values
                    .keys()
                    .filter(|k| k.starts_with("x:bind:") || *k == "x:custom_binds")
                    .cloned()
                    .collect(),
                "restore.binds",
            ),
            Section::Autostart => (vec![], "restore.autostart"),
            Section::Compose => (vec![], "restore.compose"),
            Section::Language => (
                self.store
                    .values
                    .keys()
                    .filter(|k| k.starts_with("lg:"))
                    .cloned()
                    .collect(),
                "restore.language",
            ),
            Section::NightLight => (
                self.store
                    .values
                    .keys()
                    .filter(|k| k.starts_with("n:"))
                    .cloned()
                    .collect(),
                "restore.night",
            ),
            s => {
                let mut keys: Vec<String> = groups(s, &self.store.ctx)
                    .into_iter()
                    .flat_map(|g| g.fields)
                    .map(|f| f.key)
                    .collect();
                if s == Section::Screenshots {
                    keys.push(format!("x:bind:{}", crate::capture::DEFAULT_KEYS));
                }
                if s == Section::Monitors {
                    // También las pantallas que ahora no están conectadas.
                    keys.extend(
                        self.store
                            .values
                            .keys()
                            .filter(|k| k.starts_with("x:mon:"))
                            .cloned(),
                    );
                }
                (keys, "restore.section")
            }
        }
    }

    fn request_restore(&mut self) {
        let (keys, k) = self.restore_scope();
        let action = if matches!(self.section, Section::Autostart | Section::Compose) {
            Confirm::RestoreAutostart
        } else {
            Confirm::Restore(keys)
        };
        self.popup = Some(flow::restore(k, action));
    }

    fn request_apply(&mut self) {
        if !self.store.dirty() {
            return self.toast(t("msg.nothing"), NoteKind::Info);
        }
        let lines: Vec<String> = self
            .changes_rows()
            .into_iter()
            .filter_map(|r| match r {
                Row::Note(text, _) if text.contains('→') => Some(text),
                _ => None,
            })
            .collect();
        self.popup = Some(flow::apply(lines, Confirm::Apply));
    }

    fn request_discard(&mut self) {
        if !self.store.dirty() {
            return self.toast(t("msg.nothing"), NoteKind::Info);
        }
        self.popup = Some(flow::discard(Confirm::Discard));
    }

    fn request_quit(&mut self) {
        if self.store.dirty() {
            self.popup = Some(flow::quit(Confirm::Quit));
        } else {
            self.quit = true;
        }
    }

    fn do_apply(&mut self, force: bool) {
        if !force && self.store.external_change() {
            self.popup = Some(Popup::Conflict {
                files: vec!["escritorio.lua"],
            });
            return;
        }
        match self.store.apply() {
            Ok(report) if !report.errors.is_empty() => {
                let mut lines = vec![t("msg.rolled_back")];
                lines.extend(report.errors);
                self.message(t("msg.error"), lines);
            }
            Ok(report) if !report.warnings.is_empty() => {
                self.message(t("msg.warning"), report.warnings)
            }
            Ok(_) if self.store.paths.sandbox => {
                self.toast(t("msg.applied_sandbox"), NoteKind::Info)
            }
            Ok(report) if report.reloaded => self.toast(t("msg.applied"), NoteKind::Info),
            Ok(_) => self.toast(t("msg.applied_offline"), NoteKind::Warn),
            Err(e) => self.message(t("msg.error"), vec![format!("{e:#}")]),
        }
    }

    fn handle_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Stay => {}
            Outcome::Close => self.popup = None,
            Outcome::Confirmed(c) => {
                self.popup = None;
                match c {
                    Confirm::Apply => self.do_apply(false),
                    Confirm::Discard => {
                        self.store.discard();
                        self.toast(t("msg.discarded"), NoteKind::Info);
                    }
                    Confirm::Quit => self.quit = true,
                    Confirm::BindAssign { target, keys } => self.assign(target, keys),
                    Confirm::SysRun(task) => {
                        self.command = Some(task.command(&self.store.paths.omarchy_path));
                    }
                    Confirm::Undo(stamp) => self.undo(&stamp),
                    Confirm::MigrateMeca => match self.store.migrate_meca() {
                        Ok(r) => self.toast(
                            tf(
                                "mig.done",
                                &[("n", &r.settings.to_string()), ("b", &r.binds.to_string())],
                            ),
                            NoteKind::Info,
                        ),
                        Err(e) => self.message(t("mig.failed"), vec![format!("{e:#}")]),
                    },
                    Confirm::RestoreAutostart => {
                        if self.section == Section::Compose {
                            self.store.discard_compose();
                        } else {
                            self.store.discard_autostart();
                        }
                        self.toast(t("msg.restored"), NoteKind::Info);
                    }
                    Confirm::Restore(keys) => {
                        let before = self.store.values.clone();
                        self.store.restore(&keys);
                        let msg = if self.store.values != before {
                            t("msg.restored")
                        } else {
                            t("msg.restored_nothing")
                        };
                        self.toast(msg, NoteKind::Info);
                    }
                }
            }
            Outcome::ConflictOverwrite => {
                self.popup = None;
                self.do_apply(true);
            }
            Outcome::ConflictReload => {
                self.popup = None;
                self.store.reload();
                self.toast(t("msg.reloaded_external"), NoteKind::Info);
            }
            Outcome::Picked(target, value) => {
                self.popup = None;
                match target {
                    PickTarget::Field { bind, def } => {
                        if value == Value::String(field::PICK_CUSTOM.into()) {
                            let f = FieldRow::new(*def, bind, &self.store);
                            self.popup = Some(field::text_input(&f, None));
                        } else {
                            self.store.set_field(&bind, value, def.default.as_ref());
                        }
                    }
                    PickTarget::App(pick) => self.on_pick(pick, value),
                }
            }
            Outcome::Submitted(target, text) => match target {
                cp::InputTarget::Field { bind, def } => {
                    match field::parse_submitted(&def.kind, &text) {
                        Ok(None) => {
                            self.store.unset(&bind);
                            self.popup = None;
                        }
                        Ok(Some(v)) => {
                            self.store.set_field(&bind, v, def.default.as_ref());
                            self.popup = None;
                        }
                        Err(msg) => {
                            if let Some(Popup::Input(inp)) = &mut self.popup {
                                inp.error = Some(msg);
                            }
                        }
                    }
                }
                cp::InputTarget::App(input) => {
                    let before = self.popup.take();
                    if let Err(msg) = self.on_input(input, text) {
                        self.popup = before;
                        if let Some(Popup::Input(inp)) = &mut self.popup {
                            inp.error = Some(msg);
                        }
                    }
                }
            },
            Outcome::Checked(bind, values) => {
                self.popup = None;
                if values.is_empty() {
                    self.store.unset(&bind);
                } else {
                    self.store.set_field(&bind, Value::Array(values), None);
                }
            }
            Outcome::MenuPick(act) => {
                self.popup = None;
                self.run_menu(act);
            }
        }
    }

    // ------------------------------------------------------------ ratón

    fn hit_rect(&self, hit: Hit) -> Option<Rect> {
        mouse::hit_rect(&self.hits, hit)
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        match mouse::read(m, &self.hits, &mut self.clicks) {
            Mouse::Hover(hit) => self.hover = hit,
            Mouse::Scroll { down, hit } => {
                let code = if down { KeyCode::Down } else { KeyCode::Up };
                if self.popup.is_none() && matches!(hit, Some(Hit::Sidebar(_))) {
                    self.focus = Focus::Sidebar;
                    self.sidebar_key(KeyEvent::new(code, KeyModifiers::NONE));
                } else {
                    self.on_key(KeyEvent::new(code, KeyModifiers::NONE));
                }
            }
            Mouse::Click { hit, double, x } => self.click(hit, double, x),
            Mouse::Menu { hit, at }
                if self.popup.is_none() || matches!(self.popup, Some(Popup::Menu(_))) =>
            {
                self.open_menu(hit, at)
            }
            Mouse::Drag { hit, x } => {
                self.hover = hit;
                if let Some(i) = hit.and_then(|h| h.slider_row()) {
                    self.slide(i, x);
                }
            }
            _ => {}
        }
    }

    fn slide(&mut self, row: usize, x: u16) {
        let rows = self.rows();
        if let Some((f, v)) = mouse::slider(&self.hits, &rows, row, x) {
            self.store.set_field(&f.bind, v, f.def.default.as_ref());
        }
    }

    fn click(&mut self, hit: Option<Hit>, double: bool, x: u16) {
        if self.popup.is_some() {
            return self.click_popup(hit, double);
        }
        let key = |c: KeyCode| KeyEvent::new(c, KeyModifiers::NONE);
        match hit {
            Some(Hit::Button(i)) => {
                self.focus = Focus::Buttons;
                self.button = i;
                let b = Button::ALL[i];
                if self.button_enabled(b) {
                    self.press(b);
                }
            }
            Some(Hit::Close) => self.request_quit(),
            Some(Hit::Back) => self.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(Hit::Search) => self.open_search(),
            Some(Hit::Sidebar(i)) => self.go_section(Section::ALL[i]),
            Some(Hit::ModeToggle) => self.on_key(key(KeyCode::Char('m'))),
            Some(Hit::LangToggle) => self.on_key(key(KeyCode::Char('i'))),
            Some(Hit::Row(i)) => {
                self.focus = Focus::Content;
                let rows = self.rows();
                if !rows.get(i).is_some_and(Row::selectable) {
                    return;
                }
                let again = self.form_state().sel == i;
                self.form_state().sel = i;
                if again || double {
                    match rows.get(i).cloned() {
                        Some(Row::Field(f)) => self.activate_field(f),
                        Some(Row::Action(_, act)) => self.run_act(act),
                        _ => {}
                    }
                }
            }
            Some(Hit::Ctrl(i, sub)) => {
                self.focus = Focus::Content;
                self.form_state().sel = i;
                let rows = self.rows();
                match (sub, rows.get(i).cloned()) {
                    (Sub::Minus | Sub::Plus, Some(Row::Field(f))) => {
                        nudge(&mut self.store, &f, sub == Sub::Plus)
                    }
                    (Sub::Slider, _) => self.slide(i, x),
                    (_, Some(Row::Field(f))) => self.activate_field(f),
                    (_, Some(Row::Action(_, act))) => self.run_act(act),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn click_popup(&mut self, hit: Option<Hit>, double: bool) {
        let key = |c: KeyCode| KeyEvent::new(c, KeyModifiers::NONE);
        match hit {
            Some(Hit::PopupItem(i)) => match self.popup.as_mut() {
                Some(Popup::Picker(p)) => {
                    let again = p.sel == i;
                    p.sel = i;
                    if again || double || p.anchor.is_some() {
                        self.on_key(key(KeyCode::Enter));
                    }
                }
                Some(Popup::Checklist(c)) => {
                    c.sel = i;
                    if let Some(x) = c.checked.get_mut(i) {
                        *x = !*x;
                    }
                }
                _ => {}
            },
            Some(Hit::MenuItem(i)) => {
                if let Some(Popup::Menu(m)) = self.popup.as_mut() {
                    m.sel = i;
                }
                self.on_key(key(KeyCode::Enter));
            }
            Some(Hit::ModalButton(i)) => {
                if let Some(code) = self
                    .popup
                    .as_ref()
                    .and_then(|p| p.buttons().get(i).map(|b| b.1))
                {
                    self.on_key(key(code));
                }
            }
            // Un clic fuera (o sobre el control que la abrió) cierra los menús y desplegables.
            _ => {
                let light = matches!(self.popup, Some(Popup::Menu(_)))
                    || matches!(&self.popup, Some(Popup::Picker(p)) if p.anchor.is_some());
                if light {
                    self.popup = None;
                }
            }
        }
    }

    // ------------------------------------------------- menú contextual

    pub fn open_menu(&mut self, hit: Option<Hit>, at: (u16, u16)) {
        self.popup = None;
        let item = |icon: &'static str, label: String, act: MenuAct, enabled: bool| MenuItem {
            icon,
            label,
            act,
            enabled,
            separator: act == MenuAct::Apply,
        };
        let mut items: Vec<MenuItem> = vec![];
        match hit {
            Some(Hit::Row(i)) | Some(Hit::Ctrl(i, _)) => {
                self.focus = Focus::Content;
                self.form_state().sel = i;
                let rows = self.rows();
                if let Some(Row::Field(f)) = rows.get(i)
                    && (Self::is_list_row(&f.bind.0)
                        || f.bind.0.starts_with("as:")
                        || f.bind.0.starts_with("xc:"))
                {
                    items.extend(self.list_menu_items(i, f));
                } else if let Some(Row::Field(f)) = rows.get(i) {
                    let label = match f.def.kind {
                        Kind::Bool => t("menu.toggle"),
                        Kind::Enum(_) | Kind::Multi(_) => t("menu.choose"),
                        _ => t("menu.edit"),
                    };
                    items.push(item("󰏫", label, MenuAct::Activate(i), true));
                    items.push(item(
                        "󰑓",
                        t("menu.reset_field"),
                        MenuAct::ResetField(i),
                        f.value.is_some(),
                    ));
                }
            }
            Some(Hit::Sidebar(i)) => self.section = Section::ALL[i],
            _ => {}
        }
        let dirty = self.store.dirty();
        items.push(item("󰄬", t("btn.apply"), MenuAct::Apply, dirty));
        items.push(item("󰜺", t("btn.cancel"), MenuAct::Cancel, dirty));
        items.push(item("󰑓", t(self.restore_scope().1), MenuAct::Restore, true));
        items.push(item("󰋖", t("menu.help"), MenuAct::Help, true));
        let sel = items.iter().position(|i| i.enabled).unwrap_or(0);
        self.popup = Some(Popup::Menu(cp::Menu { items, sel, at }));
    }

    fn open_menu_for_selection(&mut self) {
        let hit = match self.focus {
            Focus::Content => {
                let sel = self.form_state().sel;
                Some(Hit::Ctrl(sel, Sub::Main))
            }
            _ => None,
        };
        let at = hit
            .and_then(|h| self.hit_rect(h))
            .map(|r| (r.x, r.y + 1))
            .unwrap_or((10, 5));
        self.open_menu(hit, at);
    }

    fn run_menu(&mut self, act: MenuAct) {
        match act {
            MenuAct::Activate(i) => {
                self.form_state().sel = i;
                match self.rows().get(i).cloned() {
                    Some(Row::Field(f)) => self.activate_field(f),
                    Some(Row::Action(_, act)) => self.run_act(act),
                    _ => {}
                }
            }
            MenuAct::ResetField(i) => {
                if let Some(Row::Field(f)) = self.rows().get(i) {
                    self.store.unset(&f.bind);
                }
            }
            MenuAct::BindRecord(i) => {
                let target = match self.rows().get(i) {
                    Some(Row::Field(f)) => {
                        let key = f.bind.0.clone();
                        match key.strip_prefix("x:custom:") {
                            Some(n) => n.parse().ok().map(RecTarget::Custom),
                            None => key
                                .strip_prefix("x:bind:")
                                .map(|o| RecTarget::Move(o.to_string())),
                        }
                    }
                    _ => None,
                };
                if let Some(target) = target {
                    self.start_record(target);
                }
            }
            MenuAct::BindDisable(i) => {
                if let Some(Row::Field(f)) = self.rows().get(i) {
                    self.store.values.insert(f.bind.0.clone(), Value::Null);
                }
            }
            MenuAct::CustomRemove(n) => self.remove_custom(n),
            MenuAct::AutostartRemove(n) => self.remove_autostart(n),
            MenuAct::ComposeRemove(n) => self.remove_compose(n),
            MenuAct::Apply => self.request_apply(),
            MenuAct::Cancel => self.request_discard(),
            MenuAct::Restore => self.request_restore(),
            MenuAct::Help => self.open_help(),
        }
    }

    fn open_help(&mut self) {
        self.popup = Some(Popup::Help {
            scroll: 0,
            content: help_content(),
        });
    }

    // ------------------------------------------------- pistas al pasar

    /// Explicación del elemento bajo el ratón, para la barra de estado.
    pub fn hover_hint(&self) -> Option<String> {
        let hit = self.hover?;
        Some(match hit {
            Hit::Sidebar(i) => Section::ALL[i].description(),
            Hit::ModeToggle => t("hint.mode"),
            Hit::LangToggle => t("hint.lang"),
            Hit::Close => t("hint.close"),
            Hit::Back => t("hint.back"),
            Hit::Search => t("hint.search_options"),
            Hit::Row(i) | Hit::Ctrl(i, Sub::Main) => {
                let rows = self.rows();
                match rows.get(i)? {
                    Row::Field(f) if Self::is_list_row(&f.bind.0) => {
                        format!("{} — {}", f.def.label, t("hint.bind"))
                    }
                    _ => hints::row(&rows, i)?,
                }
            }
            Hit::Ctrl(_, sub) => hints::ctrl(sub)?,
            Hit::Button(i) => match Button::ALL[i] {
                Button::Apply if self.store.dirty() => {
                    tf("btn.apply.desc", &[("n", &self.pending().to_string())])
                }
                Button::Cancel if self.store.dirty() => t("btn.cancel.desc"),
                Button::Apply | Button::Cancel => t("btn.clean"),
                Button::Restore => t(&format!("{}.desc", self.restore_scope().1)),
            },
            Hit::PopupItem(i) => hints::popup_item(self.popup.as_ref(), i)?,
            Hit::MenuItem(_) | Hit::ModalButton(_) => return None,
        })
        .filter(|s| !s.is_empty())
    }
}

/// Contenido de la ventana de ayuda (`?`).
fn help_content() -> HelpContent {
    HelpContent {
        sections: vec![
            (
                t("help.general"),
                vec![
                    ("Tab / ⇧+Tab".into(), t("help.focus")),
                    ("Esc".into(), t("help.back")),
                    (format!("1-{}", Section::ALL.len()), t("help.jump")),
                    ("↑↓ / j k".into(), t("help.nav")),
                    ("← → / h l".into(), t("help.change")),
                    (format!("Enter / {}", t("key.space")), t("help.activate")),
                    (format!("r / {}", t("key.del")), t("help.reset")),
                    ("/ · Ctrl+F".into(), t("help.search")),
                    ("o".into(), t("help.menu")),
                    ("a".into(), t("help.apply")),
                    ("c".into(), t("help.discard")),
                    ("R".into(), t("help.restore")),
                    ("m".into(), t("help.mode")),
                    ("i".into(), t("help.lang")),
                    ("q".into(), t("help.quit")),
                ],
            ),
            (
                t("help.mouse"),
                vec![
                    (t("help.m.hover"), t("help.m.hover.desc")),
                    (t("help.m.click"), t("help.m.click.desc")),
                    (t("help.m.right"), t("help.m.right.desc")),
                    (t("help.m.wheel"), t("help.m.wheel.desc")),
                ],
            ),
        ],
        footer: t("note.base"),
    }
}

impl TuiApp for App {
    fn draw(&mut self, f: &mut Frame) {
        crate::ui::draw(f, self);
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
        self.command.take()
    }

    fn after_command(&mut self, result: Result<bool, String>) {
        match result {
            Ok(true) => self.toast(t("sys.done"), NoteKind::Info),
            Ok(false) => self.toast(t("sys.failed"), NoteKind::Warn),
            Err(e) if e == lizarbe_core::term::CANCELLED => {}
            Err(e) => self.toast(e, NoteKind::Warn),
        }
        self.sys = None;
    }

    fn should_quit(&self) -> bool {
        self.quit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;

    fn app() -> (tempfile::TempDir, App) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("hyprland.lua"),
            "require(\"hypr.looknfeel\")\n",
        )
        .unwrap();
        let store = Store::load(Paths::detect(Some(dir.path().to_path_buf())));
        (dir, App::new(store, Prefs::default()))
    }

    #[test]
    fn search_jumps_to_the_option_in_another_section() {
        let (_d, mut app) = app();
        lizarbe_core::i18n::set_lang(lizarbe_core::i18n::Lang::Es);
        assert_eq!(app.section, Section::Appearance);
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(matches!(app.popup, Some(Popup::Picker(_))));
        for c in "cursor tamano".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.popup.is_none());
        assert_eq!(app.section, Section::Cursor);
        let rows = app.rows();
        let sel = app.form_state().sel;
        assert!(
            matches!(&rows[sel], Row::Field(f) if f.def.key.contains("size")),
            "{:?}",
            rows.get(sel).map(|r| matches!(r, Row::Field(_)))
        );
    }

    #[test]
    fn ctrl_f_opens_the_search_from_the_keybinds_section() {
        let (_d, mut app) = app();
        app.go_section(Section::Keybinds);
        app.on_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(matches!(app.popup, Some(Popup::Picker(_))));
    }

    #[test]
    fn clicking_the_control_again_closes_an_open_menu() {
        let (_d, mut app) = app();
        app.open_menu(None, (10, 5));
        assert!(app.popup.is_some());
        // Un clic sobre cualquier control (el que la abrió, por ejemplo) la cierra.
        app.click(Some(Hit::Ctrl(0, Sub::Main)), false, 0);
        assert!(app.popup.is_none());
    }

    #[test]
    fn clicking_an_item_of_the_menu_does_not_just_close_it() {
        let (_d, mut app) = app();
        app.open_menu(None, (10, 5));
        let before = app.popup.is_some();
        app.click(Some(Hit::MenuItem(0)), false, 0);
        assert!(before);
    }

    #[test]
    fn close_button_and_back_button_work() {
        let (_d, mut app) = app();
        app.focus = Focus::Content;
        app.click(Some(Hit::Back), false, 0);
        assert_eq!(app.focus, Focus::Sidebar);
        app.click(Some(Hit::Close), false, 0);
        // Sin cambios pendientes cierra la aplicación.
        assert!(app.quit);
    }
}
