//! Vista previa de formatos de fecha/hora de Qt (los que usa `omarchy.clock`).
//!
//! Soporta los tokens habituales: d dd ddd dddd, M MM MMM MMMM, yy yyyy,
//! h hh H HH, m mm, s ss, AP ap A a, w ww (semana ISO) y literales entre
//! comillas simples ('' = comilla).

use chrono::{Datelike, NaiveDateTime, Timelike};

use crate::i18n::Lang;

const DAYS_EN: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const DAYS_ES: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const MONTHS_ES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// Idioma de los nombres de días y meses: el mismo que usará Qt (LC_TIME).
pub fn time_lang() -> Lang {
    for var in ["LC_ALL", "LC_TIME", "LANG"] {
        if let Ok(v) = std::env::var(var)
            && !v.is_empty()
        {
            return Lang::parse(&v).unwrap_or(Lang::En);
        }
    }
    Lang::En
}

fn short(name: &str) -> String {
    name.chars().take(3).collect()
}

pub fn format(fmt: &str, dt: &NaiveDateTime, lang: Lang) -> String {
    let (days, months) = match lang {
        Lang::Es => (DAYS_ES, MONTHS_ES),
        Lang::En => (DAYS_EN, MONTHS_EN),
    };
    let has_ampm = {
        // AP/ap/A/a fuera de literales activa el reloj de 12 horas.
        let mut in_quote = false;
        let mut found = false;
        for c in fmt.chars() {
            if c == '\'' {
                in_quote = !in_quote;
            } else if !in_quote && (c == 'A' || c == 'a') {
                found = true;
            }
        }
        found
    };
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            // Literal: '' es una comilla.
            if chars.get(i + 1) == Some(&'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            i += 1;
            while i < chars.len() {
                if chars[i] == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        out.push('\'');
                        i += 2;
                        continue;
                    }
                    break;
                }
                out.push(chars[i]);
                i += 1;
            }
            i += 1;
            continue;
        }
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        let wd = dt.weekday().num_days_from_monday() as usize;
        let h12 = {
            let h = dt.hour() % 12;
            if h == 0 { 12 } else { h }
        };
        let consumed = match c {
            'd' => {
                let n = n.min(4);
                out.push_str(&match n {
                    1 => dt.day().to_string(),
                    2 => format!("{:02}", dt.day()),
                    3 => short(days[wd]),
                    _ => days[wd].to_string(),
                });
                n
            }
            'M' => {
                let n = n.min(4);
                let m = dt.month0() as usize;
                out.push_str(&match n {
                    1 => dt.month().to_string(),
                    2 => format!("{:02}", dt.month()),
                    3 => short(months[m]),
                    _ => months[m].to_string(),
                });
                n
            }
            'y' => {
                if n >= 4 {
                    out.push_str(&format!("{:04}", dt.year()));
                    4
                } else if n >= 2 {
                    out.push_str(&format!("{:02}", dt.year() % 100));
                    2
                } else {
                    out.push('y');
                    1
                }
            }
            'h' | 'H' => {
                let n = n.min(2);
                let v = if c == 'H' || !has_ampm {
                    dt.hour()
                } else {
                    h12
                };
                out.push_str(&if n == 2 {
                    format!("{v:02}")
                } else {
                    v.to_string()
                });
                n
            }
            'm' => {
                let n = n.min(2);
                out.push_str(&if n == 2 {
                    format!("{:02}", dt.minute())
                } else {
                    dt.minute().to_string()
                });
                n
            }
            's' => {
                let n = n.min(2);
                out.push_str(&if n == 2 {
                    format!("{:02}", dt.second())
                } else {
                    dt.second().to_string()
                });
                n
            }
            'w' => {
                let n = n.min(2);
                let w = dt.iso_week().week();
                out.push_str(&if n == 2 {
                    format!("{w:02}")
                } else {
                    w.to_string()
                });
                n
            }
            'A' | 'a' => {
                let pm = dt.hour() >= 12;
                let upper = c == 'A';
                let mut s = if pm { "PM" } else { "AM" }.to_string();
                if !upper {
                    s = s.to_lowercase();
                }
                out.push_str(&s);
                // AP / ap ocupan dos caracteres.
                if matches!(chars.get(i + 1), Some('P') | Some('p')) {
                    2
                } else {
                    1
                }
            }
            other => {
                out.push(other);
                1
            }
        };
        i += consumed;
    }
    out
}

/// Vista previa en una línea (los saltos se muestran como " │ ").
pub fn preview(fmt: &str) -> String {
    let now = chrono::Local::now().naive_local();
    format(fmt, &now, time_lang()).replace('\n', " │ ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn dt() -> NaiveDateTime {
        // Martes 6 de octubre de 2026, 14:05:09 (semana ISO 41).
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap()
            .and_hms_opt(14, 5, 9)
            .unwrap()
    }

    #[test]
    fn omarchy_defaults() {
        assert_eq!(format("dddd HH:mm", &dt(), Lang::En), "Tuesday 14:05");
        assert_eq!(format("dddd h:mm AP", &dt(), Lang::En), "Tuesday 2:05 PM");
        assert_eq!(
            format("d MMMM 'W'ww yyyy", &dt(), Lang::En),
            "6 October W41 2026"
        );
        assert_eq!(format("HH\n—\nmm", &dt(), Lang::En), "14\n—\n05");
        assert_eq!(
            format("dd\nMMM\n'W'ww\n''yy", &dt(), Lang::En),
            "06\nOct\nW41\n'26"
        );
        assert_eq!(
            format("yyyy-MM-dd HH:mm", &dt(), Lang::En),
            "2026-10-06 14:05"
        );
        assert_eq!(
            format("ddd d MMM h:mm ap", &dt(), Lang::Es),
            "mar 6 oct 2:05 pm"
        );
    }
}
