//! Sección Temas: lista de temas propios, crear, duplicar, eliminar,
//! aplicar y editar sus colores.

use lizarbe_core::form::NoteKind;
use lizarbe_core::popup::{Input, PickItem, PickTarget, Picker};
use lizarbe_core::schema::{FieldDef, Kind, Opt};
use serde_json::json;

use super::{Act, App, Confirm, EInput, EPick, FieldRow, Popup, Row};
use crate::i18n::{t, tf};
use crate::store::Bind;
use crate::themes::{self, COLOR_KEYS, SIMPLE_KEYS};

fn cp_input(i: EInput) -> lizarbe_core::popup::InputTarget<super::E> {
    lizarbe_core::popup::InputTarget::App(i)
}

impl App {
    pub(super) fn theme_rows(&self) -> Vec<Row> {
        match &self.editing_theme {
            Some(slug) => self.theme_editor_rows(slug),
            None => self.theme_list_rows(),
        }
    }

    fn theme_list_rows(&self) -> Vec<Row> {
        let dirs = self.store.theme_dirs();
        let mut rows = vec![
            Row::Note(t("note.themes"), NoteKind::Info),
            Row::Action(t("th.new"), Act::NewTheme),
        ];
        let mine = dirs.list_user();
        if mine.is_empty() {
            rows.push(Row::Note(t("th.none"), NoteKind::Info));
            return rows;
        }
        rows.push(Row::Header(t("th.mine")));
        let current = self.store.paths.current_theme();
        for slug in mine {
            let label = if slug == current {
                tf("th.current", &[("name", &slug)])
            } else {
                slug.clone()
            };
            rows.push(Row::Action(label, Act::OpenTheme(slug)));
        }
        rows
    }

    fn theme_editor_rows(&self, slug: &str) -> Vec<Row> {
        let mut rows = vec![
            Row::Header(tf("th.editing", &[("name", slug)])),
            Row::Action(t("th.back"), Act::CloseTheme),
        ];
        let colors = self.store.theme_colors(slug);
        let mode = Opt::new(json!("dark"), t("th.mode=dark"));
        let light = Opt::new(json!("light"), t("th.mode=light"));
        let mode_key = format!("th:{slug}:mode");
        let def = FieldDef::new(&mode_key, t("th.mode"), Kind::Enum(vec![mode, light]))
            .desc(t("th.mode.d"))
            .default(json!("dark"));
        rows.push(Row::Field(FieldRow::new(def, Bind(mode_key), &self.store)));
        rows.push(Row::Header(t("th.colors")));
        for key in COLOR_KEYS {
            if !self.advanced && !SIMPLE_KEYS.contains(&key) {
                continue;
            }
            if !colors.iter().any(|(k, _)| k == key) {
                continue;
            }
            let bk = format!("th:{slug}:{key}");
            let def = FieldDef::new(&bk, t(&format!("th.c.{key}")), Kind::Color)
                .desc(t(&format!("th.c.{key}.d")));
            rows.push(Row::Field(FieldRow::new(def, Bind(bk), &self.store)));
        }
        rows
    }

    pub(super) fn theme_act(&mut self, act: Act) {
        match act {
            Act::NewTheme => {
                self.popup = Some(Popup::Input(Input::new(
                    t("th.new"),
                    t("th.name.hint"),
                    "",
                    cp_input(EInput::ThemeName),
                )))
            }
            Act::OpenTheme(slug) => {
                self.editing_theme = Some(slug);
                self.form_state().sel = 0;
            }
            Act::CloseTheme => self.editing_theme = None,
            _ => {}
        }
    }

    /// El usuario escribió el nombre del tema nuevo: se elige el tema base.
    pub(super) fn theme_name_entered(&mut self, text: &str) -> Result<(), String> {
        let slug = themes::slugify(text);
        if slug.is_empty() {
            return Err(t("th.name.err"));
        }
        let dirs = self.store.theme_dirs();
        if dirs.is_user(&slug) || dirs.list_all().contains(&slug) {
            return Err(t("th.exists"));
        }
        let items = dirs
            .list_all()
            .into_iter()
            .map(|n| PickItem {
                detail: if dirs.is_user(&n) {
                    t("th.base.mine")
                } else {
                    t("th.base.omarchy")
                },
                label: n.clone(),
                group: String::new(),
                value: json!(n),
                enabled: true,
            })
            .collect();
        self.popup = Some(Popup::Picker(Picker {
            title: tf("th.base.title", &[("name", &slug)]),
            items,
            sel: 0,
            filter: String::new(),
            target: PickTarget::App(EPick::ThemeBase(slug)),
            current: None,
            anchor: None,
        }));
        Ok(())
    }

    pub(super) fn theme_create(&mut self, slug: &str, base: &str) {
        if let Err(e) = self.theme_create_checked(slug, base, false) {
            self.message(t("msg.error"), vec![e]);
        }
    }

    pub(super) fn theme_create_checked(
        &mut self,
        slug: &str,
        base: &str,
        duplicate: bool,
    ) -> Result<(), String> {
        if slug.is_empty() {
            return Err(t("th.name.err"));
        }
        let dirs = self.store.theme_dirs();
        if dirs.is_user(slug) || dirs.list_all().iter().any(|n| n == slug) {
            return Err(t("th.exists"));
        }
        dirs.create(slug, base).map_err(|e| format!("{e:#}"))?;
        self.editing_theme = Some(slug.to_string());
        self.toast(
            tf(
                if duplicate {
                    "th.duplicated"
                } else {
                    "th.created"
                },
                &[("name", slug)],
            ),
            NoteKind::Info,
        );
        Ok(())
    }

    fn user_theme(&self, n: usize) -> Option<String> {
        self.store.theme_dirs().list_user().get(n).cloned()
    }

    pub(super) fn theme_apply(&mut self, n: usize) {
        let Some(slug) = self.user_theme(n) else {
            return;
        };
        if self.store.paths.sandbox {
            return self.toast(t("th.sandbox"), NoteKind::Warn);
        }
        match lizarbe_core::ipc::run("omarchy-theme-set", &[&slug]) {
            Ok(_) => self.toast(tf("th.applied", &[("name", &slug)]), NoteKind::Info),
            Err(e) => self.message(t("msg.error"), vec![e]),
        }
    }

    pub(super) fn theme_duplicate(&mut self, n: usize) {
        let Some(slug) = self.user_theme(n) else {
            return;
        };
        self.popup = Some(Popup::Input(Input::new(
            tf("th.duplicate_title", &[("name", &slug)]),
            t("th.name.hint"),
            "",
            cp_input(EInput::ThemeDuplicate(slug)),
        )));
    }

    pub(super) fn theme_delete(&mut self, n: usize) {
        let Some(slug) = self.user_theme(n) else {
            return;
        };
        self.popup = Some(Popup::Confirm {
            title: tf("th.delete_title", &[("name", &slug)]),
            lines: vec![t("th.delete.body")],
            action: Confirm::DeleteTheme(slug),
        });
    }

    pub(super) fn delete_theme(&mut self, slug: &str) {
        if slug == self.store.paths.current_theme() {
            return self.toast(t("th.delete.current"), NoteKind::Warn);
        }
        match self.store.theme_dirs().delete(slug) {
            Ok(()) => {
                self.store.theme_edits.remove(slug);
                if self.editing_theme.as_deref() == Some(slug) {
                    self.editing_theme = None;
                }
                self.toast(tf("th.deleted", &[("name", slug)]), NoteKind::Info);
            }
            Err(e) => self.message(t("msg.error"), vec![format!("{e:#}")]),
        }
    }
}
