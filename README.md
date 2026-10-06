# meca-qs — Quickshell para Omarchy

> Panel TUI para configurar la barra, los widgets, los plugins y la apariencia del
> shell **Quickshell** de [Omarchy](https://omarchy.org), sin editar JSON a mano.
> Aparece en el menú como **Quickshell**: `Menu → Setup → Config → Quickshell`.

*[English below](#english)*

---

## ✨ Qué puedes hacer

| Sección | Qué configura |
| :--- | :--- |
| **Barra** | Posición (arriba/abajo/izquierda/derecha), transparencia, widget anclado al centro y, en modo avanzado, una barra completa alternativa. |
| **Widgets** | Vista previa de la barra; añadir, quitar y mover widgets entre izquierda/centro/derecha (teclado o arrastrando con el ratón); ajustes de cada widget (formatos del reloj con vista previa en vivo, indicadores, clima, espaciador…). En modo avanzado: módulos personalizados de **comando** o **QML** y claves crudas. |
| **Plugins** | Activar/desactivar servicios, paneles y widgets; instalar desde git, personalizar (clonar) plugins de Omarchy, actualizar y eliminar. |
| **Inactividad** | Tiempo hasta el salvapantallas y hasta el bloqueo, con valores sugeridos. |
| **Apariencia** | Tamaño de letra, escala, tamaño y opacidad de la barra, menús y notificaciones. En modo avanzado: **todas** las claves del `shell.toml` del tema (colores con muestra). |
| **Cambios** | Lista legible de lo que vas a cambiar (antes → después) y herramientas avanzadas. |

Al pie de cada pantalla hay tres **botones** que se pulsan con el ratón o con el teclado
(`Tab` hasta los botones, `←→` y `Enter`):

| Botón | Atajo | Qué hace |
| :--- | :---: | :--- |
| **Aplicar** | `a` | Guarda los cambios pendientes (con copia de seguridad); el shell los usa al instante. |
| **Cancelar** | `c` | Descarta los cambios pendientes y vuelve a lo que está guardado. |
| **Restaurar** | `R` | Devuelve **la pantalla actual** a los valores de fábrica de Omarchy (la barra, los widgets, un widget concreto, los plugins, la inactividad o la apariencia; en *Cambios*, todo). No guarda nada: queda pendiente para que lo revises y pulses Aplicar o Cancelar. |

- **Modo simple y avanzado** (`m`): lo esencial con descripciones, o todas las opciones.
- **Español e inglés** (`i`), detectado automáticamente según `$LANG`.
- **Cambios por lotes y seguros**: nada se escribe hasta pulsar `a`. Antes de guardar se hace
  una copia de seguridad en `~/.local/state/meca-qs/backups/` y se escribe de forma atómica.
  El shell recarga la configuración al instante.
- Si mueves widgets directamente en la barra mientras la herramienta está abierta, se detecta
  y se recarga; si tenías cambios pendientes, te pregunta qué hacer.
- Colores tomados del tema activo de Omarchy.

## 🚀 Instalación

Requiere Rust (`omarchy pkg add rust` si no lo tienes).

```bash
git clone https://github.com/lizarbe513/omarchy-quickshell-config.git
cd omarchy-quickshell-config
./install.sh
```

El instalador (se puede volver a ejecutar para actualizar):

1. compila y copia `meca-qs` a `~/.local/bin/`,
2. registra la aplicación **Quickshell** (`~/.local/share/applications/meca-qs.desktop`),
3. añade `setup.config.quickshell` a `~/.config/omarchy/extensions/omarchy-menu.jsonc` (con copia de seguridad),
4. crea `~/.config/hypr/meca-qs.lua` (ventana flotante y centrada) y lo carga desde `hyprland.lua`.

Desinstalar: `./uninstall.sh` (no toca tu configuración de Omarchy).

## 🕹️ Uso

```bash
meca-qs                 # o Menu → Setup → Config → Quickshell
meca-qs --advanced      # empezar en modo avanzado
meca-qs --lang en       # forzar idioma
meca-qs --config-dir /tmp/prueba   # probar sobre una copia (no habla con el shell)
```

| Tecla | Acción |
| :--- | :--- |
| `Tab` / `⇧+Tab` | Cambiar de zona: secciones → contenido → botones |
| `Esc` | Volver |
| `1`–`6` | Ir a una sección |
| `↑↓` / `j k` | Moverse |
| `← →` / `h l` | Cambiar el valor (alternar, opción siguiente, ± paso) |
| `Enter` / `Espacio` | Editar, elegir de una lista o activar |
| `r` | Volver al valor por defecto |
| `a` / `c` / `R` | Aplicar / cancelar los cambios pendientes / restaurar la pantalla actual |
| `m` | Modo simple ⇄ avanzado |
| `i` | Español ⇄ inglés |
| `?` | Ayuda |
| `q` | Salir |
| **Widgets:** `⇧+flechas` o `H J K L` | Mover el widget |
| **Widgets:** `n` / `d` | Añadir / quitar widget |
| **Plugins:** `Enter` `/` `n` `p` `u` `x` | Activar · buscar · instalar desde git · personalizar · actualizar · eliminar |

Ratón: clic para elegir, doble clic para abrir ajustes, arrastrar widgets entre columnas, rueda para desplazarse.

## 🗂️ Qué archivos toca

| Archivo | Uso |
| :--- | :--- |
| `~/.config/omarchy/shell.json` | Barra, widgets y sus ajustes, plugins, inactividad. |
| `~/.config/omarchy/shell.toml` | Apariencia (capa del usuario sobre el `shell.toml` del tema). |
| `~/.local/state/meca-qs/backups/` | Copias de seguridad (se guardan las 30 más recientes). |
| `~/.config/meca-qs/config.toml` | Preferencias de la herramienta (idioma, modo). |

Nunca modifica `/usr/share/omarchy/`. Instalar, clonar, actualizar o eliminar plugins se
delega a `omarchy plugin …`, que se ejecuta en la propia terminal para que veas su salida.

## 🧑‍💻 Desarrollo

```bash
cargo test            # pruebas (incluye leer todos los manifests reales de Omarchy)
cargo clippy --all-targets
cargo run -- --config-dir "$(mktemp -d)"   # sandbox vacío
```

Estructura: `src/omarchy/` (modelo de `shell.json`, catálogo de plugins, esquemas, `shell.toml`,
tema, IPC), `src/store.rs` (cambios pendientes, diff, aplicar con backup), `src/app.rs`
(estado y teclado/ratón), `src/ui/` (dibujo, formularios, ventanas emergentes),
`src/i18n/` (textos ES/EN).

---

## English

**meca-qs** is a TUI panel to configure the bar, widgets, plugins and appearance of
Omarchy's **Quickshell** shell. It shows up as **Quickshell** under
`Menu → Setup → Config`.

- **Bar**: position, transparency, center anchor, alternative full bar (advanced).
- **Widgets**: live bar preview; add, remove and move widgets (keyboard or mouse drag);
  per-widget settings, including clock formats with live preview; custom command/QML modules
  and raw keys in advanced mode.
- **Plugins**: enable/disable, install from git, clone (customize) built-ins, update, remove.
- **Idle**: screensaver and lock timeouts.
- **Appearance**: font size, spacing, bar size and opacities; every theme `shell.toml` key in advanced mode.
- **Changes**: readable before → after list.
- **Buttons** at the bottom of every screen (mouse or keyboard): **Apply** (`a`, saves with an
  automatic backup), **Cancel** (`c`, discards pending changes) and **Restore** (`R`, resets the
  current screen to Omarchy's factory values as a pending change you can apply or cancel).

Simple/advanced mode with `m`, Spanish/English with `i`, help with `?`.
Install with `./install.sh`, remove with `./uninstall.sh`. Run `meca-qs --help` for options.
