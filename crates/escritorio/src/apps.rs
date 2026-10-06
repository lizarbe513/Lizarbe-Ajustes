//! Aplicaciones instaladas, leídas de sus archivos `.desktop`.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub name: String,
    /// Orden para lanzarla, sin los marcadores `%U`, `%f`…
    pub exec: String,
}

fn dirs() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));
    vec![
        data.join("applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/usr/share/applications"),
    ]
}

/// Lee un `.desktop`; `lang` es el idioma para `Name[es]`.
pub fn parse(text: &str, lang: &str) -> Option<App> {
    let mut in_entry = false;
    let (mut name, mut localized, mut exec) = (None, None, None);
    let mut app = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_entry = l == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        match k.trim() {
            "Type" => app = v.trim() == "Application",
            "NoDisplay" | "Hidden" if v.trim() == "true" => return None,
            "Name" => name = Some(v.trim().to_string()),
            k if k == format!("Name[{lang}]") => localized = Some(v.trim().to_string()),
            "Exec" => exec = Some(v.trim().to_string()),
            _ => {}
        }
    }
    let exec = exec?
        .split_whitespace()
        .filter(|p| !(p.starts_with('%') && p.len() == 2))
        .collect::<Vec<_>>()
        .join(" ");
    (app && !exec.is_empty()).then(|| App {
        name: localized.or(name).unwrap_or_else(|| exec.clone()),
        exec,
    })
}

/// Aplicaciones visibles, ordenadas por nombre. Si hay dos con el mismo
/// archivo, gana la del usuario.
pub fn installed(lang: &str) -> Vec<App> {
    let mut seen: Vec<String> = vec![];
    let mut out: Vec<App> = vec![];
    for dir in dirs() {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in rd.flatten() {
            let file = e.file_name().to_string_lossy().to_string();
            if !file.ends_with(".desktop") || seen.contains(&file) {
                continue;
            }
            seen.push(file);
            if let Ok(text) = std::fs::read_to_string(e.path())
                && let Some(app) = parse(&text, lang)
            {
                out.push(app);
            }
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Nombre de la aplicación que lanza `cmd`, si se reconoce.
pub fn name_for(cmd: &str, apps: &[App]) -> Option<String> {
    let first = |s: &str| s.split_whitespace().next().unwrap_or_default().to_string();
    let bin = first(cmd.trim_start_matches("uwsm-app -- "));
    apps.iter()
        .find(|a| a.exec == cmd || first(&a.exec) == bin)
        .map(|a| a.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_files() {
        let text = "[Desktop Entry]\nType=Application\nName=Files\nName[es]=Archivos\nExec=nautilus --new-window %U\n[Desktop Action new]\nName=Nueva\nExec=other\n";
        assert_eq!(
            parse(text, "es"),
            Some(App {
                name: "Archivos".into(),
                exec: "nautilus --new-window".into()
            })
        );
        assert_eq!(
            parse(
                "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n",
                "es"
            ),
            None
        );
    }

    #[test]
    fn finds_names_for_commands() {
        let apps = vec![App {
            name: "Archivos".into(),
            exec: "nautilus --new-window".into(),
        }];
        assert_eq!(name_for("nautilus", &apps), Some("Archivos".into()));
        assert_eq!(
            name_for("uwsm-app -- nautilus", &apps),
            Some("Archivos".into())
        );
        assert_eq!(name_for("hyprsunset", &apps), None);
    }
}
