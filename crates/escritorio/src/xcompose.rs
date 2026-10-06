//! `~/.XCompose`: atajos de texto (tecla Compose + letras → texto). Solo se
//! leen y escriben las líneas simples `<Multi_key> <space> <a> <b> : "texto"`;
//! el resto del archivo (include, emojis…) no se toca.

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Letras o cifras tras Compose (p. ej. `ne`).
    pub keys: String,
    pub text: String,
    /// Línea del archivo; `None` si es nueva.
    pub line: Option<usize>,
}

fn key_token(c: char) -> String {
    match c {
        ' ' => "<space>".into(),
        c => format!("<{c}>"),
    }
}

impl Entry {
    pub fn new(keys: &str, text: &str) -> Entry {
        Entry {
            keys: keys.to_string(),
            text: text.to_string(),
            line: None,
        }
    }

    fn to_line(&self) -> String {
        let seq: Vec<String> = self.keys.chars().map(key_token).collect();
        let text = self.text.replace('\\', "\\\\").replace('"', "\\\"");
        format!("<Multi_key> <space> {} : \"{text}\"", seq.join(" "))
    }
}

/// Teclas válidas: letras y cifras.
pub fn valid_keys(s: &str) -> bool {
    !s.is_empty() && s.len() <= 8 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

fn parse_line(l: &str) -> Option<(String, String)> {
    let (seq, rest) = l.split_once(':')?;
    let rest = rest.trim();
    let text = rest.strip_prefix('"')?.strip_suffix('"')?;
    let mut tokens = seq.split_whitespace();
    if tokens.next()? != "<Multi_key>" || tokens.next()? != "<space>" {
        return None;
    }
    let mut keys = String::new();
    for t in tokens {
        let c = t.strip_prefix('<')?.strip_suffix('>')?;
        let mut cs = c.chars();
        let ch = cs.next()?;
        if cs.next().is_some() || !ch.is_ascii_alphanumeric() {
            return None;
        }
        keys.push(ch);
    }
    (!keys.is_empty()).then(|| (keys, text.replace("\\\"", "\"").replace("\\\\", "\\")))
}

pub fn parse(text: &str) -> Vec<Entry> {
    text.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let (keys, text) = parse_line(l.trim())?;
            Some(Entry {
                keys,
                text,
                line: Some(i),
            })
        })
        .collect()
}

pub fn render(text: &str, entries: &[Entry]) -> String {
    let mut out = String::new();
    for (i, line) in text.lines().enumerate() {
        if parse_line(line.trim()).is_some() {
            if let Some(e) = entries.iter().find(|e| e.line == Some(i)) {
                out.push_str(&e.to_line());
                out.push('\n');
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for e in entries.iter().filter(|e| e.line.is_none()) {
        out.push_str(&e.to_line());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "include \"/usr/share/omarchy/default/xcompose\"\n\n# Identification\n<Multi_key> <space> <n> : \"leonardo\"\n<Multi_key> <m> <s> : \"emoji\"\n";

    #[test]
    fn reads_only_simple_lines() {
        let e = parse(FILE);
        assert_eq!(
            e,
            vec![Entry {
                keys: "n".into(),
                text: "leonardo".into(),
                line: Some(3)
            }]
        );
    }

    #[test]
    fn edits_without_touching_the_rest() {
        let mut e = parse(FILE);
        e[0].text = "Leo \"L\"".into();
        e.push(Entry::new("ne", "a@b.c"));
        let out = render(FILE, &e);
        assert!(out.contains("<Multi_key> <m> <s> : \"emoji\""));
        assert!(out.contains("<Multi_key> <space> <n> : \"Leo \\\"L\\\"\""));
        assert!(out.ends_with("<Multi_key> <space> <n> <e> : \"a@b.c\"\n"));
        assert_eq!(parse(&out).len(), 2);
        assert_eq!(render(FILE, &parse(FILE)), FILE);
        e.remove(0);
        assert!(!render(FILE, &e).contains("leonardo"));
        assert!(valid_keys("ne1") && !valid_keys("n e") && !valid_keys(""));
    }
}
