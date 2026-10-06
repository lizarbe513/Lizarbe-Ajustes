# Lizarbe Ajustes — Escritorio y Widgets

> Configuración de [Omarchy](https://omarchy.org) por interfaz TUI, sin editar archivos de texto.
> Este repositorio es un *workspace* de Rust con dos aplicaciones y un núcleo compartido:
>
> - **Escritorio** (`lizarbe-escritorio`): Hyprland sin editar archivos. Apariencia (espacios,
>   bordes, transparencia, desenfoque, sombras, animaciones), ventanas y escritorios, pantallas
>   (resolución, Hz, escala, posición, giro), teclado (idioma, tecla Compose, repetición), mouse
>   y touchpad, cursor, atajos de teclado (con grabación de combinaciones), inicio automático, luz
>   nocturna, atajos de texto (`~/.XCompose`) y temas propios con editor de colores. Reemplaza a Meca HyprConfig.
> - **Widgets** (`lizarbe-widgets`): barra, widgets, plugins y apariencia del shell
>   **Quickshell**. En el menú: `Menu → Setup → Widgets y barra`.
> - `crates/core`: idioma, preferencias, tema, escritura segura, Hyprland y piezas de interfaz
>   comunes, para que las dos apps se usen igual.

## Escritorio

Guarda **solo lo que cambias** en `~/.config/hypr/escritorio.lua`, que se carga después de tus
archivos (`looknfeel.lua`, `input.lua`…) y antes de los *toggles* de Omarchy, así que el modo
«sin espacios» de Omarchy sigue funcionando. Al pulsar **Aplicar** hace una copia de seguridad,
escribe de forma atómica y recarga Hyprland; si Hyprland informa un error, deshace los cambios.
**Restaurar** devuelve una sección (o todo) a los valores de Omarchy.

```bash
lizarbe-escritorio                        # o búscalo como "Escritorio" en el lanzador
lizarbe-escritorio --section teclado      # apariencia, ventanas, pantallas, teclado, mouse, cursor, atajos, inicio, luz, texto, cambios
lizarbe-escritorio --config-dir /tmp/x    # probar sobre una carpeta (no recarga Hyprland)
```

`install.sh` conecta Escritorio al menú de Omarchy reutilizando los ids nativos (`style.hyprland`,
`setup.monitors`, `setup.keybindings`, `setup.input`), así que esas entradas dejan de abrir un editor de
texto; `uninstall.sh` las devuelve a como estaban.

El **tamaño de la interfaz** se guarda en `monitors.lua`, igual que el atajo de escala de Omarchy,
para que los dos sigan funcionando juntos. La **tecla Compose** reemplaza los distintos «arreglos de
Bloq Mayús»: elige qué tecla hace de Compose (o ninguna) y el resto de opciones del teclado no se toca.

Si quedan ajustes de Meca HyprConfig (`hyprland-gui.lua`, que se carga al final y pisa a Escritorio),
la app ofrece **importarlos** (también con `lizarbe-escritorio --migrate-meca`): guarda solo lo que
difiere de Omarchy, hace copia de seguridad, comprueba el resultado con Hyprland y deshace todo si falla.
`install.sh` retira Meca del menú, de `~/.local/bin` y de los hooks.

*[English below](#english)*

---

## ✨ Qué puedes hacer

| Sección | Qué configura |
| :--- | :--- |
| **Barra** | Posición (arriba/abajo/izquierda/derecha), transparencia, widget anclado al centro y, en modo avanzado, una barra completa alternativa. |
| **Widgets** | Vista previa de la barra; añadir, quitar y mover widgets entre izquierda/centro/derecha (teclado o arrastrando con el ratón); ajustes de cada widget (formatos del reloj con vista previa en vivo, indicadores, clima, espaciador…). En modo avanzado: módulos personalizados de **comando** o **QML** y claves crudas. |
| **Plugins** | Activar/desactivar servicios, paneles y widgets; instalar desde git, personalizar (clonar) plugins de Omarchy, actualizar y eliminar, con botones visibles en la propia pantalla. |
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
  una copia de seguridad en `~/.local/state/lizarbe/backups/widgets/` y se escribe de forma atómica.
  El shell recarga la configuración al instante.
- Si mueves widgets directamente en la barra mientras la herramienta está abierta, se detecta
  y se recarga; si tenías cambios pendientes, te pregunta qué hacer.
- Colores tomados del tema activo de Omarchy.

## 🚀 Instalación

Requiere Rust (`omarchy pkg add rust` si no lo tienes).

```bash
git clone https://github.com/lizarbe513/Lizarbe-Ajustes.git
cd Lizarbe-Ajustes
./install.sh
```

El instalador (se puede volver a ejecutar para actualizar):

1. compila y copia `lizarbe-widgets` a `~/.local/bin/`,
2. retira la instalación anterior de **meca-qs**, si la hay (binario, entrada del menú, regla de ventana),
3. registra la aplicación **Widgets** (`~/.local/share/applications/lizarbe-widgets.desktop`),
4. añade `setup.widgets` a `~/.config/omarchy/extensions/omarchy-menu.jsonc` (con copia de seguridad),
5. crea `~/.config/hypr/lizarbe-widgets.lua` (ventana flotante y centrada) y lo carga desde `hyprland.lua`.

Desinstalar: `./uninstall.sh` (no toca tu configuración de Omarchy).

## 🕹️ Uso

```bash
lizarbe-widgets                     # o Menu → Setup → Widgets y barra
lizarbe-widgets --section plugins   # abrir una sección: bar, widgets, plugins, idle, appearance, changes
lizarbe-widgets --advanced          # empezar en modo avanzado
lizarbe-widgets --lang en           # forzar idioma
lizarbe-widgets --config-dir /tmp/prueba   # probar sobre una copia (no habla con el shell)
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
| `o` | Menú contextual del elemento seleccionado |

### Ratón

La interfaz sigue el estilo de **Meca**: todo lo clicable reacciona al pasar el ratón y la barra
inferior explica qué hace cada elemento.

| Gesto | Qué hace |
| :--- | :--- |
| **Pasar el ratón** | Resalta filas, controles y botones, y muestra una explicación en la barra inferior. |
| **Clic** | Elige; sobre un control lo cambia directamente: interruptor `■`, botones `−`/`+`, deslizador `──●──` (clic o arrastre) y listas `▾`, que se abren pegadas al control. |
| **Doble clic** | Abre los ajustes de un widget o activa un plugin. |
| **Clic derecho** | Menú contextual con las acciones del elemento (también con la tecla `o`). |
| **Arrastrar** | Mueve widgets entre columnas; sobre un deslizador cambia el valor en vivo. |
| **Rueda** | Desplaza la lista; sobre el panel izquierdo cambia de sección. |

En el panel izquierdo, **Modo** e **Idioma** también se cambian con un clic. Las ventanas de
confirmación tienen botones en relieve que se pueden pulsar.

## 🗂️ Qué archivos toca

| Archivo | Uso |
| :--- | :--- |
| `~/.config/omarchy/shell.json` | Barra, widgets y sus ajustes, plugins, inactividad. |
| `~/.config/omarchy/shell.toml` | Apariencia (capa del usuario sobre el `shell.toml` del tema). |
| `~/.local/state/lizarbe/backups/widgets/` | Copias de seguridad (se guardan las 30 más recientes). |
| `~/.config/lizarbe/ajustes.toml` | Preferencias compartidas por las TUIs Lizarbe (idioma, modo). Se migran solas desde `~/.config/meca-qs/config.toml`. |

Nunca modifica `/usr/share/omarchy/`. Instalar, clonar, actualizar o eliminar plugins se
delega a `omarchy plugin …`, que se ejecuta en la propia terminal para que veas su salida.

## 🧑‍💻 Desarrollo

```bash
cargo test            # pruebas de todo el workspace (incluye leer los manifests reales de Omarchy)
cargo clippy --workspace --all-targets
cargo run -p lizarbe-widgets -- --config-dir "$(mktemp -d)"   # sandbox vacío
```

Estructura:

- `crates/core/` — núcleo compartido: idiomas (`i18n`), preferencias (`prefs`), paleta del tema
  (`theme`), escritura atómica con copias de seguridad y transacciones con *rollback* (`fsutil`),
  comandos externos (`ipc`), Hyprland vía `hyprctl` (`hypr`: valor efectivo de una opción, vista
  previa con `eval`, recarga con comprobación de errores, monitores, toggles de Omarchy), bucle de la
  terminal (`term`) y primitivas de dibujo (`ui`).
- `crates/widgets/` — `src/omarchy/` (modelo de `shell.json`, catálogo de plugins, esquemas,
  `shell.toml`, IPC con el shell), `src/store.rs` (cambios pendientes, diff, aplicar),
  `src/app.rs` (estado y teclado/ratón), `src/ui/` (dibujo, formularios, ventanas emergentes),
  `src/i18n/` (textos ES/EN).

---

## English

**Widgets** (`lizarbe-widgets`) is a TUI panel to configure the bar, widgets, plugins and
appearance of Omarchy's **Quickshell** shell. It shows up as **Widgets y barra** under
`Menu → Setup`. This repository is a Rust workspace with a shared core (`crates/core`) for the
Lizarbe settings TUIs.

- **Bar**: position, transparency, center anchor, alternative full bar (advanced).
- **Widgets**: live bar preview; add, remove and move widgets (keyboard or mouse drag);
  per-widget settings, including clock formats with live preview; custom command/QML modules
  and raw keys in advanced mode.
- **Plugins**: enable/disable, install from git, clone (customize) built-ins, update, remove — with on-screen buttons.
- **Idle**: screensaver and lock timeouts.
- **Appearance**: font size, spacing, bar size and opacities; every theme `shell.toml` key in advanced mode.
- **Changes**: readable before → after list.
- **Buttons** at the bottom of every screen (mouse or keyboard): **Apply** (`a`, saves with an
  automatic backup), **Cancel** (`c`, discards pending changes) and **Restore** (`R`, resets the
  current screen to Omarchy's factory values as a pending change you can apply or cancel).

Simple/advanced mode with `m`, Spanish/English with `i`, help with `?`.
The look follows **Meca**: everything clickable reacts on hover, the bottom bar explains what is under
the mouse, controls are clickable (switches, −/+ steppers, sliders, anchored dropdowns) and
right-click (or `o`) opens a context menu.
Open a section directly with `--section <bar|widgets|plugins|idle|appearance|changes>`.
Install with `./install.sh` (it also removes an old meca-qs install), remove with `./uninstall.sh`.
Run `lizarbe-widgets --help` for options.
