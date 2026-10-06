# Funciones compartidas por install.sh y uninstall.sh. Se cargan con `source`.

MENU_EXT="$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
HYPR_DIR="$HOME/.config/hypr"
HYPR_MAIN="$HYPR_DIR/hyprland.lua"
BIN_DIR="$HOME/.local/bin"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

step() { echo -e "\033[1;34m::\033[0m $*"; }
ok() { echo -e "   \033[32m✔\033[0m $*"; }
warn() { echo -e "   \033[33m!\033[0m $*"; }

# menu_add <clave> <entrada JSONC>: inserta la entrada antes de la última
# llave del objeto, con copia de seguridad. No hace nada si la clave ya existe.
menu_add() {
  local key=$1 entry=$2
  mkdir -p "$(dirname "$MENU_EXT")"
  if [[ ! -f $MENU_EXT ]]; then
    printf '{\n%s\n}\n' "$entry" >"$MENU_EXT"
    return
  fi
  grep -q "\"$key\"" "$MENU_EXT" && return
  cp "$MENU_EXT" "$MENU_EXT.bak.$(date +%s)"
  awk -v entry="$entry" '
    { lines[NR] = $0 }
    END {
      last = 0
      for (i = NR; i > 0; i--) if (lines[i] ~ /}[[:space:]]*$/) { last = i; break }
      # ¿Hay alguna entrada antes (algo distinto de comentarios y "{")?
      prev = 0
      for (i = last - 1; i > 0; i--) {
        l = lines[i]; gsub(/^[[:space:]]+|[[:space:]]+$/, "", l)
        if (l == "" || l ~ /^\/\//) continue
        prev = i; break
      }
      for (i = 1; i <= NR; i++) {
        if (i == prev) {
          l = lines[i]; sub(/[[:space:]]+$/, "", l)
          if (l !~ /[{,]$/) lines[i] = l ","
        }
        if (i == last) {
          sub(/}[[:space:]]*$/, "", lines[i])
          if (lines[i] !~ /^[[:space:]]*$/) print lines[i]
          print entry
          print "}"
        } else {
          print lines[i]
        }
      }
    }' "$MENU_EXT" >"$MENU_EXT.tmp"
  mv "$MENU_EXT.tmp" "$MENU_EXT"
}

# menu_remove <clave>: borra el bloque de la entrada (desde su clave hasta la
# llave que lo cierra) y la coma que pudiera quedar antes del cierre.
menu_remove() {
  local key=$1
  [[ -f $MENU_EXT ]] && grep -q "\"$key\"" "$MENU_EXT" || return 0
  cp "$MENU_EXT" "$MENU_EXT.bak.$(date +%s)"
  awk -v key="\"$key\"" '
    { lines[NR] = $0 }
    END {
      skip = 0; n = 0
      for (i = 1; i <= NR; i++) {
        if (!skip && index(lines[i], key)) { skip = 1 }
        if (skip) { if (lines[i] ~ /^[[:space:]]*},?[[:space:]]*$/) skip = 0; continue }
        out[++n] = lines[i]
      }
      for (i = n; i > 0; i--) if (out[i] ~ /^[[:space:]]*}[[:space:]]*$/) { last = i; break }
      for (i = last - 1; i > 0; i--) {
        l = out[i]; gsub(/^[[:space:]]+|[[:space:]]+$/, "", l)
        if (l == "" || l ~ /^\/\//) continue
        sub(/,[[:space:]]*$/, "", out[i]); break
      }
      for (i = 1; i <= n; i++) print out[i]
    }' "$MENU_EXT" >"$MENU_EXT.tmp"
  mv "$MENU_EXT.tmp" "$MENU_EXT"
}

# hypr_require_remove <módulo> <comentario>: quita `require("<módulo>")` y su
# comentario de hyprland.lua, y las líneas en blanco que queden al final.
hypr_require_remove() {
  local module=$1 comment=$2
  [[ -f $HYPR_MAIN ]] || return 0
  local esc_module=${module//./\\.}
  sed -i "/^$comment\$/d; /^require(\"$esc_module\")\$/d" "$HYPR_MAIN"
  sed -i -e :a -e '/^\n*$/{$d;N;ba' -e '}' "$HYPR_MAIN"
}

hypr_reload() {
  if command -v hyprctl &>/dev/null && [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
    hyprctl reload &>/dev/null || true
    local errors
    errors="$(hyprctl configerrors 2>/dev/null || true)"
    if [[ -n ${errors//[[:space:]]/} ]]; then
      warn "Hyprland informa errores de configuración:"
      echo "$errors"
    fi
  fi
}

menu_refresh() {
  command -v omarchy &>/dev/null && omarchy menu refresh &>/dev/null || true
}

# Restos de cuando Widgets se llamaba meca-qs.
remove_legacy_meca_qs() {
  rm -f "$BIN_DIR/meca-qs" "$APP_DIR/meca-qs.desktop" "$HYPR_DIR/meca-qs.lua"
  hypr_require_remove "hypr.meca-qs" "-- meca-qs (Quickshell): ventana flotante"
  menu_remove "setup.config.quickshell"
}

# install_app <binario> <carpeta del crate>: copia el binario ya compilado,
# registra su .desktop y crea su regla de ventana flotante y centrada.
install_app() {
  local bin=$1 crate=$2
  local app_id="org.omarchy.$bin"
  mkdir -p "$BIN_DIR" "$APP_DIR" "$HYPR_DIR"
  install -m755 "$SCRIPT_DIR/target/release/$bin" "$BIN_DIR/$bin"
  install -m644 "$SCRIPT_DIR/crates/$crate/$bin.desktop" "$APP_DIR/$bin.desktop"
  cat >"$HYPR_DIR/$bin.lua" <<LUA
-- Gestionado por Lizarbe Ajustes (install.sh): ventana flotante y centrada.
if o and o.window then
  o.window("$app_id", { tag = "-floating-window", float = true, center = true, size = { 1120, 760 } })
end
LUA
  if [[ -f $HYPR_MAIN ]] && ! grep -q "require(\"hypr.$bin\")" "$HYPR_MAIN"; then
    printf '\n-- %s: ventana flotante\nrequire("hypr.%s")\n' "$bin" "$bin" >>"$HYPR_MAIN"
  fi
}

# uninstall_app <binario>: lo contrario de install_app.
uninstall_app() {
  local bin=$1
  rm -f "$BIN_DIR/$bin" "$APP_DIR/$bin.desktop" "$HYPR_DIR/$bin.lua"
  hypr_require_remove "hypr.$bin" "-- $bin: ventana flotante"
}

# menu_set <clave> <entrada JSONC>: añade o reemplaza la entrada.
menu_set() {
  menu_remove "$1"
  menu_add "$1" "$2"
}

# Entradas del menú de Omarchy que abren Escritorio. Las que reutilizan un id
# nativo (style.hyprland, setup.monitors…) reemplazan al editor de texto; las
# demás son nuevas.
ESCRITORIO_MENU_KEYS=(style.hyprland setup.monitors setup.keybindings setup.input setup.nightlight setup.compose setup.escritorio)

escritorio_entry() { # clave icono etiqueta sección
  local sec=""
  [[ -n $4 ]] && sec=" --section $4"
  printf '  "%s": {\n    "icon": "%s",\n    "label": "%s",\n    "action": "omarchy-launch-tui --app-id=org.omarchy.lizarbe-escritorio lizarbe-escritorio%s"\n  }' "$1" "$2" "$3" "$sec"
}

menu_install_escritorio() {
  menu_set style.hyprland "$(escritorio_entry style.hyprland "" "Apariencia de ventanas" apariencia)"
  menu_set setup.monitors "$(escritorio_entry setup.monitors "󰍹" "Pantallas" pantallas)"
  menu_set setup.keybindings "$(escritorio_entry setup.keybindings "" "Atajos de teclado" atajos)"
  menu_set setup.input "$(escritorio_entry setup.input "" "Teclado y mouse" teclado)"
  menu_set setup.nightlight "$(escritorio_entry setup.nightlight "󰖔" "Luz nocturna" luz)"
  menu_set setup.compose "$(escritorio_entry setup.compose "󰗊" "Atajos de texto" texto)"
  menu_set setup.escritorio "$(escritorio_entry setup.escritorio "󰍹" "Escritorio" "")"
}

menu_remove_escritorio() {
  local k
  for k in "${ESCRITORIO_MENU_KEYS[@]}"; do menu_remove "$k"; done
}
