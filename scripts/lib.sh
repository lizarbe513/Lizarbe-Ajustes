# Funciones compartidas por install.sh y uninstall.sh (solo para desarrollo).
# El menú de Omarchy, las reglas de ventana y los restos de versiones anteriores
# los gestiona `lizarbe-doctor` (paquete lizarbe-menu); aquí no se tocan.

BIN_DIR="$HOME/.local/bin"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

step() { echo -e "\033[1;34m::\033[0m $*"; }
ok() { echo -e "   \033[32m✔\033[0m $*"; }
warn() { echo -e "   \033[33m!\033[0m $*"; }

# install_app <binario> <carpeta del crate>: copia el binario ya compilado y
# registra su .desktop.
install_app() {
  local bin=$1 crate=$2
  mkdir -p "$BIN_DIR" "$APP_DIR"
  install -m755 "$SCRIPT_DIR/target/release/$bin" "$BIN_DIR/$bin"
  install -m644 "$SCRIPT_DIR/crates/$crate/$bin.desktop" "$APP_DIR/$bin.desktop"
}

# uninstall_app <binario>: lo contrario de install_app.
uninstall_app() {
  local bin=$1
  rm -f "$BIN_DIR/$bin" "$APP_DIR/$bin.desktop"
}

# Menú y reglas de ventana: los sincroniza lizarbe-doctor si está instalado.
sync_integration() {
  if command -v lizarbe-doctor &>/dev/null; then
    lizarbe-doctor --fix --quiet || warn "lizarbe-doctor encontró algo que revisar (ejecútalo sin --quiet)"
  else
    warn "lizarbe-doctor no está instalado: instala el paquete lizarbe-menu para el menú y las reglas de ventana"
  fi
}
