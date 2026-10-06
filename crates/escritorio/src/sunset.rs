//! `~/.config/hypr/hyprsunset.conf`: horario de la luz nocturna. Se entiende
//! como dos perfiles: de día (`identity`, sin filtro) desde `end` y de noche
//! (`temperature`) desde `start`.

#[derive(Debug, Clone, PartialEq)]
pub struct Night {
    /// Hora a la que empieza el filtro (HH:MM).
    pub start: String,
    /// Hora a la que termina el filtro (HH:MM).
    pub end: String,
    /// Temperatura de color en kelvin; más baja = más cálida.
    pub temp: i64,
}

impl Default for Night {
    fn default() -> Self {
        Night {
            start: "20:00".into(),
            end: "07:00".into(),
            temp: 4000,
        }
    }
}

/// (hora, temperatura) de cada perfil; `None` en la temperatura si es `identity`.
fn profiles(text: &str) -> Vec<(String, Option<i64>)> {
    let mut out = vec![];
    let mut cur: Option<(String, Option<i64>)> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('#') {
            continue;
        }
        if l.starts_with("profile") && l.ends_with('{') {
            cur = Some((String::new(), None));
        } else if l.starts_with('}') {
            if let Some(p) = cur.take()
                && !p.0.is_empty()
            {
                out.push(p);
            }
        } else if let (Some(p), Some((k, v))) = (cur.as_mut(), l.split_once('=')) {
            match k.trim() {
                "time" => p.0 = v.trim().to_string(),
                "temperature" => p.1 = v.trim().parse().ok(),
                _ => {}
            }
        }
    }
    out
}

/// Horario del archivo, si tiene un perfil con temperatura.
pub fn parse(text: &str) -> Option<Night> {
    let ps = profiles(text);
    let (start, temp) = ps.iter().find_map(|(t, k)| k.map(|k| (t.clone(), k)))?;
    let end = ps
        .iter()
        .find(|(_, k)| k.is_none())
        .map(|(t, _)| t.clone())
        .unwrap_or_else(|| Night::default().end);
    Some(Night { start, end, temp })
}

pub fn render(n: &Night) -> String {
    format!(
        "# Gestionado por Escritorio (Lizarbe). Luz nocturna: de día sin filtro y de\n\
         # noche con el filtro cálido. Requiere que hyprsunset se inicie con la sesión.\n\
         profile {{\n    time = {}\n    identity = true\n}}\n\n\
         profile {{\n    time = {}\n    temperature = {}\n}}\n",
        n.end, n.start, n.temp
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_default_template_as_none() {
        let t = "profile {\n    time = 07:00\n    identity = true\n}\n# profile {\n#     time = 20:00\n#     temperature = 4000\n# }\n";
        assert_eq!(parse(t), None);
    }

    #[test]
    fn roundtrips() {
        let n = Night {
            start: "21:30".into(),
            end: "06:45".into(),
            temp: 3500,
        };
        assert_eq!(parse(&render(&n)), Some(n));
    }
}
