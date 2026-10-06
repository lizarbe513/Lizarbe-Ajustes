//! Las dos secciones que son listas: Atajos de teclado (con grabación de
//! combinaciones y asistente para añadir) e Inicio automático.

use lizarbe_core::form::NoteKind;
use lizarbe_core::popup::{Input, PickItem, PickTarget, Picker};
use lizarbe_core::schema::{FieldDef, Kind};
use serde_json::{Value, json};

use super::{Act, App, Confirm, EInput, EPick, FieldRow, MenuAct, MenuItem, Popup, RecTarget, Row};
use crate::apps::{self, App as DesktopApp};
use crate::autostart;
use crate::hyprfile::CustomBind;
use crate::i18n::{self, t, tf};
use crate::record::{self, Recorder, State};
use crate::store::{Bind, Change};

/// Orden de las categorías de atajos (por archivo de origen).
const ORIGINS: [&str; 8] = [
    "applications",
    "tiling",
    "utilities",
    "media",
    "clipboard",
    "voxtype",
    "user",
    "other",
];

/// Valor del selector de inicio para escribir una orden a mano.
const PICK_CMD: &str = "\u{0}cmd";

impl App {
    // ------------------------------------------------------------ filas

    pub(super) fn keybind_rows(&self) -> Vec<Row> {
        let mut rows = vec![];
        if !self.store.live {
            rows.push(Row::Note(t("note.binds_offline"), NoteKind::Warn));
            return rows;
        }
        if self.store.meca_active {
            rows.push(Row::Note(t("note.meca"), NoteKind::Warn));
        }
        rows.push(Row::Action(t("bind.add"), Act::AddBind));
        if self.bind_filter.is_empty() {
            rows.push(Row::Action(t("bind.search"), Act::SearchBinds));
        } else {
            rows.push(Row::Action(
                tf("bind.searching", &[("text", &self.bind_filter)]),
                Act::SearchBinds,
            ));
            rows.push(Row::Action(t("bind.clear_search"), Act::ClearSearch));
        }
        let filter = lizarbe_core::ui::fold(&self.bind_filter);
        let matches = |desc: &str, keys: &str| {
            filter.is_empty()
                || lizarbe_core::ui::fold(desc).contains(&filter)
                || lizarbe_core::ui::fold(keys).contains(&filter)
        };
        let found = |text: &Option<String>| {
            !filter.is_empty()
                && text
                    .as_deref()
                    .is_some_and(|a| lizarbe_core::ui::fold(a).contains(&filter))
        };

        let custom: Vec<(usize, CustomBind)> = self
            .store
            .custom_binds()
            .into_iter()
            .enumerate()
            .filter(|(_, c)| matches(&c.desc, &c.keys) || found(&Some(c.action.clone())))
            .collect();
        if !custom.is_empty() {
            rows.push(Row::Header(t("bc.custom")));
            for (i, c) in custom {
                let def = FieldDef::new(&format!("x:custom:{i}"), c.desc.clone(), Kind::Text)
                    .desc(t("bind.custom_desc"))
                    .default(json!(c.keys));
                rows.push(Row::Field(FieldRow::new(
                    def,
                    Bind(format!("x:custom:{i}")),
                    &self.store,
                )));
            }
        }
        for origin in ORIGINS {
            let entries: Vec<_> = self
                .store
                .binds
                .iter()
                .filter(|b| b.origin == origin)
                .filter(|b| {
                    let now = match self.store.values.get(&format!("x:bind:{}", b.keys)) {
                        Some(Value::String(k)) => k.clone(),
                        _ => b.keys.clone(),
                    };
                    matches(&b.desc, &b.keys) || matches(&b.desc, &now) || found(&b.action)
                })
                .collect();
            if entries.is_empty() {
                continue;
            }
            rows.push(Row::Header(t(&format!("bc.{origin}"))));
            for b in entries {
                let key = format!("x:bind:{}", b.keys);
                let mut desc = t(&format!("bc.{origin}.d"));
                if matches!(self.store.values.get(&key), Some(Value::Null)) {
                    desc = format!("{} · {desc}", t("bind.disabled"));
                } else if b.action.is_none() {
                    desc = format!("{desc} · {}", t("bind.fixed"));
                }
                let label = if b.desc.is_empty() {
                    b.keys.clone()
                } else {
                    b.desc.clone()
                };
                let def = FieldDef::new(&key, label, Kind::Text)
                    .desc(desc)
                    .default(json!(b.keys));
                rows.push(Row::Field(FieldRow::new(def, Bind(key), &self.store)));
            }
        }
        rows
    }

    pub(super) fn autostart_rows(&self) -> Vec<Row> {
        let mut rows = vec![
            Row::Note(t("note.autostart"), NoteKind::Info),
            Row::Action(t("as.add"), Act::AddAutostart),
        ];
        if self.store.autostart.is_empty() {
            rows.push(Row::Note(t("as.none"), NoteKind::Info));
            return rows;
        }
        rows.push(Row::Header(t("as.header")));
        let apps = self.apps_cached();
        for (i, e) in self.store.autostart.iter().enumerate() {
            let name = apps::name_for(&e.cmd, apps).unwrap_or_else(|| e.cmd.clone());
            let desc = if e.exec {
                tf("as.command", &[("cmd", &e.cmd)])
            } else {
                e.cmd.clone()
            };
            let def = FieldDef::new(&format!("as:{i}"), name, Kind::Bool)
                .desc(desc)
                .default(json!(e.enabled));
            rows.push(Row::Field(FieldRow::new(
                def,
                Bind(format!("as:{i}")),
                &self.store,
            )));
        }
        rows
    }

    fn apps_cached(&self) -> &[DesktopApp] {
        self.apps
            .get_or_init(|| apps::installed(i18n::lang().code()))
            .as_slice()
    }

    // ------------------------------------------------------------ acciones

    pub(super) fn run_act(&mut self, act: Act) {
        match act {
            Act::AddBind => self.start_record(RecTarget::NewCustom),
            Act::SearchBinds => {
                self.popup = Some(Popup::Input(Input::new(
                    t("bind.search"),
                    t("bind.search.hint"),
                    &self.bind_filter,
                    cp_input(EInput::BindFilter),
                )))
            }
            Act::ClearSearch => self.bind_filter.clear(),
            Act::AddAutostart => {
                let mut items: Vec<PickItem> = self
                    .apps_cached()
                    .iter()
                    .map(|a| PickItem {
                        label: a.name.clone(),
                        detail: a.exec.clone(),
                        group: t("as.group_apps"),
                        value: json!(a.exec),
                        enabled: true,
                    })
                    .collect();
                items.push(PickItem {
                    label: t("as.custom"),
                    detail: t("as.custom.d"),
                    group: t("as.group_other"),
                    value: json!(PICK_CMD),
                    enabled: true,
                });
                self.popup = Some(Popup::Picker(Picker {
                    title: t("as.add"),
                    items,
                    sel: 0,
                    filter: String::new(),
                    target: PickTarget::App(EPick::AutostartApp),
                    current: None,
                    anchor: None,
                }));
            }
        }
    }

    /// Entradas del menú de una fila de atajo o de programa de inicio.
    pub(super) fn list_menu_items(&self, i: usize, f: &FieldRow) -> Vec<MenuItem> {
        let item = |icon: &'static str, label: String, act: MenuAct, enabled: bool| MenuItem {
            icon,
            label,
            act,
            enabled,
            separator: false,
        };
        let key = f.bind.0.as_str();
        if let Some(n) = key.strip_prefix("x:custom:").and_then(|n| n.parse().ok()) {
            return vec![
                item("󰌌", t("bind.change"), MenuAct::BindRecord(i), true),
                item("󰆴", t("bind.remove"), MenuAct::CustomRemove(n), true),
            ];
        }
        if let Some(orig) = key.strip_prefix("x:bind:") {
            let movable = self
                .store
                .binds
                .iter()
                .any(|b| b.keys == orig && b.action.is_some());
            let disabled = matches!(f.value, Some(Value::Null));
            return vec![
                item("󰌌", t("bind.change"), MenuAct::BindRecord(i), movable),
                item("󰂭", t("bind.disable"), MenuAct::BindDisable(i), !disabled),
                item(
                    "󰑓",
                    t("bind.restore"),
                    MenuAct::ResetField(i),
                    f.value.is_some(),
                ),
            ];
        }
        if let Some(n) = key.strip_prefix("as:").and_then(|n| n.parse().ok()) {
            return vec![
                item(
                    if f.effective().and_then(Value::as_bool) == Some(true) {
                        "󰨙"
                    } else {
                        "󰔡"
                    },
                    t("menu.toggle"),
                    MenuAct::Activate(i),
                    true,
                ),
                item("󰆴", t("as.remove"), MenuAct::AutostartRemove(n), true),
            ];
        }
        vec![]
    }

    pub(super) fn is_list_row(key: &str) -> bool {
        key.starts_with("x:bind:") || key.starts_with("x:custom:")
    }

    pub(super) fn remove_autostart(&mut self, i: usize) {
        if i < self.store.autostart.len() {
            self.store.autostart.remove(i);
        }
    }

    // ------------------------------------------------------------ grabar

    pub(super) fn start_record(&mut self, target: RecTarget) {
        if !self.store.live {
            return self.toast(t("rec.offline"), NoteKind::Warn);
        }
        match Recorder::start() {
            Ok(rec) => {
                self.recording = Some((rec, target));
                self.popup = Some(Popup::Message {
                    title: t("rec.title"),
                    lines: vec![t("rec.hint"), String::new(), "…".into()],
                });
            }
            Err(e) => self.message(t("msg.error"), vec![e]),
        }
    }

    /// Sigue la grabación en curso (se llama en cada vuelta del bucle).
    pub(super) fn poll_record(&mut self) {
        let Some((rec, _)) = &self.recording else {
            return;
        };
        match rec.poll() {
            State::Waiting(keys) => {
                if let Some(Popup::Message { lines, .. }) = &mut self.popup {
                    lines[2] = if keys.is_empty() { "…".into() } else { keys };
                }
            }
            State::Cancelled => {
                self.recording = None;
                self.popup = None;
                self.toast(t("rec.cancelled"), NoteKind::Info);
            }
            State::Done(keys) => {
                let target = self.recording.take().map(|(_, t)| t);
                self.popup = None;
                if let Some(target) = target {
                    self.on_recorded(target, keys);
                }
            }
        }
    }

    /// Atajos que ya usan `keys`, sin contar el que se está cambiando.
    fn conflicts(&self, target: &RecTarget, keys: &str) -> Vec<String> {
        let own = match target {
            RecTarget::Move(orig) => self
                .store
                .values
                .get(&format!("x:bind:{orig}"))
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| orig.clone()),
            RecTarget::Custom(i) => self
                .store
                .custom_binds()
                .get(*i)
                .map(|c| c.keys.clone())
                .unwrap_or_default(),
            RecTarget::NewCustom => String::new(),
        };
        self.store
            .keys_in_use()
            .into_iter()
            .filter(|(k, _)| k == keys && *k != own)
            .map(|(_, desc)| desc)
            .collect()
    }

    fn on_recorded(&mut self, target: RecTarget, keys: String) {
        if !record::is_valid(&keys) {
            return self.toast(t("rec.only_mods"), NoteKind::Warn);
        }
        let others = self.conflicts(&target, &keys);
        if others.is_empty() {
            return self.assign(target, keys);
        }
        let mut lines = vec![tf("rec.conflict", &[("keys", &keys)])];
        lines.extend(others.iter().map(|d| format!("  • {d}")));
        lines.push(String::new());
        lines.push(t("rec.conflict.d"));
        self.popup = Some(Popup::Confirm {
            title: t("rec.conflict.title"),
            lines,
            action: Confirm::BindAssign { target, keys },
        });
    }

    /// Asigna `keys` y desactiva lo que las usaba.
    pub(super) fn assign(&mut self, target: RecTarget, keys: String) {
        let own_orig = match &target {
            RecTarget::Move(orig) => Some(orig.clone()),
            _ => None,
        };
        let entries: Vec<String> = self.store.binds.iter().map(|b| b.keys.clone()).collect();
        for orig in entries {
            if Some(&orig) == own_orig.as_ref() {
                continue;
            }
            let k = format!("x:bind:{orig}");
            let now = match self.store.values.get(&k) {
                Some(Value::String(n)) => Some(n.clone()),
                Some(_) => None,
                None => Some(orig.clone()),
            };
            if now.as_deref() == Some(keys.as_str()) {
                self.store.values.insert(k, Value::Null);
            }
        }
        let mut custom = self.store.custom_binds();
        let own_custom = match &target {
            RecTarget::Custom(i) => Some(*i),
            _ => None,
        };
        if let Some(c) = own_custom.and_then(|i| custom.get_mut(i)) {
            c.keys = keys.clone();
        }
        let mut idx = 0;
        custom.retain(|c| {
            let keep = Some(idx) == own_custom || c.keys != keys;
            idx += 1;
            keep
        });
        self.store.set_custom_binds(&custom);
        match target {
            RecTarget::Move(orig) => {
                let k = format!("x:bind:{orig}");
                if keys == orig {
                    self.store.values.remove(&k);
                } else {
                    self.store.values.insert(k, json!(keys));
                }
            }
            RecTarget::Custom(_) => {}
            RecTarget::NewCustom => {
                let items = [
                    ("app", "bind.kind.app", "bind.kind.app.d"),
                    ("cmd", "bind.kind.cmd", "bind.kind.cmd.d"),
                    ("web", "bind.kind.web", "bind.kind.web.d"),
                ]
                .iter()
                .map(|(v, l, d)| PickItem {
                    label: t(l),
                    detail: t(d),
                    group: String::new(),
                    value: json!(v),
                    enabled: true,
                })
                .collect();
                self.popup = Some(Popup::Picker(Picker {
                    title: tf("bind.kind.title", &[("keys", &keys)]),
                    items,
                    sel: 0,
                    filter: String::new(),
                    target: PickTarget::App(EPick::BindKind(keys)),
                    current: None,
                    anchor: None,
                }));
            }
        }
    }

    fn add_custom(&mut self, keys: String, desc: String, action: String) {
        let mut list = self.store.custom_binds();
        list.push(CustomBind { keys, desc, action });
        self.store.set_custom_binds(&list);
        self.bind_filter.clear();
        self.toast(t("bind.added"), NoteKind::Info);
    }

    pub(super) fn remove_custom(&mut self, i: usize) {
        let mut list = self.store.custom_binds();
        if i < list.len() {
            list.remove(i);
            self.store.set_custom_binds(&list);
        }
    }

    // ------------------------------------------------- ventanas propias

    pub(super) fn on_pick(&mut self, pick: EPick, value: Value) {
        let text = value.as_str().unwrap_or_default().to_string();
        match pick {
            EPick::BindKind(keys) => match text.as_str() {
                "app" => {
                    let items = self
                        .apps_cached()
                        .iter()
                        .map(|a| PickItem {
                            label: a.name.clone(),
                            detail: a.exec.clone(),
                            group: String::new(),
                            value: json!(a.exec),
                            enabled: true,
                        })
                        .collect();
                    self.popup = Some(Popup::Picker(Picker {
                        title: tf("bind.kind.title", &[("keys", &keys)]),
                        items,
                        sel: 0,
                        filter: String::new(),
                        target: PickTarget::App(EPick::BindApp(keys)),
                        current: None,
                        anchor: None,
                    }));
                }
                "cmd" => {
                    self.popup = Some(Popup::Input(Input::new(
                        t("bind.kind.cmd"),
                        t("bind.cmd.hint"),
                        "",
                        cp_input(EInput::BindCommand(keys)),
                    )))
                }
                _ => {
                    self.popup = Some(Popup::Input(Input::new(
                        t("bind.kind.web"),
                        t("bind.web.hint"),
                        "https://",
                        cp_input(EInput::BindWeb(keys)),
                    )))
                }
            },
            EPick::BindApp(keys) => {
                let name =
                    apps::name_for(&text, self.apps_cached()).unwrap_or_else(|| text.clone());
                self.add_custom(keys, name, format!("{{ launch = {text:?} }}"));
            }
            EPick::AutostartApp => {
                if text == PICK_CMD {
                    self.popup = Some(Popup::Input(Input::new(
                        t("as.custom"),
                        t("as.custom.hint"),
                        "",
                        cp_input(EInput::AutostartCmd),
                    )));
                } else {
                    self.store
                        .autostart
                        .push(autostart::Entry::new(&text, false));
                }
            }
        }
    }

    /// Texto escrito en un cuadro propio. `Err` = mensaje para el cuadro.
    pub(super) fn on_input(&mut self, input: EInput, text: String) -> Result<(), String> {
        let text = text.trim().to_string();
        match input {
            EInput::BindFilter => self.bind_filter = text,
            EInput::BindCommand(keys) => {
                if text.is_empty() {
                    return Err(t("err.empty"));
                }
                self.add_custom(keys, text.clone(), format!("{text:?}"));
            }
            EInput::BindWeb(keys) => {
                let url = if text.contains("://") {
                    text
                } else {
                    format!("https://{text}")
                };
                let host = url
                    .split("://")
                    .nth(1)
                    .and_then(|r| r.split('/').next())
                    .unwrap_or_default()
                    .trim_start_matches("www.")
                    .to_string();
                if host.is_empty() || !host.contains('.') {
                    return Err(t("err.url"));
                }
                self.add_custom(keys, host, format!("{{ webapp = {url:?} }}"));
            }
            EInput::AutostartCmd => {
                if text.is_empty() {
                    return Err(t("err.empty"));
                }
                self.store
                    .autostart
                    .push(autostart::Entry::new(&text, true));
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ cambios

    /// (nombre, antes, después) legibles para la lista de cambios, en los
    /// ajustes que no son campos del catálogo.
    pub(super) fn describe_list_change(&self, c: &Change) -> Option<(String, String, String)> {
        if let Some(orig) = c.key.strip_prefix("x:bind:") {
            let desc = self
                .store
                .binds
                .iter()
                .find(|b| b.keys == orig)
                .map(|b| b.desc.clone())
                .unwrap_or_else(|| orig.to_string());
            let show = |v: &Option<Value>| match v {
                None => orig.to_string(),
                Some(Value::String(k)) => k.clone(),
                Some(_) => t("bind.disabled"),
            };
            return Some((
                tf("ch.bind", &[("name", &desc)]),
                show(&c.old),
                show(&c.new),
            ));
        }
        if c.key == "x:custom_binds" {
            let count = |v: &Option<Value>| {
                v.as_ref()
                    .and_then(Value::as_array)
                    .map(|a| a.len())
                    .unwrap_or(0)
                    .to_string()
            };
            return Some((t("ch.custom_binds"), count(&c.old), count(&c.new)));
        }
        if let Some(cmd) = c.key.strip_prefix("as:") {
            let name = apps::name_for(cmd, self.apps_cached()).unwrap_or_else(|| cmd.to_string());
            let show = |v: &Option<Value>, missing: &str| match v {
                None => t(missing),
                Some(Value::Bool(true)) => t("as.on"),
                Some(_) => t("as.off"),
            };
            return Some((
                tf("ch.autostart", &[("name", &name)]),
                show(&c.old, "as.new"),
                show(&c.new, "as.removed"),
            ));
        }
        None
    }
}

fn cp_input(input: EInput) -> lizarbe_core::popup::InputTarget<super::E> {
    lizarbe_core::popup::InputTarget::App(input)
}
