//! Estado del estudio de temas y manejo de teclado y ratón.
//!
//! Dos pantallas: la lista de temas ("Inicio") y el editor con ocho pestañas y
//! la maqueta en vivo. Probar en el escritorio aplica el tema de verdad con
//! `omarchy-theme-set` y vuelve al anterior si no se confirma.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use lizarbe_core::field::{self, Activation};
use lizarbe_core::form::{FieldRow, FieldStore, FormState, NoteKind, Row, nudge};
use lizarbe_core::hints;
use lizarbe_core::mouse::{self, Mouse};
use lizarbe_core::popup::{self as cp, HelpContent, PickItem, PickTarget, Picker, PopupTypes};
use lizarbe_core::schema::{FieldDef, Kind, Opt, float, int};
use lizarbe_core::search;
use lizarbe_core::term::{Command, TuiApp};
use lizarbe_core::theme::Palette;
use lizarbe_core::themes::{Dirs, ThemeSpec, images_in, slugify};
use lizarbe_core::view::CoreHit as _;
pub use lizarbe_core::view::Sub;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::layout::Rect;
use serde_json::{Value, json};

use crate::draft::{Bind, Catalog, Draft};
use crate::i18n::{t, tf};
use crate::images::Images;
use crate::palette;

pub type Popup = cp::Popup<T>;
pub type Outcome = cp::Outcome<T>;
pub type MenuItem = cp::MenuItem<T>;
pub type Row2 = Row<Bind, Act>;

/// Carpeta temporal del tema que se prueba en el escritorio.
pub const TRIAL_SLUG: &str = "lizarbe-prueba";
const TRIAL_SECS: u64 = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct T;

#[derive(Debug, Clone, PartialEq)]
pub enum Confirm {
    Quit,
    BackHome,
    DeleteTheme(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TIn {
    /// Nombre del tema nuevo, partiendo de la carpeta `base`.
    NewName(PathBuf),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TPick {
    Base,
    AddBackground,
    PaletteImage,
    /// Búsqueda de opciones: salta a la elegida.
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAct {
    AddBackground,
    RemoveBackground(usize),
    MoveBackground(usize, bool),
    RemoveInherited,
    /// Fila de la lista de inicio.
    OpenTheme(usize),
    DeleteTheme(usize),
}

impl PopupTypes for T {
    type Bind = Bind;
    type Confirm = Confirm;
    type Input = TIn;
    type Pick = TPick;
    type Menu = MenuAct;
}

/// Acciones de filas tipo botón.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    NewTheme,
    Open(String),
    GenAccent,
    GenImage,
    AddBackground,
    /// Un fondo de la lista (Enter solo muestra la ayuda; D lo quita).
    Background(usize),
    Save,
    SaveActivate,
    Try,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tabs,
    Editor,
    Buttons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Theme,
    Palette,
    Interface,
    Terminal,
    Backgrounds,
    Icons,
    Surfaces,
    Save,
}

impl Tab {
    pub const ALL: [Tab; 8] = [
        Tab::Theme,
        Tab::Palette,
        Tab::Interface,
        Tab::Terminal,
        Tab::Backgrounds,
        Tab::Icons,
        Tab::Surfaces,
        Tab::Save,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Tab::Theme => "tab.theme",
            Tab::Palette => "tab.palette",
            Tab::Interface => "tab.interface",
            Tab::Terminal => "tab.terminal",
            Tab::Backgrounds => "tab.backgrounds",
            Tab::Icons => "tab.icons",
            Tab::Surfaces => "tab.surfaces",
            Tab::Save => "tab.save",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Save,
    Try,
    Undo,
}

impl Button {
    pub const ALL: [Button; 3] = [Button::Save, Button::Try, Button::Undo];

    pub fn label(self) -> String {
        match self {
            Button::Save => t("btn.save"),
            Button::Try => t("btn.try"),
            Button::Undo => t("btn.undo"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Button::Save => "S",
            Button::Try => "P",
            Button::Undo => "Z",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Tab(usize),
    Row(usize),
    Ctrl(usize, Sub),
    PopupItem(usize),
    ModalButton(usize),
    MenuItem(usize),
    Button(usize),
    Close,
    Back,
    Search,
    Keep,
    Revert,
}

// Las pestañas hacen de barra lateral; el Estudio no tiene interruptores.
lizarbe_core::core_hit!(Hit {
    sidebar: Tab,
    search: Search,
    mode: Hit::Close,
    lang: Hit::Close,
});

pub struct Toast {
    pub text: String,
    pub kind: NoteKind,
    until: Instant,
}

/// Prueba del tema en el escritorio real.
pub struct Trial {
    /// Tema que había antes (al que se vuelve).
    pub prev: String,
    pub deadline: Instant,
}

/// Un tema propio en la pantalla de inicio.
pub struct HomeItem {
    pub slug: String,
    pub mode: String,
    /// Tira de colores (hex): fondo, texto, acento y los de terminal.
    pub strip: Vec<String>,
}

enum Done {
    TrialApplied {
        prev: String,
        res: Result<(), String>,
    },
    Restored,
    Activated(Result<(), String>),
    Captured(Result<PathBuf, String>),
}

pub struct App {
    pub pal: Palette,
    pub dirs: Dirs,
    pub sandbox: bool,
    pub screen: Screen,
    pub focus: Focus,
    pub tab: usize,
    pub button: usize,
    pub home_sel: usize,
    pub home: Vec<HomeItem>,
    pub draft: Option<Draft>,
    pub forms: Vec<FormState>,
    pub popup: Option<Popup>,
    pub toast: Option<Toast>,
    pub hits: Vec<(Rect, Hit)>,
    pub hover: Option<Hit>,
    pub quit: bool,
    pub trial: Option<Trial>,
    pub busy: Option<String>,
    pub images: Images,
    /// Imagen de la que se está sacando una paleta (esperando a que cargue).
    pending_palette: Option<PathBuf>,
    catalog: Catalog,
    rx: Option<Receiver<Done>>,
    clicks: mouse::Clicks<Hit>,
    cache_dir: PathBuf,
    /// Colores del tema activo de Omarchy: si cambian (por ejemplo al probar un
    /// tema) la interfaz recarga su paleta.
    palette_watch: lizarbe_core::theme::PaletteWatcher,
}

fn pretty(slug: &str) -> String {
    slug.replace('-', " ")
}

impl App {
    pub fn new(
        dirs: Dirs,
        pal: Palette,
        sandbox: bool,
        cache_dir: PathBuf,
        picker: ratatui_image::picker::Picker,
        colors_path: PathBuf,
    ) -> App {
        let catalog = Catalog::load(&dirs);
        let mut app = App {
            pal,
            dirs,
            sandbox,
            screen: Screen::Home,
            focus: Focus::Editor,
            tab: 0,
            button: 0,
            home_sel: 0,
            home: vec![],
            draft: None,
            forms: vec![FormState::default(); Tab::ALL.len() + 1],
            popup: None,
            toast: None,
            hits: vec![],
            hover: None,
            quit: false,
            trial: None,
            busy: None,
            images: Images::new(picker),
            pending_palette: None,
            catalog,
            rx: None,
            clicks: mouse::Clicks::default(),
            palette_watch: lizarbe_core::theme::PaletteWatcher::new(&colors_path),
            cache_dir,
        };
        app.reload_home();
        app
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

    pub fn reload_home(&mut self) {
        self.home = self
            .dirs
            .list_user()
            .into_iter()
            .filter(|s| s != TRIAL_SLUG)
            .map(|slug| {
                let spec = self.dirs.find(&slug).and_then(|d| ThemeSpec::load(&d).ok());
                let strip = spec
                    .as_ref()
                    .map(|s| {
                        [
                            "background",
                            "foreground",
                            "accent",
                            "red",
                            "green",
                            "yellow",
                            "blue",
                            "magenta",
                        ]
                        .iter()
                        .filter_map(|k| s.colors.get(*k).cloned())
                        .collect()
                    })
                    .unwrap_or_default();
                HomeItem {
                    mode: spec.map(|s| s.mode).unwrap_or_default(),
                    slug,
                    strip,
                }
            })
            .collect();
        self.home_sel = self.home_sel.min(self.home.len());
    }

    pub fn tab_kind(&self) -> Tab {
        Tab::ALL[self.tab.min(Tab::ALL.len() - 1)]
    }

    pub fn dirty(&self) -> bool {
        self.draft.as_ref().is_some_and(Draft::dirty)
    }

    fn form_state(&mut self) -> &mut FormState {
        &mut self.forms[self.tab]
    }

    // ------------------------------------------------------------ filas

    fn field(&self, key: &str, label: &str, desc: &str, kind: Kind) -> Option<Row2> {
        let d = self.draft.as_ref()?;
        let def = FieldDef::new(key, t(label), kind).desc(t(desc));
        Some(Row::Field(FieldRow::new(def, Bind(key.to_string()), d)))
    }

    fn color(&self, key: &str) -> Option<Row2> {
        self.field(
            &format!("c:{key}"),
            &format!("c.{key}"),
            &format!("c.{key}.d"),
            Kind::Color,
        )
    }

    pub fn rows(&self) -> Vec<Row2> {
        match self.screen {
            Screen::Home => self.home_rows(),
            Screen::Edit => self.edit_rows(),
        }
    }

    fn home_rows(&self) -> Vec<Row2> {
        let mut rows = vec![
            Row::Note(t("home.intro"), NoteKind::Info),
            Row::Action(t("home.new"), Act::NewTheme),
        ];
        if self.home.is_empty() {
            rows.push(Row::Note(t("home.none"), NoteKind::Info));
        } else {
            rows.push(Row::Header(t("home.mine")));
            for h in &self.home {
                rows.push(Row::Action(h.slug.clone(), Act::Open(h.slug.clone())));
            }
        }
        rows
    }

    fn edit_rows(&self) -> Vec<Row2> {
        let Some(d) = self.draft.as_ref() else {
            return vec![];
        };
        let mut rows: Vec<Row2> = vec![];
        match self.tab_kind() {
            Tab::Theme => {
                rows.push(Row::Header(t("g.identity")));
                let mut r = vec![];
                if d.editing.is_none() {
                    r.push(self.field("name", "o.name", "o.name.d", Kind::Text));
                } else {
                    rows.push(Row::Note(
                        tf("note.editing", &[("name", &d.slug())]),
                        NoteKind::Info,
                    ));
                }
                let mode = vec![
                    Opt::new(json!("dark"), t("o.mode=dark")),
                    Opt::new(json!("light"), t("o.mode=light")),
                ];
                r.push(self.field("mode", "o.mode", "o.mode.d", Kind::Enum(mode)));
                rows.extend(r.into_iter().flatten());
                if let Some(b) = &d.base {
                    rows.push(Row::Note(
                        tf("note.base", &[("base", &b.display().to_string())]),
                        NoteKind::Info,
                    ));
                }
                rows.push(Row::Note(
                    tf(
                        "note.dest",
                        &[("dir", &self.dirs.user.join(d.slug()).display().to_string())],
                    ),
                    NoteKind::Info,
                ));
            }
            Tab::Palette => {
                rows.push(Row::Header(t("g.brand")));
                for k in ["accent", "selection", "muted"] {
                    rows.extend(self.color(k));
                }
                rows.push(Row::Header(t("g.generate")));
                rows.push(Row::Note(t("note.generate"), NoteKind::Info));
                rows.push(Row::Action(t("act.gen_accent"), Act::GenAccent));
                rows.push(Row::Action(t("act.gen_image"), Act::GenImage));
            }
            Tab::Interface => {
                rows.push(Row::Header(t("g.surfaces")));
                for k in [
                    "background",
                    "dark_background",
                    "darker_background",
                    "lighter_background",
                ] {
                    rows.extend(self.color(k));
                }
                rows.push(Row::Header(t("g.text")));
                for k in [
                    "foreground",
                    "dark_foreground",
                    "light_foreground",
                    "bright_foreground",
                ] {
                    rows.extend(self.color(k));
                }
                for (side, title) in [
                    ("active", "g.border_active"),
                    ("inactive", "g.border_inactive"),
                ] {
                    rows.push(Row::Header(t(title)));
                    rows.extend(self.field(
                        &format!("b:{side}:from"),
                        "o.b.from",
                        "o.b.from.d",
                        Kind::Color,
                    ));
                    rows.extend(self.field(
                        &format!("b:{side}:grad"),
                        "o.b.grad",
                        "o.b.grad.d",
                        Kind::Bool,
                    ));
                    if d.spec.active_border.to.is_some() && side == "active"
                        || d.spec.inactive_border.to.is_some() && side == "inactive"
                    {
                        rows.extend(self.field(
                            &format!("b:{side}:to"),
                            "o.b.to",
                            "o.b.to.d",
                            Kind::Color,
                        ));
                        rows.extend(self.field(
                            &format!("b:{side}:angle"),
                            "o.b.angle",
                            "o.b.angle.d",
                            int(Some(0), Some(360), 15),
                        ));
                    }
                }
            }
            Tab::Terminal => {
                rows.push(Row::Header(t("g.ansi_normal")));
                for k in [
                    "red", "green", "yellow", "blue", "magenta", "cyan", "orange", "brown",
                ] {
                    rows.extend(self.color(k));
                }
                rows.push(Row::Header(t("g.ansi_bright")));
                for k in [
                    "bright_red",
                    "bright_green",
                    "bright_yellow",
                    "bright_blue",
                    "bright_magenta",
                    "bright_cyan",
                ] {
                    rows.extend(self.color(k));
                }
            }
            Tab::Backgrounds => {
                rows.push(Row::Note(t("note.backgrounds"), NoteKind::Info));
                rows.push(Row::Action(t("act.add_bg"), Act::AddBackground));
                if d.spec.backgrounds.is_empty() {
                    rows.push(Row::Note(t("note.no_backgrounds"), NoteKind::Warn));
                } else {
                    rows.push(Row::Header(tf(
                        "g.backgrounds",
                        &[("n", &d.spec.backgrounds.len().to_string())],
                    )));
                    for (i, p) in d.spec.backgrounds.iter().enumerate() {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                        let mut label = if i == 0 {
                            tf("bg.first", &[("name", name)])
                        } else {
                            name.to_string()
                        };
                        if d.is_inherited(p) {
                            label = format!("{label}  {}", t("bg.inherited"));
                        }
                        rows.push(Row::Action(label, Act::Background(i)));
                    }
                }
            }
            Tab::Icons => {
                rows.push(Row::Header(t("g.icons")));
                let icons: Vec<Opt> = self
                    .catalog
                    .icons
                    .iter()
                    .map(|n| Opt::new(json!(n), n.clone()))
                    .collect();
                rows.extend(self.field("icons", "o.icons", "o.icons.d", Kind::Enum(icons)));
                rows.push(Row::Header(t("g.editors")));
                let names = |v: &[(String, String)]| {
                    let mut o = vec![Opt::new(json!(""), t("val.none"))];
                    o.extend(v.iter().map(|(n, _)| Opt::new(json!(n), n.clone())));
                    o
                };
                rows.extend(self.field(
                    "nvim",
                    "o.nvim",
                    "o.nvim.d",
                    Kind::Enum(names(&self.catalog.nvim)),
                ));
                rows.extend(self.field(
                    "vscode",
                    "o.vscode",
                    "o.vscode.d",
                    Kind::Enum(names(&self.catalog.vscode)),
                ));
                rows.push(Row::Header(t("g.keyboard")));
                rows.extend(self.field("kb", "o.kb", "o.kb.d", Kind::Color));
            }
            Tab::Surfaces => {
                rows.push(Row::Note(t("note.surfaces"), NoteKind::Info));
                rows.extend(self.field("sf:on", "o.sf.on", "o.sf.on.d", Kind::Bool));
                if d.spec.surfaces.is_some() {
                    rows.push(Row::Header(t("g.bar")));
                    rows.extend(self.field(
                        "sf:bar_bg",
                        "o.sf.bar_bg",
                        "o.sf.bar_bg.d",
                        Kind::Color,
                    ));
                    rows.extend(self.field(
                        "sf:bar_alpha",
                        "o.sf.alpha",
                        "o.sf.alpha.d",
                        float(Some(0.2), Some(1.0), 0.05),
                    ));
                    rows.extend(self.field("sf:bar_text", "o.sf.text", "o.sf.text.d", Kind::Color));
                    rows.extend(self.field(
                        "sf:bar_active",
                        "o.sf.bar_active",
                        "o.sf.bar_active.d",
                        Kind::Color,
                    ));
                    rows.push(Row::Header(t("g.card")));
                    rows.extend(self.field(
                        "sf:card_bg",
                        "o.sf.card_bg",
                        "o.sf.card_bg.d",
                        Kind::Color,
                    ));
                    rows.extend(self.field(
                        "sf:card_alpha",
                        "o.sf.alpha",
                        "o.sf.alpha.d",
                        float(Some(0.2), Some(1.0), 0.02),
                    ));
                    rows.extend(self.field(
                        "sf:card_text",
                        "o.sf.text",
                        "o.sf.text.d",
                        Kind::Color,
                    ));
                    rows.extend(self.field(
                        "sf:card_border",
                        "o.sf.card_border",
                        "o.sf.card_border.d",
                        Kind::Color,
                    ));
                    rows.extend(self.field(
                        "sf:card_selected",
                        "o.sf.card_sel",
                        "o.sf.card_sel.d",
                        Kind::Color,
                    ));
                    rows.push(Row::Header(t("g.lock")));
                    rows.extend(self.field(
                        "sf:lock_bg",
                        "o.sf.lock_bg",
                        "o.sf.lock_bg.d",
                        Kind::Color,
                    ));
                    rows.extend(self.field(
                        "sf:lock_text",
                        "o.sf.text",
                        "o.sf.text.d",
                        Kind::Color,
                    ));
                    rows.extend(self.field(
                        "sf:lock_border",
                        "o.sf.card_border",
                        "o.sf.card_border.d",
                        Kind::Color,
                    ));
                }
            }
            Tab::Save => {
                rows.push(Row::Header(t("g.summary")));
                rows.push(Row::Note(
                    tf(
                        "note.save_to",
                        &[("dir", &self.dirs.user.join(d.slug()).display().to_string())],
                    ),
                    NoteKind::Info,
                ));
                rows.push(Row::Note(
                    tf(
                        "note.save_files",
                        &[
                            ("bg", &d.spec.backgrounds.len().to_string()),
                            ("icons", &d.spec.icons),
                        ],
                    ),
                    NoteKind::Info,
                ));
                if d.name.trim().is_empty() && d.editing.is_none() {
                    rows.push(Row::Note(t("note.need_name"), NoteKind::Warn));
                }
                rows.push(Row::Header(t("g.actions")));
                rows.push(Row::Action(t("act.save"), Act::Save));
                rows.push(Row::Action(t("act.save_activate"), Act::SaveActivate));
                rows.push(Row::Action(t("act.try"), Act::Try));
            }
        }
        rows
    }

    /// El borrador con el valor que se está eligiendo en la ventana de color,
    /// para ver el cambio en la maqueta antes de aceptarlo.
    pub fn preview_spec(&self) -> Option<ThemeSpec> {
        let d = self.draft.as_ref()?;
        if let Some(cp::Popup::Color(c)) = &self.popup
            && let PickTarget::Field { bind, .. } = &c.target
        {
            let mut copy = d.clone();
            copy.set_field_silent(bind, &json!(c.hex()));
            return Some(copy.spec);
        }
        Some(d.spec.clone())
    }

    // ------------------------------------------------------------ pantallas

    fn open_draft(
        &mut self,
        spec: ThemeSpec,
        name: String,
        base: Option<PathBuf>,
        editing: Option<String>,
    ) {
        self.draft = Some(Draft::new(spec, name, base, editing, self.catalog.clone()));
        self.screen = Screen::Edit;
        self.tab = 0;
        self.focus = Focus::Editor;
        self.forms = vec![FormState::default(); Tab::ALL.len() + 1];
    }

    pub fn open_theme(&mut self, slug: &str) {
        let Some(dir) = self.dirs.find(slug) else {
            return;
        };
        match ThemeSpec::load(&dir) {
            Ok(spec) => self.open_draft(spec, slug.to_string(), None, Some(slug.to_string())),
            Err(e) => self.message(t("msg.error"), vec![format!("{e:#}")]),
        }
    }

    pub fn start_new(&mut self, base: &str) {
        let Some(dir) = self.dirs.find(base) else {
            return;
        };
        let title = t("new.title");
        self.popup = Some(Popup::Input(cp::Input::new(
            title,
            t("new.hint"),
            "",
            cp::InputTarget::App(TIn::NewName(dir)),
        )));
    }

    fn base_picker(&self) -> Popup {
        let items: Vec<PickItem> = self
            .dirs
            .list_all()
            .into_iter()
            .filter(|s| s != TRIAL_SLUG)
            .map(|s| PickItem {
                detail: if self.dirs.is_user(&s) {
                    t("pick.mine")
                } else {
                    String::new()
                },
                label: pretty(&s),
                group: String::new(),
                value: json!(s),
                enabled: true,
            })
            .collect();
        Popup::Picker(Picker {
            title: t("pick.base"),
            items,
            sel: 0,
            filter: String::new(),
            target: PickTarget::App(TPick::Base),
            current: None,
            anchor: None,
        })
    }

    /// Imágenes que se pueden usar como fondo: carpetas del usuario y fondos de otros temas.
    fn background_candidates(&self) -> Vec<PickItem> {
        let home = dirs::home_dir().unwrap_or_default();
        let mut out: Vec<PickItem> = vec![];
        let mut add = |group: String, files: Vec<PathBuf>| {
            for p in files {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                out.push(PickItem {
                    detail: p
                        .parent()
                        .map(|d| d.display().to_string())
                        .unwrap_or_default(),
                    label: name,
                    group: group.clone(),
                    value: json!(p.display().to_string()),
                    enabled: true,
                });
            }
        };
        for (title, dir) in [
            (t("grp.pictures"), home.join("Pictures")),
            (t("grp.pictures"), home.join("Imágenes")),
            (t("grp.wallpapers"), home.join("Pictures/Wallpapers")),
        ] {
            let mut files = images_in(&dir);
            files.truncate(60);
            add(title, files);
        }
        for slug in self.dirs.list_all() {
            if slug == TRIAL_SLUG {
                continue;
            }
            if let Some(d) = self.dirs.find(&slug) {
                add(
                    tf("grp.theme", &[("name", &pretty(&slug))]),
                    images_in(&d.join("backgrounds")),
                );
            }
        }
        out
    }

    // ------------------------------------------------------------ teclado

    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('w')) {
            return self.request_quit();
        }
        if self.trial.is_some() && self.popup.is_none() {
            return self.trial_key(key);
        }
        if let Some(mut popup) = self.popup.take() {
            let outcome = popup.on_key(key);
            self.popup = Some(popup);
            return self.handle_outcome(outcome);
        }
        if self.screen == Screen::Home {
            return self.home_key(key);
        }
        if ctrl && key.code == KeyCode::Char('f') {
            return self.open_search();
        }
        match key.code {
            KeyCode::Char('/') => return self.open_search(),
            KeyCode::Char('q') => return self.request_quit(),
            KeyCode::Char('?') => return self.open_help(),
            KeyCode::Char('s') => return self.save(false),
            KeyCode::Char('a') => return self.save(true),
            KeyCode::Char('p') => return self.start_try(),
            KeyCode::Char('z') => return self.undo(),
            KeyCode::Char('g') if self.tab_kind() == Tab::Palette => {
                return self.run_act(Act::GenAccent);
            }
            KeyCode::Char('f') if self.tab_kind() == Tab::Palette => {
                return self.run_act(Act::GenImage);
            }
            KeyCode::Char(c @ '1'..='8') => {
                self.tab = c as usize - '1' as usize;
                self.focus = Focus::Editor;
                return;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Tabs => Focus::Editor,
                    Focus::Editor => Focus::Buttons,
                    Focus::Buttons => Focus::Tabs,
                };
                return;
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Tabs => Focus::Buttons,
                    Focus::Editor => Focus::Tabs,
                    Focus::Buttons => Focus::Editor,
                };
                return;
            }
            _ => {}
        }
        match self.focus {
            Focus::Tabs => self.tabs_key(key),
            Focus::Buttons => self.buttons_key(key),
            Focus::Editor => self.form_key(key),
        }
    }

    fn tabs_key(&mut self, key: KeyEvent) {
        let last = Tab::ALL.len() - 1;
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.tab = self.tab.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => self.tab = (self.tab + 1).min(last),
            KeyCode::Down | KeyCode::Enter | KeyCode::Char('j') => self.focus = Focus::Editor,
            KeyCode::Esc => self.back(),
            _ => {}
        }
    }

    fn buttons_key(&mut self, key: KeyEvent) {
        let n = Button::ALL.len();
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.button = (self.button + n - 1) % n,
            KeyCode::Right | KeyCode::Char('l') => self.button = (self.button + 1) % n,
            KeyCode::Enter | KeyCode::Char(' ') => self.press(Button::ALL[self.button]),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Esc => self.focus = Focus::Editor,
            _ => {}
        }
    }

    pub fn press(&mut self, b: Button) {
        match b {
            Button::Save => self.save(false),
            Button::Try => self.start_try(),
            Button::Undo => self.undo(),
        }
    }

    fn form_key(&mut self, key: KeyEvent) {
        let rows = self.rows();
        self.form_state().clamp(&rows);
        let sel = self.form_state().sel;
        match key.code {
            KeyCode::Esc => self.focus = Focus::Tabs,
            KeyCode::Up | KeyCode::Char('k') => self.form_state().step(&rows, false),
            KeyCode::Down | KeyCode::Char('j') => self.form_state().step(&rows, true),
            KeyCode::PageUp => (0..8).for_each(|_| self.form_state().step(&rows, false)),
            KeyCode::PageDown => (0..8).for_each(|_| self.form_state().step(&rows, true)),
            KeyCode::Home | KeyCode::Char('g') => self.form_state().first(&rows),
            KeyCode::End | KeyCode::Char('G') => self.form_state().last(&rows),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {
                let forward = matches!(key.code, KeyCode::Right | KeyCode::Char('l'));
                if let (Some(Row::Field(f)), Some(d)) = (rows.get(sel), self.draft.as_mut()) {
                    nudge(d, f, forward);
                } else if !forward {
                    self.focus = Focus::Tabs;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => match rows.get(sel).cloned() {
                Some(Row::Field(f)) => self.activate_field(f),
                Some(Row::Action(_, act)) => self.run_act(act),
                _ => {}
            },
            KeyCode::Char('n') if self.tab_kind() == Tab::Backgrounds => {
                self.run_act(Act::AddBackground)
            }
            KeyCode::Char('m') => self.open_menu_for_selection(),
            KeyCode::Char('d') | KeyCode::Delete if self.tab_kind() == Tab::Backgrounds => {
                if let Some(Row::Action(_, Act::Background(i))) = rows.get(sel) {
                    self.remove_background(*i);
                }
            }
            KeyCode::Char('[') | KeyCode::Char(']') if self.tab_kind() == Tab::Backgrounds => {
                if let Some(Row::Action(_, Act::Background(i))) = rows.get(sel) {
                    self.move_background(*i, key.code == KeyCode::Char(']'));
                }
            }
            KeyCode::Char('r') | KeyCode::Backspace => {
                if let (Some(Row::Field(f)), Some(d)) = (rows.get(sel), self.draft.as_mut()) {
                    d.unset(&f.bind);
                }
            }
            _ => {}
        }
    }

    fn home_key(&mut self, key: KeyEvent) {
        let rows = self.home_rows();
        self.form_state_home().clamp(&rows);
        let sel = self.form_state_home().sel;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Up | KeyCode::Char('k') => self.form_state_home().step(&rows, false),
            KeyCode::Down | KeyCode::Char('j') => self.form_state_home().step(&rows, true),
            KeyCode::Char('n') => self.run_act(Act::NewTheme),
            KeyCode::Char('m') => self.open_menu_for_selection(),
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(Row::Action(_, Act::Open(slug))) = rows.get(sel) {
                    self.popup = Some(Popup::Confirm {
                        title: t("del.title"),
                        lines: vec![tf("del.body", &[("name", slug)])],
                        action: Confirm::DeleteTheme(slug.clone()),
                    });
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(Row::Action(_, act)) = rows.get(sel).cloned() {
                    self.run_act(act);
                }
            }
            _ => {}
        }
    }

    /// El estado de lista de la pantalla de inicio comparte el último hueco.
    fn form_state_home(&mut self) -> &mut FormState {
        let i = self.forms.len() - 1;
        &mut self.forms[i]
    }

    fn open_help(&mut self) {
        let s = |k: &str, d: &str| (k.to_string(), t(d));
        self.popup = Some(Popup::Help {
            scroll: 0,
            content: HelpContent {
                sections: vec![
                    (
                        t("help.s.edit"),
                        vec![
                            s("1-8", "help.tabs"),
                            s("↑↓  j k", "help.move"),
                            s("←→  h l", "help.change"),
                            s("Enter", "help.edit"),
                            s("r", "help.reset"),
                            s("/ · Ctrl+F", "help.search"),
                            s("m", "help.menu"),
                            s("z", "help.undo"),
                        ],
                    ),
                    (
                        t("help.s.theme"),
                        vec![
                            s("s", "help.save"),
                            s("a", "help.activate"),
                            s("p", "help.try"),
                            s("g", "help.generate"),
                            s("Esc", "help.back"),
                            s("q", "help.quit"),
                        ],
                    ),
                ],
                footer: t("help.footer"),
            },
        });
    }

    // ------------------------------------------------------------ acciones

    fn activate_field(&mut self, f: FieldRow<Bind>) {
        let sel = self.form_state().sel;
        let anchor = self.hit_rect(Hit::Ctrl(sel, Sub::Main));
        match field::activate::<T>(&f, anchor, None) {
            Activation::Set(v) => {
                if let Some(d) = self.draft.as_mut() {
                    d.set_field(&f.bind, v, f.def.default.as_ref());
                }
            }
            Activation::Open(mut popup) => {
                // El selector de color ofrece los colores del tema como atajos 1-9.
                if let cp::Popup::Color(c) = &mut popup
                    && let Some(d) = &self.draft
                {
                    c.swatches = [
                        "accent",
                        "background",
                        "foreground",
                        "red",
                        "green",
                        "yellow",
                        "blue",
                        "magenta",
                        "cyan",
                    ]
                    .iter()
                    .filter_map(|k| d.spec.colors.get(*k).cloned())
                    .collect();
                }
                self.popup = Some(popup);
            }
        }
    }

    fn run_act(&mut self, act: Act) {
        match act {
            Act::NewTheme => self.popup = Some(self.base_picker()),
            Act::Open(slug) => self.open_theme(&slug),
            Act::GenAccent => self.gen_from_accent(),
            Act::GenImage => self.pick_palette_image(),
            Act::AddBackground => {
                let items = self.background_candidates();
                if items.is_empty() {
                    self.toast(t("msg.no_images"), NoteKind::Warn);
                } else {
                    self.popup = Some(Popup::Picker(Picker {
                        title: t("pick.background"),
                        items,
                        sel: 0,
                        filter: String::new(),
                        target: PickTarget::App(TPick::AddBackground),
                        current: None,
                        anchor: None,
                    }));
                }
            }
            Act::Background(_) => self.open_menu_for_selection(),
            Act::Save => self.save(false),
            Act::SaveActivate => self.save(true),
            Act::Try => self.start_try(),
        }
    }

    fn undo(&mut self) {
        if let Some(d) = self.draft.as_mut() {
            if d.undo() {
                self.toast(t("msg.undone"), NoteKind::Info);
            } else {
                self.toast(t("msg.nothing_to_undo"), NoteKind::Info);
            }
        }
    }

    /// Elige la imagen de la que sacar la paleta: los fondos del tema o cualquier otra.
    fn pick_palette_image(&mut self) {
        let mut items: Vec<PickItem> = self
            .draft
            .as_ref()
            .map(|d| {
                d.spec
                    .backgrounds
                    .iter()
                    .map(|p| PickItem {
                        label: p
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("?")
                            .to_string(),
                        detail: t("grp.this_theme"),
                        group: t("grp.this_theme"),
                        value: json!(p.display().to_string()),
                        enabled: true,
                    })
                    .collect()
            })
            .unwrap_or_default();
        items.extend(self.background_candidates());
        if items.is_empty() {
            return self.toast(t("msg.no_images"), NoteKind::Warn);
        }
        self.popup = Some(Popup::Picker(Picker {
            title: t("pick.palette_image"),
            items,
            sel: 0,
            filter: String::new(),
            target: PickTarget::App(TPick::PaletteImage),
            current: None,
            anchor: None,
        }));
    }

    /// Aplica la paleta sacada de los píxeles de `path` (si ya se cargó).
    fn apply_image_palette(&mut self, path: &Path) {
        let Some(px) = self.images.pixels(path).cloned() else {
            return;
        };
        let Some((colors, light)) = palette::from_pixels(&px) else {
            return self.toast(t("msg.palette_failed"), NoteKind::Warn);
        };
        let Some(d) = self.draft.as_mut() else { return };
        d.push_undo();
        for (k, v) in colors {
            d.spec.colors.insert(k, v);
        }
        d.spec.mode = if light { "light".into() } else { "dark".into() };
        d.spec.active_border = lizarbe_core::themes::Border::solid(&d.spec.colors["accent"]);
        self.toast(t("msg.generated_image"), NoteKind::Info);
    }

    fn gen_from_accent(&mut self) {
        let Some(d) = self.draft.as_mut() else { return };
        let Some(acc) = d
            .spec
            .colors
            .get("accent")
            .and_then(|a| lizarbe_core::color::parse_hex(a))
        else {
            return;
        };
        let light = d.spec.mode == "light";
        let old_accent = d.spec.colors.get("accent").cloned();
        let generated = palette::from_accent(acc, light, None);
        d.push_undo();
        for (k, v) in generated {
            d.spec.colors.insert(k, v);
        }
        // Si el borde activo seguía al acento, sigue al nuevo.
        if d.spec.active_border.to.is_none()
            && Some(&d.spec.active_border.from) == old_accent.as_ref()
        {
            d.spec.active_border.from = d.spec.colors["accent"].clone();
        }
        self.toast(t("msg.generated"), NoteKind::Info);
    }

    /// Opciones de todas las pestañas del borrador (la clave es la posición
    /// de la fila dentro de su pestaña).
    pub fn search_entries(&mut self) -> Vec<search::Entry> {
        let saved = self.tab;
        let mut out = vec![];
        for (ti, tab) in Tab::ALL.iter().enumerate() {
            self.tab = ti;
            let title = t(tab.key());
            out.push(search::section_entry(ti, &title));
            out.extend(search::entries_from_rows(ti, &title, &self.edit_rows()));
        }
        self.tab = saved;
        out
    }

    pub fn open_search(&mut self) {
        if self.screen != Screen::Edit {
            return;
        }
        let entries = self.search_entries();
        let target = PickTarget::App(TPick::Search);
        self.popup = Some(Popup::Picker(search::picker(&entries, target)));
    }

    fn jump_to(&mut self, tab: usize, key: &str) {
        if tab >= Tab::ALL.len() {
            return;
        }
        self.tab = tab;
        self.focus = Focus::Editor;
        if let Some(i) = search::row_of(key) {
            self.form_state().sel = i;
        }
    }

    fn list_state(&mut self) -> &mut FormState {
        if self.screen == Screen::Home {
            self.form_state_home()
        } else {
            self.form_state()
        }
    }

    /// Menú de clic derecho (o tecla `m`) sobre una fila.
    fn open_menu(&mut self, hit: Option<Hit>, at: (u16, u16)) {
        let Some(Hit::Row(i) | Hit::Ctrl(i, _)) = hit else {
            return;
        };
        let rows = self.rows();
        let item =
            |icon: &'static str, label: String, act: MenuAct, enabled: bool, sep: bool| MenuItem {
                icon,
                label,
                act,
                enabled,
                separator: sep,
            };
        let mut items: Vec<MenuItem> = vec![];
        match rows.get(i) {
            Some(Row::Action(_, Act::Background(bi))) if self.screen == Screen::Edit => {
                let bi = *bi;
                let (n, inherited) = self
                    .draft
                    .as_ref()
                    .map(|d| {
                        (
                            d.spec.backgrounds.len(),
                            d.spec.backgrounds.iter().any(|p| d.is_inherited(p)),
                        )
                    })
                    .unwrap_or((0, false));
                items.push(item(
                    "󰆴",
                    t("menu.bg.remove"),
                    MenuAct::RemoveBackground(bi),
                    true,
                    false,
                ));
                items.push(item(
                    "󰁝",
                    t("menu.bg.up"),
                    MenuAct::MoveBackground(bi, false),
                    bi > 0,
                    false,
                ));
                items.push(item(
                    "󰁅",
                    t("menu.bg.down"),
                    MenuAct::MoveBackground(bi, true),
                    bi + 1 < n,
                    false,
                ));
                items.push(item(
                    "󰋩",
                    t("menu.bg.inherited"),
                    MenuAct::RemoveInherited,
                    inherited,
                    true,
                ));
                items.push(item(
                    "󰐕",
                    t("menu.bg.add"),
                    MenuAct::AddBackground,
                    true,
                    false,
                ));
            }
            Some(Row::Action(_, Act::AddBackground)) if self.screen == Screen::Edit => {
                items.push(item(
                    "󰐕",
                    t("menu.bg.add"),
                    MenuAct::AddBackground,
                    true,
                    false,
                ));
            }
            Some(Row::Action(_, Act::Open(_))) if self.screen == Screen::Home => {
                items.push(item(
                    "󰏫",
                    t("menu.open"),
                    MenuAct::OpenTheme(i),
                    true,
                    false,
                ));
                items.push(item(
                    "󰆴",
                    t("menu.delete"),
                    MenuAct::DeleteTheme(i),
                    true,
                    true,
                ));
            }
            _ => {}
        }
        if items.is_empty() {
            return;
        }
        self.list_state().sel = i;
        self.focus = Focus::Editor;
        self.popup = Some(Popup::Menu(cp::Menu { items, sel: 0, at }));
    }

    fn open_menu_for_selection(&mut self) {
        let sel = self.list_state().sel;
        let at = self
            .hit_rect(Hit::Row(sel))
            .map(|r| (r.x + 2, r.y + 1))
            .unwrap_or((10, 5));
        self.open_menu(Some(Hit::Row(sel)), at);
    }

    fn run_menu(&mut self, act: MenuAct) {
        match act {
            MenuAct::AddBackground => self.run_act(Act::AddBackground),
            MenuAct::RemoveBackground(i) => self.remove_background(i),
            MenuAct::MoveBackground(i, down) => self.move_background(i, down),
            MenuAct::RemoveInherited => {
                if let Some(d) = self.draft.as_mut() {
                    d.push_undo();
                    let keep: Vec<_> = d
                        .spec
                        .backgrounds
                        .iter()
                        .filter(|p| !d.is_inherited(p))
                        .cloned()
                        .collect();
                    d.spec.backgrounds = keep;
                }
            }
            MenuAct::OpenTheme(i) | MenuAct::DeleteTheme(i) => {
                let Some(Row::Action(_, Act::Open(slug))) = self.rows().get(i).cloned() else {
                    return;
                };
                if matches!(act, MenuAct::OpenTheme(_)) {
                    self.open_theme(&slug);
                } else {
                    self.popup = Some(Popup::Confirm {
                        title: t("del.title"),
                        lines: vec![tf("del.body", &[("name", &slug)])],
                        action: Confirm::DeleteTheme(slug),
                    });
                }
            }
        }
    }

    fn remove_background(&mut self, i: usize) {
        if let Some(d) = self.draft.as_mut()
            && i < d.spec.backgrounds.len()
        {
            d.push_undo();
            d.spec.backgrounds.remove(i);
        }
    }

    fn move_background(&mut self, i: usize, down: bool) {
        if let Some(d) = self.draft.as_mut() {
            let n = d.spec.backgrounds.len();
            let j = if down { i + 1 } else { i.wrapping_sub(1) };
            if i < n && j < n {
                d.push_undo();
                d.spec.backgrounds.swap(i, j);
            }
        }
    }

    fn back(&mut self) {
        if self.dirty() {
            self.popup = Some(Popup::Confirm {
                title: t("back.title"),
                lines: vec![t("back.body")],
                action: Confirm::BackHome,
            });
        } else {
            self.go_home();
        }
    }

    fn go_home(&mut self) {
        self.draft = None;
        self.screen = Screen::Home;
        self.reload_home();
    }

    fn request_quit(&mut self) {
        if self.screen == Screen::Edit && self.dirty() {
            self.popup = Some(Popup::Confirm {
                title: t("quit.title"),
                lines: vec![t("quit.body")],
                action: Confirm::Quit,
            });
        } else {
            self.quit = true;
        }
    }

    // ------------------------------------------------------------ guardar y probar

    /// Escribe el borrador en la carpeta de temas del usuario.
    fn write_theme(&mut self) -> Result<String, String> {
        let Some(d) = self.draft.as_mut() else {
            return Err("sin borrador".into());
        };
        let slug = d.slug();
        if slug.is_empty() || slug.contains('/') || slug == TRIAL_SLUG {
            return Err(t("note.need_name"));
        }
        let dest = self.dirs.user.join(&slug);
        if d.editing.is_none() && dest.exists() {
            return Err(t("err.exists"));
        }
        let base = d.base.clone();
        d.spec
            .save(&dest, base.as_deref())
            .map_err(|e| format!("{e:#}"))?;
        d.editing = Some(slug.clone());
        d.orig = d.spec.clone();
        d.orig_name = d.name.clone();
        d.base = None;
        Ok(slug)
    }

    fn save(&mut self, activate: bool) {
        if self.screen != Screen::Edit {
            return;
        }
        match self.write_theme() {
            Ok(slug) => {
                self.toast(tf("msg.saved", &[("name", &slug)]), NoteKind::Info);
                if activate {
                    self.spawn_activate(slug);
                }
            }
            Err(e) => self.message(t("msg.error"), vec![e]),
        }
    }

    fn spawn<F: FnOnce() -> Done + Send + 'static>(&mut self, label: String, f: F) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.busy = Some(label);
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
    }

    fn spawn_activate(&mut self, slug: String) {
        if self.sandbox {
            self.toast(tf("msg.activated", &[("name", &slug)]), NoteKind::Info);
            return;
        }
        self.spawn(t("busy.activate"), move || {
            Done::Activated(lizarbe_core::ipc::run("omarchy-theme-set", &[&slug]).map(|_| ()))
        });
    }

    fn start_try(&mut self) {
        if self.screen != Screen::Edit || self.busy.is_some() || self.trial.is_some() {
            return;
        }
        let Some(d) = self.draft.as_ref() else { return };
        let dest = self.dirs.user.join(TRIAL_SLUG);
        let base = d.base.clone();
        if let Err(e) = d.spec.save(&dest, base.as_deref()) {
            return self.message(t("msg.error"), vec![format!("{e:#}")]);
        }
        if self.sandbox {
            self.trial = Some(Trial {
                prev: "sandbox".into(),
                deadline: Instant::now() + Duration::from_secs(TRIAL_SECS),
            });
            return;
        }
        self.spawn(t("busy.apply"), move || {
            let prev = lizarbe_core::ipc::run("omarchy-theme-current", &[]).unwrap_or_default();
            let res = lizarbe_core::ipc::run("omarchy-theme-set", &[TRIAL_SLUG]).map(|_| ());
            Done::TrialApplied { prev, res }
        });
    }

    fn trial_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('k') | KeyCode::Enter => self.keep_trial(),
            KeyCode::Esc | KeyCode::Char('q') => self.revert_trial(),
            KeyCode::Char('c') => self.capture(),
            _ => {}
        }
    }

    fn keep_trial(&mut self) {
        self.trial = None;
        let _ = self.dirs.delete(TRIAL_SLUG);
        self.save(true);
    }

    fn revert_trial(&mut self) {
        let Some(tr) = self.trial.take() else { return };
        let _ = self.dirs.delete(TRIAL_SLUG);
        if self.sandbox {
            return self.toast(t("msg.reverted"), NoteKind::Info);
        }
        let prev = tr.prev;
        self.spawn(t("busy.revert"), move || {
            if !prev.is_empty() {
                let _ = lizarbe_core::ipc::run("omarchy-theme-set", &[&prev]);
            }
            Done::Restored
        });
    }

    /// Captura la pantalla como vista previa del tema (`preview.png`).
    fn capture(&mut self) {
        if self.busy.is_some() {
            return;
        }
        if self.sandbox {
            return self.toast(t("msg.sandbox_capture"), NoteKind::Warn);
        }
        let shot = self.cache_dir.join("captura.png");
        let out = self.cache_dir.join("preview.png");
        let _ = std::fs::create_dir_all(&self.cache_dir);
        self.images.forget(&out);
        self.spawn(t("busy.capture"), move || {
            // El estudio se esconde un momento para que no salga en la captura.
            let hidden = hide_studio();
            std::thread::sleep(Duration::from_millis(700));
            let o = shot.display().to_string();
            let res = lizarbe_core::ipc::run("grim", &[&o])
                .and_then(|_| crate::images::save_preview(&shot, &out))
                .map(|_| out);
            if let Some((addr, ws)) = hidden {
                let _ = lizarbe_core::ipc::run(
                    "hyprctl",
                    &[
                        "dispatch",
                        "movetoworkspacesilent",
                        &format!("{ws},address:{addr}"),
                    ],
                );
            }
            Done::Captured(res)
        });
    }

    fn apply_pending_palette(&mut self) {
        if let Some(p) = self.pending_palette.clone()
            && self.images.pixels(&p).is_some()
        {
            self.pending_palette = None;
            self.apply_image_palette(&p);
        }
    }

    pub fn tick(&mut self) {
        self.palette_watch.poll(&mut self.pal);
        if !self.images.poll().is_empty() {
            self.apply_pending_palette();
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|t| Instant::now() > t.until)
        {
            self.toast = None;
        }
        if let Some(tr) = &self.trial
            && Instant::now() >= tr.deadline
            && self.busy.is_none()
        {
            self.revert_trial();
        }
        let done = self.rx.as_ref().and_then(|rx| rx.try_recv().ok());
        let Some(done) = done else { return };
        self.rx = None;
        self.busy = None;
        match done {
            Done::TrialApplied { prev, res } => match res {
                Ok(()) => {
                    self.trial = Some(Trial {
                        prev,
                        deadline: Instant::now() + Duration::from_secs(TRIAL_SECS),
                    });
                }
                Err(e) => {
                    let _ = self.dirs.delete(TRIAL_SLUG);
                    self.message(t("msg.error"), vec![e]);
                }
            },
            Done::Restored => self.toast(t("msg.reverted"), NoteKind::Info),
            Done::Activated(Ok(())) => self.toast(t("msg.activated_ok"), NoteKind::Info),
            Done::Activated(Err(e)) => self.message(t("msg.error"), vec![e]),
            Done::Captured(Ok(path)) => {
                if let Some(d) = self.draft.as_mut() {
                    d.spec.preview = Some(path);
                }
                self.toast(t("msg.captured"), NoteKind::Info);
            }
            Done::Captured(Err(e)) => self.message(t("msg.error"), vec![e]),
        }
    }

    // ------------------------------------------------------------ ventanas emergentes

    fn handle_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Stay => {}
            Outcome::Close => self.popup = None,
            Outcome::Confirmed(c) => {
                self.popup = None;
                match c {
                    Confirm::Quit => self.quit = true,
                    Confirm::BackHome => self.go_home(),
                    Confirm::DeleteTheme(slug) => match self.dirs.delete(&slug) {
                        Ok(()) => {
                            self.reload_home();
                            self.toast(tf("msg.deleted", &[("name", &slug)]), NoteKind::Info);
                        }
                        Err(e) => self.message(t("msg.error"), vec![format!("{e:#}")]),
                    },
                }
            }
            Outcome::Picked(target, value) => {
                self.popup = None;
                match target {
                    PickTarget::Field { bind, def } => {
                        if value == Value::String(field::PICK_CUSTOM.into()) {
                            return;
                        }
                        if let Some(d) = self.draft.as_mut() {
                            d.set_field(&bind, value, def.default.as_ref());
                        }
                    }
                    PickTarget::App(TPick::Search) => {
                        if let Some((tab, key)) = lizarbe_core::search::decode(&value) {
                            self.jump_to(tab, &key);
                        }
                    }
                    PickTarget::App(TPick::Base) => {
                        if let Some(slug) = value.as_str() {
                            self.start_new(slug);
                        }
                    }
                    PickTarget::App(TPick::PaletteImage) => {
                        if let Some(p) = value.as_str() {
                            let path = PathBuf::from(p);
                            self.images.request(&path);
                            self.pending_palette = Some(path.clone());
                            self.toast(t("busy.palette"), NoteKind::Info);
                            self.apply_pending_palette();
                        }
                    }
                    PickTarget::App(TPick::AddBackground) => {
                        if let (Some(p), Some(d)) = (value.as_str(), self.draft.as_mut()) {
                            let path = PathBuf::from(p);
                            if !d.spec.backgrounds.contains(&path) {
                                d.push_undo();
                                d.spec.backgrounds.push(path);
                            }
                        }
                    }
                }
            }
            Outcome::Submitted(target, text) => match target {
                cp::InputTarget::Field { bind, def } => {
                    match field::parse_submitted(&def.kind, &text) {
                        Ok(v) => {
                            if let Some(d) = self.draft.as_mut() {
                                match v {
                                    Some(v) => d.set_field(&bind, v, def.default.as_ref()),
                                    None => d.unset(&bind),
                                }
                            }
                            self.popup = None;
                        }
                        Err(msg) => {
                            if let Some(Popup::Input(inp)) = &mut self.popup {
                                inp.error = Some(msg);
                            }
                        }
                    }
                }
                cp::InputTarget::App(TIn::NewName(base)) => {
                    let slug = slugify(&text);
                    let err = if slug.is_empty() {
                        Some(t("err.empty_name"))
                    } else if self.dirs.is_user(&slug) || self.dirs.find(&slug).is_some() {
                        Some(t("err.exists"))
                    } else {
                        None
                    };
                    match err {
                        Some(msg) => {
                            if let Some(Popup::Input(inp)) = &mut self.popup {
                                inp.error = Some(msg);
                            }
                        }
                        None => {
                            self.popup = None;
                            match ThemeSpec::load(&base) {
                                Ok(spec) => {
                                    self.open_draft(spec, text.trim().to_string(), Some(base), None)
                                }
                                Err(e) => self.message(t("msg.error"), vec![format!("{e:#}")]),
                            }
                        }
                    }
                }
            },
            Outcome::Checked(..) | Outcome::ConflictOverwrite | Outcome::ConflictReload => {
                self.popup = None;
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

    /// Explicación del elemento bajo el ratón, para la barra de estado.
    pub fn hover_hint(&self) -> Option<String> {
        Some(match self.hover? {
            Hit::Tab(i) => t(&format!("{}.d", Tab::ALL.get(i)?.key())),
            Hit::Close => t("hint.close"),
            Hit::Back => t("hint.back"),
            Hit::Search => t("hint.search_options"),
            Hit::Row(i) | Hit::Ctrl(i, Sub::Main) => hints::row(&self.rows(), i)?,
            Hit::Ctrl(_, sub) => hints::ctrl(sub)?,
            Hit::PopupItem(i) => hints::popup_item(self.popup.as_ref(), i)?,
            _ => return None,
        })
        .filter(|s| !s.is_empty())
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        match mouse::read(m, &self.hits, &mut self.clicks) {
            Mouse::Hover(hit) => self.hover = hit,
            Mouse::Scroll { down, .. } => {
                let code = if down { KeyCode::Down } else { KeyCode::Up };
                self.on_key(KeyEvent::new(code, KeyModifiers::NONE));
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
        if let (Some((f, v)), Some(d)) = (
            mouse::slider(&self.hits, &rows, row, x),
            self.draft.as_mut(),
        ) {
            d.set_field(&f.bind, v, f.def.default.as_ref());
        }
    }

    fn click(&mut self, hit: Option<Hit>, double: bool, x: u16) {
        let key = |c: KeyCode| KeyEvent::new(c, KeyModifiers::NONE);
        if self.popup.is_some() {
            return self.click_popup(hit, double);
        }
        match hit {
            Some(Hit::Close) => self.request_quit(),
            Some(Hit::Back) => self.on_key(key(KeyCode::Esc)),
            Some(Hit::Search) => self.open_search(),
            Some(Hit::Keep) => self.keep_trial(),
            Some(Hit::Revert) => self.revert_trial(),
            Some(Hit::Tab(i)) => {
                self.tab = i.min(Tab::ALL.len() - 1);
                self.focus = Focus::Editor;
            }
            Some(Hit::Button(i)) => {
                self.focus = Focus::Buttons;
                self.button = i;
                self.press(Button::ALL[i]);
            }
            Some(Hit::Row(i)) | Some(Hit::Ctrl(i, Sub::Main)) => {
                self.focus = Focus::Editor;
                let rows = self.rows();
                if !rows.get(i).is_some_and(Row::selectable) {
                    return;
                }
                let st = if self.screen == Screen::Home {
                    self.form_state_home()
                } else {
                    self.form_state()
                };
                let again = st.sel == i;
                st.sel = i;
                let on_ctrl = matches!(hit, Some(Hit::Ctrl(..)));
                if again || double || on_ctrl || self.screen == Screen::Home {
                    match rows.get(i).cloned() {
                        Some(Row::Field(f)) => self.activate_field(f),
                        Some(Row::Action(_, act)) => self.run_act(act),
                        _ => {}
                    }
                }
            }
            Some(Hit::Ctrl(i, sub @ (Sub::Minus | Sub::Plus))) => {
                let rows = self.rows();
                if let (Some(Row::Field(f)), Some(d)) = (rows.get(i), self.draft.as_mut()) {
                    nudge(d, f, sub == Sub::Plus);
                }
            }
            Some(Hit::Ctrl(i, Sub::Slider)) => self.slide(i, x),
            _ => {}
        }
    }

    fn click_popup(&mut self, hit: Option<Hit>, double: bool) {
        let key = |c: KeyCode| KeyEvent::new(c, KeyModifiers::NONE);
        match hit {
            Some(Hit::PopupItem(i)) => match self.popup.as_mut() {
                Some(cp::Popup::Picker(p)) => {
                    let again = p.sel == i;
                    p.sel = i;
                    if again || double || p.anchor.is_some() {
                        self.on_key(key(KeyCode::Enter));
                    }
                }
                Some(cp::Popup::Checklist(c)) => c.sel = i,
                _ => {}
            },
            Some(Hit::MenuItem(i)) => {
                if let Some(cp::Popup::Menu(m)) = self.popup.as_mut() {
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
            _ => {
                let light = matches!(self.popup, Some(cp::Popup::Menu(_)))
                    || matches!(&self.popup, Some(cp::Popup::Picker(p)) if p.anchor.is_some());
                if light {
                    self.popup = None;
                }
            }
        }
    }

    pub fn countdown(&self) -> Option<u64> {
        self.trial.as_ref().map(|t| {
            t.deadline
                .saturating_duration_since(Instant::now())
                .as_secs()
                + 1
        })
    }
}

/// Mueve la ventana del estudio a un espacio de trabajo oculto y devuelve su
/// dirección y el espacio donde estaba, para devolverla después.
fn hide_studio() -> Option<(String, String)> {
    let json = lizarbe_core::ipc::run("hyprctl", &["clients", "-j"]).ok()?;
    let clients: Vec<Value> = serde_json::from_str(&json).ok()?;
    let me = clients
        .iter()
        .find(|c| c["class"].as_str() == Some("org.omarchy.lizarbe-temas"))?;
    let addr = me["address"].as_str()?.to_string();
    let ws = me["workspace"]["id"].as_i64()?.to_string();
    lizarbe_core::ipc::run(
        "hyprctl",
        &[
            "dispatch",
            "movetoworkspacesilent",
            &format!("special:lizarbe-temas,address:{addr}"),
        ],
    )
    .ok()?;
    Some((addr, ws))
}

impl Drop for App {
    fn drop(&mut self) {
        // Si se cierra a mitad de una prueba, el escritorio vuelve a su tema.
        if let Some(tr) = self.trial.take() {
            let _ = self.dirs.delete(TRIAL_SLUG);
            if !self.sandbox && !tr.prev.is_empty() {
                let _ = lizarbe_core::ipc::run("omarchy-theme-set", &[&tr.prev]);
            }
        }
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
        None
    }
    fn after_command(&mut self, _result: Result<bool, String>) {}
    fn should_quit(&self) -> bool {
        self.quit
    }
}
