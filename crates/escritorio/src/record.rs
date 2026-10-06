//! Grabar una combinación de teclas.
//!
//! La terminal no recibe combinaciones con Super (Hyprland las usa), así que
//! se escuchan las teclas desde Hyprland: un submapa vacío desactiva los
//! atajos mientras se graba y un gancho Lua escribe en un archivo las teclas
//! pulsadas. La grabación termina al soltar las teclas; Escape la cancela y,
//! por seguridad, se cancela sola a los 15 segundos.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use lizarbe_core::hypr;
use serde_json::Value;

use crate::binds::normalize;

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// Esperando; lleva lo que se lleva pulsado.
    Waiting(String),
    Done(String),
    Cancelled,
}

pub struct Recorder {
    file: PathBuf,
}

/// Teclas con nombre fijo (código de teclado XKB → nombre en Hyprland).
const KEYS: &[(u32, &str)] = &[
    (9, "ESCAPE"),
    (22, "BACKSPACE"),
    (23, "TAB"),
    (36, "RETURN"),
    (65, "SPACE"),
    (37, "CTRL"),
    (105, "CTRL"),
    (50, "SHIFT"),
    (62, "SHIFT"),
    (64, "ALT"),
    (108, "ALT"),
    (133, "SUPER"),
    (134, "SUPER"),
    (107, "PRINT"),
    (218, "PRINT"),
    (110, "HOME"),
    (111, "UP"),
    (112, "PAGE_UP"),
    (113, "LEFT"),
    (114, "RIGHT"),
    (115, "END"),
    (116, "DOWN"),
    (117, "PAGE_DOWN"),
    (118, "INSERT"),
    (119, "DELETE"),
    (67, "F1"),
    (68, "F2"),
    (69, "F3"),
    (70, "F4"),
    (71, "F5"),
    (72, "F6"),
    (73, "F7"),
    (74, "F8"),
    (75, "F9"),
    (76, "F10"),
    (95, "F11"),
    (96, "F12"),
    (10, "1"),
    (11, "2"),
    (12, "3"),
    (13, "4"),
    (14, "5"),
    (15, "6"),
    (16, "7"),
    (17, "8"),
    (18, "9"),
    (19, "0"),
    (24, "Q"),
    (25, "W"),
    (26, "E"),
    (27, "R"),
    (28, "T"),
    (29, "Y"),
    (30, "U"),
    (31, "I"),
    (32, "O"),
    (33, "P"),
    (38, "A"),
    (39, "S"),
    (40, "D"),
    (41, "F"),
    (42, "G"),
    (43, "H"),
    (44, "J"),
    (45, "K"),
    (46, "L"),
    (52, "Z"),
    (53, "X"),
    (54, "C"),
    (55, "V"),
    (56, "B"),
    (57, "N"),
    (58, "M"),
    (121, "XF86AudioMute"),
    (122, "XF86AudioLowerVolume"),
    (123, "XF86AudioRaiseVolume"),
    (148, "XF86Calculator"),
    (171, "XF86AudioNext"),
    (172, "XF86AudioPlay"),
    (173, "XF86AudioPrev"),
    (232, "XF86MonBrightnessDown"),
    (233, "XF86MonBrightnessUp"),
];

/// Nombre de cada código: los fijos y, para el resto, el símbolo de la
/// distribución de teclado activa (p. ej. `ntilde` en español).
fn names() -> &'static HashMap<u32, String> {
    static NAMES: OnceLock<HashMap<u32, String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut map: HashMap<u32, String> = KEYS.iter().map(|(c, n)| (*c, n.to_string())).collect();
        if let Ok(dump) = lizarbe_core::ipc::run("xkbcli", &["dump-keymap-wayland"]) {
            for (code, sym) in keymap_symbols(&dump) {
                map.entry(code).or_insert(sym);
            }
        }
        map
    })
}

/// (código, primer símbolo) de un volcado de `xkbcli dump-keymap-wayland`.
fn keymap_symbols(dump: &str) -> Vec<(u32, String)> {
    let mut codes: HashMap<String, u32> = HashMap::new();
    let mut out = vec![];
    for line in dump.lines() {
        let l = line.trim();
        // <AC10> = 47;
        if let Some(rest) = l.strip_prefix('<')
            && let Some((name, num)) = rest.split_once('>')
            && let Some(n) = num.trim().strip_prefix('=')
            && let Ok(code) = n.trim().trim_end_matches(';').trim().parse::<u32>()
        {
            codes.insert(name.to_string(), code);
            continue;
        }
        // key <AC10> { [ ntilde, Ntilde ] };
        if let Some(rest) = l.strip_prefix("key <")
            && let Some((name, body)) = rest.split_once('>')
            && let Some(code) = codes.get(name)
            && let Some(start) = body.find('[')
        {
            let sym = body[start + 1..]
                .split([',', ']'])
                .next()
                .unwrap_or_default()
                .trim();
            if !sym.is_empty() && sym != "NoSymbol" && !sym.starts_with('U') {
                out.push((*code, sym.to_string()));
            }
        }
    }
    out
}

fn combo(codes: &[u32]) -> String {
    let parts: Vec<String> = codes
        .iter()
        .map(|c| {
            names()
                .get(c)
                .cloned()
                .unwrap_or_else(|| format!("code:{c}"))
        })
        .collect();
    normalize(&parts.join(" + "))
}

impl Recorder {
    pub fn start() -> Result<Recorder, String> {
        let file = std::env::temp_dir().join(format!("lizarbe-rec-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let path = file
            .display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let lua = format!(
            r#"
_G.__lz_rec = {{ active = true, done = false, first = nil, held = {{}}, all = {{}}, file = "{path}" }}
local function write_state()
  local r = _G.__lz_rec
  local all = {{}}
  for _, k in ipairs(r.all) do table.insert(all, tostring(k)) end
  local f = io.open(r.file, "w")
  if f then
    f:write(string.format('{{"done":%s,"cancel":%s,"all":[%s]}}',
      r.done and "true" or "false", r.cancel and "true" or "false", table.concat(all, ",")))
    f:close()
  end
end
local function finish(cancel)
  local r = _G.__lz_rec
  if not r or not r.active then return end
  r.active = false
  r.done = not cancel
  r.cancel = cancel
  write_state()
  hl.dispatch(hl.dsp.submap("reset"))
end
_G.__lz_rec_cb = function(keycode, _, state)
  local r = _G.__lz_rec
  if not r or not r.active or keycode == 0 then return end
  if state == 1 then
    if keycode == 9 and #r.all == 0 then return finish(true) end
    if not r.held[keycode] then
      r.held[keycode] = true
      local seen = false
      for _, k in ipairs(r.all) do if k == keycode then seen = true end end
      if not seen then table.insert(r.all, keycode) end
    end
    write_state()
  elseif state == 0 then
    r.held[keycode] = nil
    if next(r.held) == nil and #r.all > 0 then finish(false) end
  end
end
if not _G.__lz_rec_hook then
  _G.__lz_rec_hook = true
  hl.on("input.keyboard.key", function(keycode, time, state)
    if _G.__lz_rec_cb then _G.__lz_rec_cb(keycode, time, state) end
  end)
end
hl.define_submap("lizarbe_rec", function() hl.bind("escape", hl.dsp.submap("reset")) end)
hl.dispatch(hl.dsp.submap("lizarbe_rec"))
hl.timer(function() finish(true) end, {{ timeout = 15000, type = "oneshot" }})
write_state()
"#
        );
        if let Err(e) = hypr::eval(&lua) {
            let _ = std::fs::remove_file(&file);
            return Err(e);
        }
        Ok(Recorder { file })
    }

    pub fn poll(&self) -> State {
        let Some(v) = std::fs::read_to_string(&self.file)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        else {
            return State::Waiting(String::new());
        };
        if v.get("cancel").and_then(Value::as_bool) == Some(true) {
            return State::Cancelled;
        }
        let codes: Vec<u32> = v
            .get("all")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_u64().map(|n| n as u32))
                    .collect()
            })
            .unwrap_or_default();
        let keys = combo(&codes);
        if v.get("done").and_then(Value::as_bool) == Some(true) {
            State::Done(keys)
        } else {
            State::Waiting(keys)
        }
    }

    /// Termina la grabación y devuelve el teclado a la normalidad.
    pub fn stop(&self) {
        let _ = hypr::eval(
            "if _G.__lz_rec then _G.__lz_rec.active = false end\nhl.dispatch(hl.dsp.submap(\"reset\"))",
        );
        let _ = std::fs::remove_file(&self.file);
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Una combinación que solo tiene modificadores no sirve como atajo.
pub fn is_valid(keys: &str) -> bool {
    keys.split(" + ")
        .any(|k| !matches!(k, "SUPER" | "CTRL" | "ALT" | "SHIFT" | ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_keymap_symbols() {
        let dump = "xkb_keycodes {\n  <AC10> = 47;\n  <AB08> = 59;\n};\nxkb_symbols {\n  key <AC10> { [ ntilde, Ntilde ] };\n  key <AB08> {\n type= \"FOUR_LEVEL\", symbols[1]= [ comma, semicolon ] };\n};";
        let syms = keymap_symbols(dump);
        assert!(syms.contains(&(47, "ntilde".to_string())));
    }

    #[test]
    fn builds_combos_from_codes() {
        assert_eq!(combo(&[133, 50, 36]), "SUPER + SHIFT + RETURN");
        assert!(is_valid("SUPER + T"));
        assert!(!is_valid("SUPER + SHIFT"));
    }
}
