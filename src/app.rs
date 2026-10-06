//! Estado de la aplicación y manejo de teclado/ratón.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use serde_json::{Value, json};

use crate::i18n::{self, t, tf};
use crate::omarchy::catalog::Plugin;
use crate::omarchy::curated;
use crate::omarchy::ipc;
use crate::omarchy::schema::{self, FieldDef, Kind};
use crate::omarchy::shell_json::{self as sj, key};
use crate::omarchy::shell_toml as st;
use crate::omarchy::theme::Palette;
use crate::prefs::Prefs;
use crate::store::{Bind, Scope, Store};
use crate::ui::form::{self, Act, FieldRow, FormState, NoteKind, Row};
use crate::ui::popup::{
    Checklist, Confirm, Input, InputTarget, Outcome, PickItem, PickTarget, Picker, Popup, fold,
};

/// Marcadores de opciones especiales en los selectores.
const PICK_CUSTOM: &str = "\u{0}custom";
const PICK_CMD: &str = "\u{0}command";
const PICK_QML: &str = "\u{0}qml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Bar,
    Widgets,
    Plugins,
    Idle,
    Appearance,
    Changes,
}

impl Section {
    pub const ALL: [Section; 6] = [
        Section::Bar,
        Section::Widgets,
        Section::Plugins,
        Section::Idle,
        Section::Appearance,
        Section::Changes,
    ];

    pub fn index(self) -> usize {
        Section::ALL.iter().position(|s| *s == self).unwrap()
    }

    pub fn icon(self) -> &'static str {
        match self {
            Section::Bar => "󰍜",
            Section::Widgets => "󰕮",
            Section::Plugins => "󰐱",
            Section::Idle => "󰒲",
            Section::Appearance => "󰏘",
            Section::Changes => "󰄬",
        }
    }

    pub fn title(self) -> String {
        t(match self {
            Section::Bar => "sec.bar",
            Section::Widgets => "sec.widgets",
            Section::Plugins => "sec.plugins",
            Section::Idle => "sec.idle",
            Section::Appearance => "sec.appearance",
            Section::Changes => "sec.changes",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Content,
    /// Botones Aplicar / Cancelar / Restaurar.
    Buttons,
}

/// Botones de acción fijos al pie del panel de contenido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Apply,
    Cancel,
    Restore,
}

impl Button {
    pub const ALL: [Button; 3] = [Button::Apply, Button::Cancel, Button::Restore];

    pub fn label(self) -> String {
        match self {
            Button::Apply => format!("󰄬  {}  a", t("btn.apply")),
            Button::Cancel => format!("󰜺  {}  c", t("btn.cancel")),
            Button::Restore => format!("󰑓  {}  R", t("btn.restore")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Sidebar(usize),
    Row(usize),
    Widget(usize, usize),
    Column(usize),
    Plugin(usize),
    PopupItem(usize),
    Button(usize),
}

#[derive(Debug, Default)]
pub struct WidgetsView {
    pub col: usize,
    pub row: [usize; 3],
    pub editing: Option<(usize, usize)>,
    pub form: FormState,
    pub drag: Option<(usize, usize)>,
    pub drag_over: Option<Hit>,
}

#[derive(Debug, Default)]
pub struct PluginsView {
    pub sel: usize,
    pub offset: usize,
    pub filter: String,
    pub filtering: bool,
}

#[derive(Debug, Clone)]
pub enum PluginRow {
    Header(String),
    Item(String),
}

/// Comando externo que se ejecuta con la interfaz suspendida.
#[derive(Debug, Clone)]
pub struct ExecRequest {
    pub program: String,
    pub args: Vec<String>,
    pub rescan: bool,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub kind: NoteKind,
    pub until: Instant,
}

pub struct App {
    pub store: Store,
    pub prefs: Prefs,
    pub pal: Palette,
    pub section: Section,
    pub focus: Focus,
    /// Botón resaltado cuando el foco está en la fila de botones.
    pub button: usize,
    pub advanced: bool,
    pub forms: [FormState; 6],
    pub widgets: WidgetsView,
    pub plugins: PluginsView,
    pub popup: Option<Popup>,
    pub toast: Option<Toast>,
    pub hits: Vec<(Rect, Hit)>,
    pub quit: bool,
    pub exec: Option<ExecRequest>,
    pub shell_running: bool,
    last_check: Instant,
    last_click: Option<(Instant, Hit)>,
}

impl App {
    pub fn new(store: Store, prefs: Prefs) -> App {
        let pal = Palette::load(&store.paths.theme_colors());
        let shell_running = !store.paths.sandbox && ipc::shell_running();
        App {
            advanced: prefs.advanced,
            store,
            prefs,
            pal,
            section: Section::Bar,
            focus: Focus::Content,
            button: 0,
            forms: Default::default(),
            widgets: WidgetsView::default(),
            plugins: PluginsView::default(),
            popup: None,
            toast: None,
            hits: vec![],
            quit: false,
            exec: None,
            shell_running,
            last_check: Instant::now(),
            last_click: None,
        }
    }

    /// Las preferencias no se guardan en modo sandbox (pruebas).
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

    /// Tareas periódicas: caducidad de avisos y recarga si los archivos
    /// cambiaron por fuera (p. ej. al arrastrar widgets en la propia barra).
    pub fn tick(&mut self) {
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
        if self.popup.is_none() && !self.store.dirty() && !self.store.external_changes().is_empty()
        {
            self.store.reload();
            self.clamp_views();
            self.toast(t("msg.reloaded_external"), NoteKind::Info);
        }
    }

    pub fn clamp_views(&mut self) {
        for s in 0..3 {
            let n = sj::section(&self.store.json, s).len();
            self.widgets.row[s] = self.widgets.row[s].min(n.saturating_sub(1));
        }
        if let Some((s, i)) = self.widgets.editing
            && i >= sj::section(&self.store.json, s).len()
        {
            self.widgets.editing = None;
        }
    }

    // ------------------------------------------------------------ filas

    pub fn rows(&self) -> Vec<Row> {
        match self.section {
            Section::Bar => self.bar_rows(),
            Section::Widgets => match self.widgets.editing {
                Some((s, i)) => self.widget_rows(s, i),
                None => vec![],
            },
            Section::Plugins => vec![],
            Section::Idle => self.idle_rows(),
            Section::Appearance => self.appearance_rows(),
            Section::Changes => self.changes_rows(),
        }
    }

    fn form_state(&mut self) -> &mut FormState {
        if self.section == Section::Widgets {
            &mut self.widgets.form
        } else {
            &mut self.forms[self.section.index()]
        }
    }

    fn push_fields(&self, rows: &mut Vec<Row>, fields: Vec<FieldDef>, bind: impl Fn(&str) -> Bind) {
        for f in fields {
            if f.advanced && !self.advanced {
                continue;
            }
            let b = bind(&f.key);
            rows.push(Row::Field(FieldRow::new(f, b, &self.store)));
        }
    }

    fn bar_rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::Header(t("bar.header"))];
        if let Some(e) = &self.store.json_error {
            rows.push(Row::Note(
                tf("msg.json_invalid", &[("err", e)]),
                NoteKind::Warn,
            ));
        }
        if !self.store.json_from_user {
            rows.push(Row::Note(t("msg.using_defaults"), NoteKind::Info));
        }
        let fields = curated::bar_fields(&self.store.json, &self.store.catalog);
        self.push_fields(&mut rows, fields, |k| Bind::Json(vec![key("bar"), key(k)]));
        let anchor = sj::get(&self.store.json, &[key("bar"), key("centerAnchor")])
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !anchor.is_empty()
            && !sj::section(&self.store.json, 1)
                .iter()
                .any(|e| sj::entry_id(e) == anchor)
        {
            rows.push(Row::Note(t("bar.anchor_missing"), NoteKind::Warn));
        }
        rows.push(Row::Note(t("bar.tip"), NoteKind::Info));
        rows
    }

    fn widget_rows(&self, s: usize, i: usize) -> Vec<Row> {
        let Some(entry) = sj::section(&self.store.json, s).get(i).cloned() else {
            return vec![];
        };
        let id = sj::entry_id(&entry);
        let plugin = self.store.catalog.get(&id);
        let mut rows = vec![Row::Header(curated::widget_name(
            &id,
            Some(&entry),
            &self.store.catalog,
        ))];
        if let Some(p) = plugin {
            let d = curated::widget_description(p);
            if !d.is_empty() {
                rows.push(Row::Note(d, NoteKind::Info));
            }
        } else if entry.get("type").is_none() {
            rows.push(Row::Note(t("w.unknown"), NoteKind::Warn));
        }
        let fields = curated::widget_fields(&id, &entry, plugin);
        let field_keys: Vec<String> = fields.iter().map(|f| f.key.clone()).collect();
        let visible = fields
            .iter()
            .filter(|f| self.advanced || !f.advanced)
            .count();
        let base = sj::entry_path(s, i);
        self.push_fields(&mut rows, fields, |k| {
            Bind::Json([base.clone(), vec![key(k)]].concat())
        });
        if visible == 0 {
            rows.push(Row::Note(
                if field_keys.is_empty() {
                    t("w.no_settings")
                } else {
                    t("w.only_advanced")
                },
                NoteKind::Info,
            ));
        }
        if entry.get("type").and_then(Value::as_str) == Some("qml") {
            let path = self.qml_path(&id, &entry);
            rows.push(Row::Note(
                tf("w.qml_path", &[("path", &path.display().to_string())]),
                NoteKind::Info,
            ));
            if !path.exists() {
                rows.push(Row::Action(
                    t("w.qml_create"),
                    Act::CreateQmlTemplate(id.clone()),
                ));
            }
        }
        if self.advanced {
            rows.push(Row::Header(t("w.raw_header")));
            for (k, _) in sj::entry_settings(&entry) {
                if field_keys.contains(&k) {
                    continue;
                }
                let def = FieldDef::new(&k, k.clone(), Kind::Raw).desc(t("w.raw_desc"));
                let b = Bind::Json([base.clone(), vec![key(&k)]].concat());
                rows.push(Row::Field(FieldRow::new(def, b, &self.store)));
            }
            rows.push(Row::Action(
                t("w.raw_add"),
                Act::AddRawKey(Bind::Json(base)),
            ));
        }
        rows
    }

    fn qml_path(&self, id: &str, entry: &Value) -> PathBuf {
        match entry.get("source").and_then(Value::as_str) {
            Some(src) if !src.is_empty() => {
                src.strip_prefix("~/").map_or(PathBuf::from(src), |rest| {
                    dirs::home_dir().unwrap_or_default().join(rest)
                })
            }
            _ => self.store.paths.bar_modules().join(format!("{id}.qml")),
        }
    }

    fn idle_rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::Header(t("idle.header"))];
        self.push_fields(&mut rows, curated::idle_fields(), |k| {
            Bind::Json(vec![key("idle"), key(k)])
        });
        let get = |k: &str, d: i64| {
            sj::get(&self.store.json, &[key("idle"), key(k)])
                .and_then(Value::as_i64)
                .unwrap_or(d)
        };
        let (saver, lock) = (get("screensaver", 150), get("lock", 300));
        rows.push(Row::Note(
            tf(
                "idle.summary",
                &[
                    ("saver", &curated::human_seconds(saver)),
                    ("lock", &curated::human_seconds(lock)),
                ],
            ),
            NoteKind::Info,
        ));
        if lock < saver {
            rows.push(Row::Note(t("idle.warn_order"), NoteKind::Warn));
        }
        rows.push(Row::Note(t("idle.tip"), NoteKind::Info));
        rows
    }

    fn appearance_rows(&self) -> Vec<Row> {
        let theme = &self.store.theme;
        let mut rows = vec![];
        if let Some(e) = &self.store.toml_error {
            rows.push(Row::Note(
                tf("msg.toml_invalid", &[("err", e)]),
                NoteKind::Warn,
            ));
        }
        if theme.keys.is_empty() {
            rows.push(Row::Note(t("ap.no_theme"), NoteKind::Warn));
        }
        let bind = |s: &str, k: &str| Bind::Toml(s.to_string(), k.to_string());
        if !self.advanced {
            rows.push(Row::Header(t("ap.header")));
            for (s, k) in st::SIMPLE_KEYS {
                let mut f = st::field_for(theme, s, k);
                f.label = t(&format!("ap.{s}.{k}"));
                f.desc = t(&format!("ap.{s}.{k}.desc"));
                rows.push(Row::Field(FieldRow::new(f, bind(s, k), &self.store)));
            }
            rows.push(Row::Note(t("ap.tip"), NoteKind::Info));
            return rows;
        }
        rows.push(Row::Note(t("ap.tip_advanced"), NoteKind::Info));
        for section in theme.sections() {
            rows.push(Row::Header(format!("[{section}]")));
            for tk in theme.keys.iter().filter(|k| k.section == section) {
                let f = st::field_for(theme, &section, &tk.key);
                rows.push(Row::Field(FieldRow::new(
                    f,
                    bind(&section, &tk.key),
                    &self.store,
                )));
            }
        }
        // Claves del usuario que el tema no documenta.
        let mut extra = vec![];
        for (s, item) in self.store.toml.iter() {
            if let Some(tbl) = item.as_table_like() {
                for (k, _) in tbl.iter() {
                    if theme.get(s, k).is_none() {
                        extra.push((s.to_string(), k.to_string()));
                    }
                }
            }
        }
        if !extra.is_empty() {
            rows.push(Row::Header(t("ap.other")));
            for (s, k) in extra {
                let mut f = st::field_for(theme, &s, &k);
                f.label = format!("{s}.{k}");
                rows.push(Row::Field(FieldRow::new(f, bind(&s, &k), &self.store)));
            }
        }
        rows
    }

    fn changes_rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::Header(t("ch.header"))];
        let changes = self.store.changes();
        if changes.is_empty() && self.store.ops.is_empty() {
            rows.push(Row::Note(t("ch.none"), NoteKind::Info));
        } else {
            for c in changes {
                let old = c.old.unwrap_or_else(|| "∅".into());
                let new = c.new.unwrap_or_else(|| "∅".into());
                rows.push(Row::Note(
                    format!("{}  {}:  {old}  →  {new}", c.file, c.path),
                    NoteKind::Info,
                ));
            }
            for op in &self.store.ops {
                let (verb, id) = match op {
                    crate::store::PluginOp::Enable(id) => (t("ch.enable"), id),
                    crate::store::PluginOp::Disable(id) => (t("ch.disable"), id),
                };
                rows.push(Row::Note(format!("plugin  {verb}: {id}"), NoteKind::Info));
            }
            rows.push(Row::Note(t("ch.use_buttons"), NoteKind::Info));
        }
        rows.push(Row::Note(
            tf(
                "ch.backups",
                &[("dir", &self.store.paths.backup_dir.display().to_string())],
            ),
            NoteKind::Info,
        ));
        if self.advanced {
            rows.push(Row::Header(t("ch.tools")));
            rows.push(Row::Action(
                t("ch.edit_json"),
                Act::OpenEditor(self.store.paths.shell_json()),
            ));
            rows.push(Row::Action(
                t("ch.edit_toml"),
                Act::OpenEditor(self.store.paths.shell_toml()),
            ));
            if !self.store.paths.sandbox {
                rows.push(Row::Action(t("ch.restart"), Act::RestartShell));
            }
        }
        rows
    }

    pub fn plugin_rows(&self) -> Vec<PluginRow> {
        let f = fold(&self.plugins.filter);
        type Group = (&'static str, fn(&Plugin) -> bool);
        let groups: [Group; 4] = [
            ("pl.g.widgets", |p| p.is_bar_widget()),
            ("pl.g.services", |p| {
                !p.is_bar_widget() && p.has_kind("service")
            }),
            ("pl.g.panels", |p| {
                !p.is_bar_widget() && !p.has_kind("service") && !p.is_bar_option()
            }),
            ("pl.g.bars", |p| p.is_bar_option()),
        ];
        let mut rows = vec![];
        for (title, pred) in groups {
            let mut items: Vec<&Plugin> = self
                .store
                .catalog
                .plugins
                .iter()
                .filter(|p| pred(p))
                .filter(|p| {
                    f.is_empty()
                        || fold(&p.id).contains(&f)
                        || fold(&p.display_name()).contains(&f)
                        || fold(&curated::widget_name(&p.id, None, &self.store.catalog))
                            .contains(&f)
                        || fold(&curated::widget_description(p)).contains(&f)
                })
                .collect();
            if items.is_empty() {
                continue;
            }
            items.sort_by_cached_key(|p| {
                curated::widget_name(&p.id, None, &self.store.catalog).to_lowercase()
            });
            rows.push(PluginRow::Header(t(title)));
            for p in items {
                rows.push(PluginRow::Item(p.id.clone()));
            }
        }
        rows
    }

    pub fn selected_plugin(&self) -> Option<Plugin> {
        match self.plugin_rows().get(self.plugins.sel) {
            Some(PluginRow::Item(id)) => self.store.catalog.get(id).cloned(),
            _ => None,
        }
    }

    // ------------------------------------------------------------ teclado

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.request_quit();
            return;
        }
        if let Some(mut popup) = self.popup.take() {
            let outcome = popup.on_key(key);
            self.popup = Some(popup);
            self.handle_outcome(outcome);
            return;
        }
        if self.section == Section::Plugins && self.plugins.filtering {
            self.plugin_filter_key(key);
            return;
        }
        // Teclas globales.
        match key.code {
            KeyCode::Char('q') => return self.request_quit(),
            KeyCode::Char('?') => {
                self.popup = Some(Popup::Help { scroll: 0 });
                return;
            }
            KeyCode::Char('a') => return self.request_apply(),
            KeyCode::Char('c') => return self.request_discard(),
            KeyCode::Char('R') => return self.request_restore(),
            KeyCode::Char('m') => {
                self.advanced = !self.advanced;
                self.prefs.advanced = self.advanced;
                self.save_prefs();
                self.toast(
                    if self.advanced {
                        t("mode.advanced_on")
                    } else {
                        t("mode.simple_on")
                    },
                    NoteKind::Info,
                );
                return;
            }
            KeyCode::Char('i') => {
                let l = i18n::lang().toggle();
                i18n::set_lang(l);
                self.prefs.lang = l.code().into();
                self.save_prefs();
                return;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Content,
                    Focus::Content => Focus::Buttons,
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
            KeyCode::Char(c @ '1'..='6') => {
                self.go_section(Section::ALL[c as usize - '1' as usize]);
                return;
            }
            _ => {}
        }
        match self.focus {
            Focus::Sidebar => self.sidebar_key(key),
            Focus::Buttons => self.buttons_key(key),
            Focus::Content => match self.section {
                Section::Widgets if self.widgets.editing.is_none() => self.layout_key(key),
                Section::Plugins => self.plugins_key(key),
                _ => self.form_key(key),
            },
        }
    }

    // ------------------------------------------------------------ botones

    pub fn button_enabled(&self, b: Button) -> bool {
        match b {
            Button::Apply | Button::Cancel => self.store.dirty(),
            Button::Restore => true,
        }
    }

    fn buttons_key(&mut self, key: KeyEvent) {
        let n = Button::ALL.len();
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.button = (self.button + n - 1) % n,
            KeyCode::Right | KeyCode::Char('l') => self.button = (self.button + 1) % n,
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

    /// Qué restaura el botón "Restaurar" en la pantalla actual.
    pub fn restore_scope(&self) -> (Scope, &'static str) {
        match self.section {
            Section::Bar => (Scope::Bar, "restore.bar"),
            Section::Widgets => match self.widgets.editing {
                Some((s, i)) => (Scope::Widget(s, i), "restore.widget"),
                None => (Scope::Layout, "restore.layout"),
            },
            Section::Plugins => (Scope::Plugins, "restore.plugins"),
            Section::Idle => (Scope::Idle, "restore.idle"),
            Section::Appearance if self.advanced => {
                (Scope::Appearance { all: true }, "restore.appearance_all")
            }
            Section::Appearance => (Scope::Appearance { all: false }, "restore.appearance"),
            Section::Changes => (Scope::All, "restore.all"),
        }
    }

    fn request_restore(&mut self) {
        let (scope, k) = self.restore_scope();
        self.popup = Some(Popup::Confirm {
            title: t(k),
            lines: vec![t(&format!("{k}.desc")), String::new(), t("restore.note")],
            action: Confirm::Restore(scope),
        });
    }

    fn go_section(&mut self, s: Section) {
        self.section = s;
        self.focus = Focus::Content;
    }

    fn sidebar_key(&mut self, key: KeyEvent) {
        let i = self.section.index();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.section = Section::ALL[i.saturating_sub(1)],
            KeyCode::Down | KeyCode::Char('j') => self.section = Section::ALL[(i + 1).min(5)],
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
                self.focus = Focus::Content
            }
            KeyCode::Esc => self.request_quit(),
            _ => {}
        }
    }

    fn form_key(&mut self, key: KeyEvent) {
        let rows = self.rows();
        self.form_state().clamp(&rows);
        let sel = self.form_state().sel;
        match key.code {
            KeyCode::Esc => {
                if self.section == Section::Widgets {
                    self.widgets.editing = None;
                } else {
                    self.focus = Focus::Sidebar;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.form_state().step(&rows, false),
            KeyCode::Down | KeyCode::Char('j') => self.form_state().step(&rows, true),
            KeyCode::PageUp => (0..8).for_each(|_| self.form_state().step(&rows, false)),
            KeyCode::PageDown => (0..8).for_each(|_| self.form_state().step(&rows, true)),
            KeyCode::Home | KeyCode::Char('g') => self.form_state().first(&rows),
            KeyCode::End | KeyCode::Char('G') => self.form_state().last(&rows),
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {
                let forward = matches!(key.code, KeyCode::Right | KeyCode::Char('l'));
                if let Some(Row::Field(f)) = rows.get(sel) {
                    form::nudge(&mut self.store, f, forward);
                } else if !forward && self.section != Section::Widgets {
                    self.focus = Focus::Sidebar;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(row) = rows.get(sel).cloned() {
                    self.activate(row);
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

    fn activate(&mut self, row: Row) {
        match row {
            Row::Field(f) => self.activate_field(f),
            Row::Action(_, act) => self.run_act(act),
            _ => {}
        }
    }

    fn activate_field(&mut self, f: FieldRow) {
        let def = f.def.clone();
        match &def.kind {
            Kind::Bool => form::nudge(&mut self.store, &f, true),
            Kind::Enum(opts) => {
                self.popup = Some(Popup::Picker(Picker {
                    title: def.label.clone(),
                    items: opts.iter().map(PickItem::from_opt).collect(),
                    sel: opts
                        .iter()
                        .position(|o| Some(&o.value) == f.value.as_ref().or(def.default.as_ref()))
                        .unwrap_or(0),
                    filter: String::new(),
                    current: Some(f.value.clone().unwrap_or(Value::Null)),
                    target: PickTarget::Field {
                        bind: f.bind.clone(),
                        def: Box::new(def.clone()),
                    },
                }));
            }
            Kind::Multi(opts) => {
                let cur: Vec<Value> = f
                    .effective()
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                self.popup = Some(Popup::Checklist(Checklist {
                    title: def.label.clone(),
                    checked: opts.iter().map(|o| cur.contains(&o.value)).collect(),
                    opts: opts.clone(),
                    sel: 0,
                    bind: f.bind.clone(),
                }));
            }
            _ if !def.presets.is_empty() => {
                let mut items: Vec<PickItem> = def.presets.iter().map(PickItem::from_opt).collect();
                items.push(PickItem {
                    label: t("pick.custom"),
                    detail: t("pick.custom.desc"),
                    group: String::new(),
                    value: json!(PICK_CUSTOM),
                    enabled: true,
                });
                let cur = f.effective().cloned();
                self.popup = Some(Popup::Picker(Picker {
                    title: def.label.clone(),
                    sel: def
                        .presets
                        .iter()
                        .position(|o| Some(&o.value) == cur.as_ref())
                        .unwrap_or(items.len() - 1),
                    items,
                    filter: String::new(),
                    current: cur,
                    target: PickTarget::Field {
                        bind: f.bind.clone(),
                        def: Box::new(def.clone()),
                    },
                }));
            }
            _ => self.open_field_input(&f),
        }
    }

    fn open_field_input(&mut self, f: &FieldRow) {
        let def = &f.def;
        let mut hint = def.desc.clone();
        if matches!(def.kind, Kind::Text | Kind::Raw) {
            hint = format!(
                "{hint}\n{}",
                t(if def.kind == Kind::Raw {
                    "input.raw_hint"
                } else {
                    "input.text_hint"
                })
            );
        }
        if def.date_format {
            hint = format!("{hint}\n{}", t("input.date_hint"));
        }
        self.popup = Some(Popup::Input(Input::new(
            def.label.clone(),
            hint.trim().to_string(),
            &form::input_text(f),
            InputTarget::Field {
                bind: f.bind.clone(),
                def: Box::new(def.clone()),
            },
        )));
    }

    fn run_act(&mut self, act: Act) {
        match act {
            Act::AddRawKey(entry) => {
                self.popup = Some(Popup::Input(Input::new(
                    t("w.raw_add"),
                    t("input.key_hint"),
                    "",
                    InputTarget::NewRawKey { entry },
                )))
            }
            Act::CreateQmlTemplate(id) => {
                let entry = self
                    .widgets
                    .editing
                    .and_then(|(s, i)| sj::section(&self.store.json, s).get(i).cloned())
                    .unwrap_or(Value::Null);
                let path = self.qml_path(&id, &entry);
                let res = path
                    .parent()
                    .map(std::fs::create_dir_all)
                    .transpose()
                    .and_then(|_| std::fs::write(&path, qml_template(&id)));
                match res {
                    Ok(()) => self.toast(
                        tf("msg.file_created", &[("path", &path.display().to_string())]),
                        NoteKind::Info,
                    ),
                    Err(e) => self.toast(e.to_string(), NoteKind::Warn),
                }
            }
            Act::OpenEditor(path) => {
                if self.store.dirty() {
                    return self.message(t("msg.title"), vec![t("msg.pending_first")]);
                }
                let editor = std::env::var("EDITOR")
                    .ok()
                    .filter(|e| !e.is_empty())
                    .unwrap_or_else(|| "nvim".into());
                let mut parts = editor.split_whitespace().map(String::from);
                let program = parts.next().unwrap_or_else(|| "nvim".into());
                let mut args: Vec<String> = parts.collect();
                args.push(path.display().to_string());
                self.exec = Some(ExecRequest {
                    program,
                    args,
                    rescan: false,
                });
            }
            Act::RestartShell => {
                self.popup = Some(Popup::Confirm {
                    title: t("ch.restart"),
                    lines: vec![t("confirm.restart")],
                    action: Confirm::RestartShell,
                })
            }
        }
    }

    // --------------------------------------------- vista de widgets (layout)

    fn layout_key(&mut self, key: KeyEvent) {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let col = self.widgets.col;
        let n = sj::section(&self.store.json, col).len();
        let row = self.widgets.row[col].min(n.saturating_sub(1));
        match key.code {
            KeyCode::Esc => self.focus = Focus::Sidebar,
            // Mover el widget seleccionado.
            KeyCode::Char('K') => self.move_widget(0, -1),
            KeyCode::Char('J') => self.move_widget(0, 1),
            KeyCode::Char('H') => self.move_widget(-1, 0),
            KeyCode::Char('L') => self.move_widget(1, 0),
            KeyCode::Up if shift => self.move_widget(0, -1),
            KeyCode::Down if shift => self.move_widget(0, 1),
            KeyCode::Left if shift => self.move_widget(-1, 0),
            KeyCode::Right if shift => self.move_widget(1, 0),
            // Navegar.
            KeyCode::Up | KeyCode::Char('k') => self.widgets.row[col] = row.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.widgets.row[col] = (row + 1).min(n.saturating_sub(1))
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if col == 0 {
                    self.focus = Focus::Sidebar;
                } else {
                    self.widgets.col -= 1;
                }
            }
            KeyCode::Right | KeyCode::Char('l') => self.widgets.col = (col + 1).min(2),
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('e') => {
                if n > 0 {
                    self.widgets.editing = Some((col, row));
                    self.widgets.form = FormState::default();
                }
            }
            KeyCode::Char('n') | KeyCode::Insert => self.open_add_widget(),
            KeyCode::Char('d') | KeyCode::Delete | KeyCode::Char('x') if n > 0 => {
                sj::remove_entry(&mut self.store.json, col, row);
                self.clamp_views();
            }
            _ => {}
        }
    }

    /// Mueve el widget seleccionado: `dx` entre secciones, `dy` dentro.
    fn move_widget(&mut self, dx: i32, dy: i32) {
        let col = self.widgets.col;
        let n = sj::section(&self.store.json, col).len();
        if n == 0 {
            return;
        }
        let row = self.widgets.row[col].min(n - 1);
        let (to_s, to_i) = if dx != 0 {
            let s = (col as i32 + dx).clamp(0, 2) as usize;
            if s == col {
                return;
            }
            // Al pasar a la derecha entra por el principio; a la izquierda, por el final.
            let i = if dx > 0 {
                0
            } else {
                sj::section(&self.store.json, s).len()
            };
            (s, i)
        } else {
            let i = row as i32 + dy;
            if i < 0 || i >= n as i32 {
                return;
            }
            (col, i as usize)
        };
        if let Some((s, i)) = sj::move_entry(&mut self.store.json, (col, row), to_s, to_i) {
            self.widgets.col = s;
            self.widgets.row[s] = i;
        }
    }

    fn open_add_widget(&mut self) {
        let col = self.widgets.col;
        let n = sj::section(&self.store.json, col).len();
        let at = (
            col,
            if n == 0 {
                0
            } else {
                self.widgets.row[col].min(n - 1) + 1
            },
        );
        let mut items: Vec<PickItem> = self
            .store
            .catalog
            .bar_widgets()
            .map(|p| {
                let present = !sj::find_in_bar(&self.store.json, &p.id).is_empty();
                let multiple = p.bar_widget.as_ref().is_some_and(|b| b.allow_multiple);
                let cat = p
                    .bar_widget
                    .as_ref()
                    .map(|b| curated::category_name(&b.category))
                    .unwrap_or_default();
                let mut detail = curated::widget_description(p);
                if present && !multiple {
                    detail = format!("{} · {detail}", t("add.already"));
                }
                PickItem {
                    label: curated::widget_name(&p.id, None, &self.store.catalog),
                    detail,
                    group: cat,
                    value: json!(p.id),
                    enabled: !present || multiple,
                }
            })
            .collect();
        items.sort_by(|a, b| {
            (a.group.to_lowercase(), a.label.to_lowercase())
                .cmp(&(b.group.to_lowercase(), b.label.to_lowercase()))
        });
        if self.advanced {
            let g = t("add.custom_group");
            items.push(PickItem {
                label: t("add.command"),
                detail: t("add.command.desc"),
                group: g.clone(),
                value: json!(PICK_CMD),
                enabled: true,
            });
            items.push(PickItem {
                label: t("add.qml"),
                detail: t("add.qml.desc"),
                group: g,
                value: json!(PICK_QML),
                enabled: true,
            });
        }
        let sel = items.iter().position(|i| i.enabled).unwrap_or(0);
        self.popup = Some(Popup::Picker(Picker {
            title: tf(
                "add.title",
                &[("section", &t(&format!("col.{}", sj::SECTIONS[col])))],
            ),
            items,
            sel,
            filter: String::new(),
            current: None,
            target: PickTarget::AddWidget { at },
        }));
    }

    // ------------------------------------------------------------ plugins

    fn plugin_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.plugins.filter.clear();
                self.plugins.filtering = false;
            }
            KeyCode::Enter | KeyCode::Down => self.plugins.filtering = false,
            KeyCode::Backspace => {
                self.plugins.filter.pop();
            }
            KeyCode::Char(c) => self.plugins.filter.push(c),
            _ => {}
        }
        self.plugins.sel = 0;
        self.clamp_plugin_sel(true);
    }

    pub fn clamp_plugin_sel(&mut self, down: bool) {
        let rows = self.plugin_rows();
        if rows.is_empty() {
            self.plugins.sel = 0;
            return;
        }
        self.plugins.sel = self.plugins.sel.min(rows.len() - 1);
        let ok = |i: usize| matches!(rows.get(i), Some(PluginRow::Item(_)));
        if ok(self.plugins.sel) {
            return;
        }
        let fwd = (self.plugins.sel..rows.len()).find(|&i| ok(i));
        let back = (0..self.plugins.sel).rev().find(|&i| ok(i));
        self.plugins.sel = if down { fwd.or(back) } else { back.or(fwd) }.unwrap_or(0);
    }

    fn plugins_key(&mut self, key: KeyEvent) {
        let rows = self.plugin_rows();
        let sel = self.plugins.sel;
        let item = |i: usize| matches!(rows.get(i), Some(PluginRow::Item(_)));
        match key.code {
            KeyCode::Esc => {
                if self.plugins.filter.is_empty() {
                    self.focus = Focus::Sidebar;
                } else {
                    self.plugins.filter.clear();
                    self.clamp_plugin_sel(true);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(i) = (0..sel).rev().find(|&i| item(i)) {
                    self.plugins.sel = i;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(i) = (sel + 1..rows.len()).find(|&i| item(i)) {
                    self.plugins.sel = i;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => self.focus = Focus::Sidebar,
            KeyCode::Char('/') => self.plugins.filtering = true,
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(p) = self.selected_plugin() {
                    let infrastructure = p.first_party && !p.is_bar_widget() && !p.is_bar_option();
                    if infrastructure && self.store.plugin_enabled(&p) {
                        let name = curated::widget_name(&p.id, None, &self.store.catalog);
                        self.popup = Some(Popup::Confirm {
                            title: tf("confirm.disable.title", &[("name", &name)]),
                            lines: vec![t("confirm.disable")],
                            action: Confirm::TogglePlugin(p.id.clone()),
                        });
                    } else {
                        self.toggle_plugin(&p.id);
                    }
                }
            }
            KeyCode::Char('n') => {
                if self.lifecycle_blocked() {
                    return;
                }
                self.popup = Some(Popup::Input(Input::new(
                    t("pl.add"),
                    t("pl.add.hint"),
                    "",
                    InputTarget::GitUrl,
                )));
            }
            KeyCode::Char('p') => {
                let Some(p) = self.selected_plugin() else {
                    return;
                };
                if !p.first_party {
                    return self.toast(t("pl.clone.only_builtin"), NoteKind::Warn);
                }
                if let Some(c) = self.store.catalog.clone_of(&p.id) {
                    return self.toast(tf("pl.clone.exists", &[("id", &c.id)]), NoteKind::Warn);
                }
                if self.lifecycle_blocked() {
                    return;
                }
                self.exec_omarchy(&["plugin", "clone", &p.id]);
            }
            KeyCode::Char('u') => {
                let Some(p) = self.selected_plugin() else {
                    return;
                };
                if !p.is_git {
                    return self.toast(t("pl.update.not_git"), NoteKind::Warn);
                }
                if self.lifecycle_blocked() {
                    return;
                }
                self.exec_omarchy(&["plugin", "update", &p.id]);
            }
            KeyCode::Char('U') => {
                if self.lifecycle_blocked() {
                    return;
                }
                self.exec_omarchy(&["plugin", "update"]);
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                let Some(p) = self.selected_plugin() else {
                    return;
                };
                if p.first_party {
                    return self.toast(t("pl.remove.builtin"), NoteKind::Warn);
                }
                if self.lifecycle_blocked() {
                    return;
                }
                self.exec_omarchy(&["plugin", "remove", &p.id]);
            }
            KeyCode::Char('e') if self.advanced => {
                let Some(p) = self.selected_plugin() else {
                    return;
                };
                if p.first_party {
                    return self.toast(t("pl.edit.builtin"), NoteKind::Warn);
                }
                self.run_act(Act::OpenEditor(p.source_dir.clone()));
            }
            _ => {}
        }
    }

    fn toggle_plugin(&mut self, id: &str) {
        if let Err(k) = self.store.toggle_plugin(id) {
            self.toast(t(k), NoteKind::Warn);
        }
    }

    /// Las operaciones de ciclo de vida reescriben `shell.json` por su cuenta:
    /// exigen no tener cambios pendientes y el shell en marcha.
    fn lifecycle_blocked(&mut self) -> bool {
        if self.store.dirty() {
            self.message(t("msg.title"), vec![t("msg.pending_first")]);
            return true;
        }
        if self.store.paths.sandbox {
            self.toast(t("msg.sandbox_no_cli"), NoteKind::Warn);
            return true;
        }
        false
    }

    fn exec_omarchy(&mut self, args: &[&str]) {
        self.exec = Some(ExecRequest {
            program: "omarchy".into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            rescan: true,
        });
    }

    /// Tras volver de un comando externo.
    pub fn after_exec(&mut self, req: &ExecRequest, result: Result<bool, String>) {
        if req.rescan && self.shell_running {
            let _ = ipc::rescan_plugins();
        }
        self.store.reload();
        self.clamp_views();
        self.clamp_plugin_sel(true);
        match result {
            Ok(true) => self.toast(t("msg.cmd_ok"), NoteKind::Info),
            Ok(false) => self.toast(t("msg.cmd_failed"), NoteKind::Warn),
            Err(e) => self.toast(e, NoteKind::Warn),
        }
    }

    // ------------------------------------------------- aplicar / salir

    fn request_apply(&mut self) {
        if !self.store.dirty() {
            return self.toast(t("msg.nothing"), NoteKind::Info);
        }
        let mut lines: Vec<String> = self
            .store
            .changes()
            .into_iter()
            .map(|c| {
                format!(
                    "{}  {}  →  {}",
                    c.path,
                    c.old.unwrap_or_else(|| "∅".into()),
                    c.new.unwrap_or_else(|| "∅".into())
                )
            })
            .collect();
        for op in &self.store.ops {
            lines.push(format!("plugin {op:?}"));
        }
        self.popup = Some(Popup::Confirm {
            title: tf("confirm.apply", &[("n", &lines.len().to_string())]),
            lines,
            action: Confirm::Apply,
        });
    }

    fn request_discard(&mut self) {
        if !self.store.dirty() {
            return self.toast(t("msg.nothing"), NoteKind::Info);
        }
        self.popup = Some(Popup::Confirm {
            title: t("confirm.discard.title"),
            lines: vec![t("confirm.discard")],
            action: Confirm::Discard,
        });
    }

    fn request_quit(&mut self) {
        if self.store.dirty() {
            self.popup = Some(Popup::Confirm {
                title: t("confirm.quit.title"),
                lines: vec![t("confirm.quit")],
                action: Confirm::Quit,
            });
        } else {
            self.quit = true;
        }
    }

    fn do_apply(&mut self, force: bool) {
        if !force {
            let ext = self.store.external_changes();
            if !ext.is_empty() {
                self.popup = Some(Popup::Conflict { files: ext });
                return;
            }
        }
        match self.store.apply() {
            Ok(report) => {
                self.clamp_views();
                if !report.errors.is_empty() {
                    let mut lines = vec![t("msg.applied_with_errors")];
                    lines.extend(report.errors.iter().map(|e| {
                        if e == "omarchy-shell" {
                            t("msg.shell_not_running_ops")
                        } else {
                            e.clone()
                        }
                    }));
                    self.message(t("msg.title"), lines);
                } else if self.store.paths.sandbox {
                    self.toast(t("msg.applied_sandbox"), NoteKind::Info);
                } else if report.reloaded {
                    self.toast(t("msg.applied"), NoteKind::Info);
                } else {
                    self.toast(t("msg.applied_no_shell"), NoteKind::Warn);
                }
            }
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
                        self.clamp_views();
                        self.toast(t("msg.discarded"), NoteKind::Info);
                    }
                    Confirm::Quit => self.quit = true,
                    Confirm::RestartShell => match ipc::run("omarchy", &["restart", "shell"]) {
                        Ok(_) => self.toast(t("msg.cmd_ok"), NoteKind::Info),
                        Err(e) => self.toast(e, NoteKind::Warn),
                    },
                    Confirm::Restore(scope) => {
                        self.store.restore(&scope);
                        self.clamp_views();
                        self.toast(
                            if self.store.dirty() {
                                t("msg.restored")
                            } else {
                                t("msg.restored_nothing")
                            },
                            NoteKind::Info,
                        );
                    }
                    Confirm::TogglePlugin(id) => self.toggle_plugin(&id),
                }
            }
            Outcome::ConflictOverwrite => {
                self.popup = None;
                self.do_apply(true);
            }
            Outcome::ConflictReload => {
                self.popup = None;
                self.store.reload();
                self.clamp_views();
                self.toast(t("msg.reloaded_external"), NoteKind::Info);
            }
            Outcome::Picked(target, value) => {
                self.popup = None;
                self.on_picked(target, value);
            }
            Outcome::Submitted(target, text) => self.on_submitted(target, text),
            Outcome::Checked(bind, values) => {
                self.popup = None;
                if values.is_empty() {
                    self.store.unset(&bind);
                } else {
                    self.store.set(&bind, Value::Array(values));
                }
            }
        }
    }

    fn on_picked(&mut self, target: PickTarget, value: Value) {
        match target {
            PickTarget::Field { bind, def } => {
                if value == json!(PICK_CUSTOM) {
                    let f = FieldRow::new(*def, bind, &self.store);
                    self.open_field_input(&f);
                } else {
                    self.store.set_field(&bind, value, def.default.as_ref());
                }
            }
            PickTarget::AddWidget { at } => {
                if value == json!(PICK_CMD) || value == json!(PICK_QML) {
                    let qml = value == json!(PICK_QML);
                    self.popup = Some(Popup::Input(Input::new(
                        t(if qml { "add.qml" } else { "add.command" }),
                        t("input.module_id_hint"),
                        "",
                        InputTarget::NewModule { qml, at },
                    )));
                    return;
                }
                let Some(id) = value.as_str() else { return };
                let (s, i) = self.store.add_widget(id, Some(at));
                self.widgets.col = s;
                self.widgets.row[s] = i;
            }
        }
    }

    fn input_error(&mut self, msg: String) {
        if let Some(Popup::Input(inp)) = &mut self.popup {
            inp.error = Some(msg);
        }
    }

    fn on_submitted(&mut self, target: InputTarget, text: String) {
        match target {
            InputTarget::Field { bind, def } => {
                if text.trim().is_empty() && def.kind != Kind::Bool {
                    self.store.unset(&bind);
                    self.popup = None;
                    return;
                }
                match schema::parse_input(&def.kind, &text) {
                    Ok(v) => {
                        self.store.set_field(&bind, v, def.default.as_ref());
                        self.popup = None;
                    }
                    Err(code) => self.input_error(form::input_error(&def.kind, &code)),
                }
            }
            InputTarget::NewRawKey { entry } => {
                let k = text.trim().to_string();
                if k.is_empty() || k == "id" || k.contains(char::is_whitespace) {
                    return self.input_error(t("err.key"));
                }
                self.popup = Some(Popup::Input(Input::new(
                    k.clone(),
                    t("input.raw_hint"),
                    "",
                    InputTarget::RawValue { entry, key: k },
                )));
            }
            InputTarget::RawValue { entry, key: k } => {
                let Bind::Json(mut path) = entry else { return };
                path.push(key(&k));
                let v = schema::parse_input(&Kind::Raw, &text).unwrap_or(Value::Null);
                self.store.set(&Bind::Json(path), v);
                self.popup = None;
            }
            InputTarget::GitUrl => {
                let url = text.trim().to_string();
                if url.is_empty() {
                    return self.input_error(t("err.url"));
                }
                self.popup = None;
                self.exec_omarchy(&["plugin", "add", &url]);
            }
            InputTarget::NewModule { qml, at } => {
                let id = text.trim().to_string();
                let valid = !id.is_empty()
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
                if !valid {
                    return self.input_error(t("err.module_id"));
                }
                if !sj::find_in_bar(&self.store.json, &id).is_empty()
                    || self.store.catalog.get(&id).is_some()
                {
                    return self.input_error(t("err.module_exists"));
                }
                let entry = if qml {
                    json!({ "id": id, "type": "qml" })
                } else {
                    json!({ "id": id, "type": "command", "exec": "", "interval": 5 })
                };
                let len = sj::section(&self.store.json, at.0).len();
                let i = at.1.min(len);
                sj::insert_entry(&mut self.store.json, at.0, i, entry);
                self.popup = None;
                self.widgets.col = at.0;
                self.widgets.row[at.0] = i;
                self.widgets.editing = Some((at.0, i));
                self.widgets.form = FormState::default();
            }
        }
    }

    // ------------------------------------------------------------ ratón

    fn hit_at(&self, x: u16, y: u16) -> Option<Hit> {
        let p = Position { x, y };
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(p))
            .map(|(_, h)| *h)
    }

    pub fn on_mouse(&mut self, m: MouseEvent) {
        let hit = self.hit_at(m.column, m.row);
        match m.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let code = if m.kind == MouseEventKind::ScrollUp {
                    KeyCode::Up
                } else {
                    KeyCode::Down
                };
                self.on_key(KeyEvent::new(code, KeyModifiers::NONE));
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let double = self.last_click.is_some_and(|(at, h)| {
                    Some(h) == hit && at.elapsed() < Duration::from_millis(400)
                });
                self.last_click = hit.map(|h| (Instant::now(), h));
                self.click(hit, double);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.widgets.drag.is_some() {
                    self.widgets.drag_over = hit;
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some(from) = self.widgets.drag.take() {
                    let over = self.widgets.drag_over.take();
                    let to = match over {
                        Some(Hit::Widget(s, i)) => Some((s, i)),
                        Some(Hit::Column(s)) => Some((s, sj::section(&self.store.json, s).len())),
                        _ => None,
                    };
                    if let Some(to) = to
                        && to != from
                        && let Some((s, i)) = sj::move_entry(&mut self.store.json, from, to.0, to.1)
                    {
                        self.widgets.col = s;
                        self.widgets.row[s] = i;
                    }
                }
            }
            _ => {}
        }
    }

    fn click(&mut self, hit: Option<Hit>, double: bool) {
        if self.popup.is_some() {
            if let Some(Hit::PopupItem(i)) = hit {
                let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
                match self.popup.as_mut() {
                    Some(Popup::Picker(p)) => {
                        let again = p.sel == i;
                        p.sel = i;
                        if again || double {
                            self.on_key(enter);
                        }
                    }
                    Some(Popup::Checklist(c)) => {
                        c.sel = i;
                        if let Some(x) = c.checked.get_mut(i) {
                            *x = !*x;
                        }
                    }
                    _ => {}
                }
            }
            return;
        }
        match hit {
            Some(Hit::Button(i)) => {
                self.focus = Focus::Buttons;
                self.button = i;
                let b = Button::ALL[i];
                if self.button_enabled(b) {
                    self.press(b);
                }
            }
            Some(Hit::Sidebar(i)) => {
                self.section = Section::ALL[i];
                self.focus = Focus::Content;
            }
            Some(Hit::Row(i)) => {
                self.focus = Focus::Content;
                let rows = self.rows();
                if !rows.get(i).is_some_and(Row::selectable) {
                    return;
                }
                let again = self.form_state().sel == i;
                self.form_state().sel = i;
                if again && let Some(row) = rows.get(i).cloned() {
                    self.activate(row);
                }
            }
            Some(Hit::Widget(s, i)) => {
                self.focus = Focus::Content;
                let again = self.widgets.col == s && self.widgets.row[s] == i;
                self.widgets.col = s;
                self.widgets.row[s] = i;
                if again && double {
                    self.widgets.editing = Some((s, i));
                    self.widgets.form = FormState::default();
                } else {
                    self.widgets.drag = Some((s, i));
                    self.widgets.drag_over = None;
                }
            }
            Some(Hit::Column(s)) => {
                self.focus = Focus::Content;
                self.widgets.col = s;
            }
            Some(Hit::Plugin(i)) => {
                self.focus = Focus::Content;
                let again = self.plugins.sel == i;
                self.plugins.sel = i;
                self.clamp_plugin_sel(true);
                if again && double {
                    self.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
            }
            _ => {}
        }
    }
}

fn qml_template(id: &str) -> String {
    format!(
        r#"// Módulo QML personalizado para la barra de Omarchy ({id}).
// Documentación: $OMARCHY_PATH/shell/plugins/bar/README.md
import QtQuick

Item {{
  property var bar
  property string moduleName
  property var settings

  implicitWidth: label.implicitWidth + 12
  implicitHeight: bar ? bar.barSize : 26

  Text {{
    id: label
    anchors.centerIn: parent
    text: "{id}"
    color: bar ? bar.foreground : "white"
    font.family: bar ? bar.fontFamily : "monospace"
    font.pixelSize: 12
  }}

  MouseArea {{
    anchors.fill: parent
    onClicked: if (bar) bar.run("notify-send '{id}'")
  }}
}}
"#
    )
}
