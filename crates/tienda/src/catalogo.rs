//! Catálogo curado de aplicaciones (`catalogo.toml`, embebido en el binario).

use std::collections::HashSet;

use lizarbe_core::i18n::{Lang, lang};
use lizarbe_core::theme;
use lizarbe_core::ui::fold;
use serde::Deserialize;

const TOML: &str = include_str!("../catalogo.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct Texto {
    pub es: String,
    pub en: String,
}

impl Texto {
    pub fn get(&self) -> &str {
        match lang() {
            Lang::Es => &self.es,
            Lang::En => &self.en,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Categoria {
    pub id: String,
    pub icono: String,
    pub color: Option<String>,
    pub nombre: Texto,
}

/// De dónde sale el paquete: los repositorios de pacman o el AUR (comunidad).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fuente {
    #[default]
    Repos,
    Aur,
}

impl Fuente {
    pub fn parse(s: &str) -> Option<Fuente> {
        match s {
            "repos" => Some(Fuente::Repos),
            "aur" => Some(Fuente::Aur),
            _ => None,
        }
    }
}

fn rgb(s: &str) -> Option<(u8, u8, u8)> {
    match theme::hex(s)? {
        ratatui::style::Color::Rgb(r, g, b) => Some((r, g, b)),
        _ => None,
    }
}

fn si() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct App {
    pub id: String,
    pub nombre: String,
    pub paquetes: Vec<String>,
    pub categoria: String,
    /// Carácter de Nerd Font; sin él se usa el de la categoría.
    pub icono: Option<String>,
    /// `#rrggbb` de la marca; sin él se usa el de la categoría.
    pub color: Option<String>,
    #[serde(default)]
    pub fuente: Fuente,
    pub resumen: Texto,
    pub descripcion: Texto,
    #[serde(default)]
    pub etiquetas: Vec<String>,
    #[serde(default)]
    pub recomendada: bool,
    pub desktop: Option<String>,
    #[serde(default = "si")]
    pub quitar: bool,
    /// Paquete que no es del catálogo: viene de una búsqueda en el repositorio o de lo instalado.
    #[serde(default)]
    pub externa: bool,
}

impl App {
    /// Tarjeta para un paquete cualquiera, con el aspecto de las del catálogo.
    pub fn de_paquete(p: &crate::sistema::Encontrado, fuente: Fuente, quitar: bool) -> App {
        let texto = Texto {
            es: p.descripcion.clone(),
            en: p.descripcion.clone(),
        };
        App {
            id: format!("pkg:{}", p.nombre),
            nombre: p.nombre.clone(),
            paquetes: vec![p.nombre.clone()],
            categoria: String::new(),
            icono: Some("\u{f0d6}".into()),
            color: Some(
                if fuente == Fuente::Aur {
                    "#8a7fb8"
                } else {
                    "#6f8fa8"
                }
                .into(),
            ),
            fuente,
            resumen: texto.clone(),
            descripcion: texto,
            etiquetas: if p.repo.is_empty() {
                vec![]
            } else {
                vec![p.repo.clone()]
            },
            recomendada: false,
            desktop: None,
            quitar,
            externa: true,
        }
    }

    /// ¿Coincide con todas las palabras de `query` (ya normalizada con `fold`)?
    pub fn coincide(&self, query: &str) -> bool {
        let heno = fold(&format!(
            "{} {} {} {} {} {}",
            self.nombre,
            self.resumen.es,
            self.resumen.en,
            self.descripcion.es,
            self.descripcion.en,
            self.etiquetas.join(" ")
        ));
        query.split_whitespace().all(|w| heno.contains(w))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Catalogo {
    #[serde(rename = "categoria")]
    pub categorias: Vec<Categoria>,
    #[serde(rename = "app")]
    pub apps: Vec<App>,
}

impl Catalogo {
    pub fn embebido() -> Catalogo {
        Self::parse(TOML).expect("catalogo.toml inválido")
    }

    pub fn parse(text: &str) -> Result<Catalogo, String> {
        let cat: Catalogo = toml::from_str(text).map_err(|e| e.to_string())?;
        let problemas = cat.problemas();
        if problemas.is_empty() {
            Ok(cat)
        } else {
            Err(problemas.join("\n"))
        }
    }

    pub fn app(&self, id: &str) -> Option<usize> {
        self.apps.iter().position(|a| a.id == id)
    }

    /// ¿Hay alguna app de `fuente` en la categoría `i`?
    pub fn categoria_con_apps(&self, i: usize, fuente: Fuente) -> bool {
        self.apps
            .iter()
            .any(|a| a.fuente == fuente && a.categoria == self.categorias[i].id)
    }

    /// Icono de la app o, si no tiene, el de su categoría.
    pub fn icono_de(&self, a: &App) -> String {
        a.icono
            .clone()
            .or_else(|| {
                self.categoria(&a.categoria)
                    .map(|i| self.categorias[i].icono.clone())
            })
            .unwrap_or_else(|| "?".into())
    }

    /// Color de la app o, si no tiene, el de su categoría (gris si ninguno es válido).
    pub fn color_de(&self, a: &App) -> (u8, u8, u8) {
        let cat = self
            .categoria(&a.categoria)
            .and_then(|i| self.categorias[i].color.as_deref());
        a.color
            .as_deref()
            .and_then(rgb)
            .or_else(|| cat.and_then(rgb))
            .unwrap_or((122, 122, 122))
    }

    pub fn categoria(&self, id: &str) -> Option<usize> {
        self.categorias.iter().position(|c| c.id == id)
    }

    fn problemas(&self) -> Vec<String> {
        let mut out = vec![];
        let mut ids = HashSet::new();
        for c in &self.categorias {
            if c.color.as_deref().is_some_and(|x| rgb(x).is_none()) {
                out.push(format!("categoría {}: color no válido", c.id));
            }
            if !ids.insert(format!("cat:{}", c.id)) {
                out.push(format!("categoría repetida: {}", c.id));
            }
        }
        let mut apps = HashSet::new();
        for a in &self.apps {
            if !apps.insert(a.id.as_str()) {
                out.push(format!("app repetida: {}", a.id));
            }
            if a.icono.as_deref().is_some_and(|i| i.chars().count() != 1) {
                out.push(format!("{}: el icono debe ser un solo carácter", a.id));
            }
            if a.color.as_deref().is_some_and(|c| rgb(c).is_none()) {
                out.push(format!("{}: color no válido", a.id));
            }
            if self.categoria(&a.categoria).is_none() {
                out.push(format!("{}: categoría desconocida «{}»", a.id, a.categoria));
            }
            if a.paquetes.is_empty() {
                out.push(format!("{}: sin paquetes", a.id));
            }
            if [
                &a.resumen.es,
                &a.resumen.en,
                &a.descripcion.es,
                &a.descripcion.en,
            ]
            .iter()
            .any(|t| t.trim().is_empty())
            {
                out.push(format!("{}: faltan textos", a.id));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_is_valid() {
        let c = Catalogo::embebido();
        assert!(c.apps.len() >= 30);
        for f in [Fuente::Repos, Fuente::Aur] {
            assert!(c.apps.iter().any(|a| a.fuente == f && a.recomendada));
        }
    }

    #[test]
    fn every_app_has_its_own_icon_and_a_valid_color() {
        let c = Catalogo::embebido();
        for a in &c.apps {
            assert!(a.icono.is_some(), "{} sin icono", a.id);
            assert!(a.color.is_some(), "{} sin color", a.id);
            assert_ne!(c.color_de(a), (122, 122, 122), "{}", a.id);
        }
        for cat in &c.categorias {
            assert!(cat.color.is_some(), "categoría {} sin color", cat.id);
        }
    }

    #[test]
    fn falls_back_to_category_icon_and_color() {
        let mut c = Catalogo::embebido();
        let i = c.app("firefox").unwrap();
        c.apps[i].icono = None;
        c.apps[i].color = None;
        let cat = c.categoria(&c.apps[i].categoria).unwrap();
        assert_eq!(c.icono_de(&c.apps[i]), c.categorias[cat].icono);
        assert_eq!(c.color_de(&c.apps[i]), rgb("#4285F4").unwrap());
    }

    #[test]
    fn rejects_bad_icon_and_color() {
        let base = r#"
[[categoria]]
id = "x"
icono = "i"
nombre = { es = "X", en = "X" }
"#;
        let app = |extra: &str| {
            format!(
                "{base}[[app]]\nid = \"a\"\nnombre = \"N\"\npaquetes = [\"p\"]\ncategoria = \"x\"\n{extra}\nresumen = {{ es = \"a\", en = \"a\" }}\ndescripcion = {{ es = \"a\", en = \"a\" }}\n"
            )
        };
        assert!(Catalogo::parse(&app("color = \"#112233\"\nicono = \"x\"")).is_ok());
        assert!(
            Catalogo::parse(&app("color = \"rojo\""))
                .unwrap_err()
                .contains("color")
        );
        assert!(
            Catalogo::parse(&app("icono = \"ab\""))
                .unwrap_err()
                .contains("icono")
        );
    }

    #[test]
    fn source_defaults_to_repos_and_parses() {
        let c = Catalogo::embebido();
        assert_eq!(c.apps[c.app("firefox").unwrap()].fuente, Fuente::Repos);
        assert_eq!(c.apps[c.app("onlyoffice").unwrap()].fuente, Fuente::Aur);
        assert_eq!(Fuente::parse("aur"), Some(Fuente::Aur));
        assert_eq!(Fuente::parse("x"), None);
    }

    #[test]
    fn rejects_unknown_category_and_duplicates() {
        let base = r#"
[[categoria]]
id = "x"
icono = "i"
nombre = { es = "X", en = "X" }
"#;
        let app = |id: &str, cat: &str| {
            format!(
                "[[app]]\nid = \"{id}\"\nnombre = \"N\"\npaquetes = [\"p\"]\ncategoria = \"{cat}\"\nresumen = {{ es = \"a\", en = \"a\" }}\ndescripcion = {{ es = \"a\", en = \"a\" }}\n"
            )
        };
        assert!(Catalogo::parse(&format!("{base}{}", app("a", "x"))).is_ok());
        let e = Catalogo::parse(&format!("{base}{}", app("a", "nope"))).unwrap_err();
        assert!(e.contains("categoría desconocida"));
        let e = Catalogo::parse(&format!("{base}{}{}", app("a", "x"), app("a", "x"))).unwrap_err();
        assert!(e.contains("app repetida"));
    }

    #[test]
    fn search_ignores_accents_case_and_language() {
        let c = Catalogo::embebido();
        let q = |s: &str| -> Vec<&str> {
            c.apps
                .iter()
                .filter(|a| a.coincide(&fold(s)))
                .map(|a| a.id.as_str())
                .collect()
        };
        assert!(q("MUSICA").contains(&"spotify"));
        assert!(q("video editar").contains(&"kdenlive"));
        assert!(q("browser").contains(&"firefox"));
        assert!(q("zzzz").is_empty());
    }
}
