# Lizarbe Ajustes — Escritorio, Widgets, Estudio de temas, Tienda y Bienvenida

> Configuración de [Omarchy](https://omarchy.org) por interfaz TUI, sin editar archivos de texto.
> Este repositorio es un *workspace* de Rust con cinco aplicaciones y un núcleo compartido. Los
> usuarios lo reciben con el paquete `lizarbe-ajustes` del repositorio de Lizarbe y se actualiza
> con `omarchy update`.

| Aplicación | Binario | En el menú de Omarchy |
| :--- | :--- | :--- |
| **Escritorio** | `lizarbe-escritorio` | Configuración › Pantallas, Atajos de teclado, Teclado, Luz nocturna, Capturas, Atajos de texto, Idioma, Inicio automático · Apariencia › Hyprland |
| **Widgets** | `lizarbe-widgets` | Apariencia › Widgets y barra · Configuración › Plugins, Notificaciones |
| **Estudio de temas** | `lizarbe-temas` | Apariencia › Crear tema |
| **Tienda** | `lizarbe-tienda` | Instalar › Paquete y Instalar › AUR (misma pantalla; cambia solo la fuente) |
| **Bienvenida** | `lizarbe-bienvenida` | Aprender › Bienvenida (y sola, una vez, en el primer inicio de un usuario nuevo) |

*[English below](#english)*

---

## Escritorio

Hyprland sin editar archivos:

| Sección | Qué configura |
| :--- | :--- |
| **Apariencia** | Espacios, bordes, esquinas, transparencia, desenfoque, sombras y animaciones. |
| **Ventanas** | Distribución, escritorios fijos y gestos. |
| **Pantallas** | Resolución, Hz, escala, posición y giro de cada pantalla. |
| **Teclado** | Distribución, tecla Compose y repetición. |
| **Mouse y touchpad** · **Cursor** | Velocidad, desplazamiento natural, tema y tamaño del cursor. |
| **Atajos de teclado** | Cambiar, desactivar o añadir atajos grabando la combinación. |
| **Inicio automático** | Programas que se abren al entrar. |
| **Luz nocturna** | Horario y calidez del filtro de luz azul. |
| **Atajos de texto** | Secuencias de `~/.XCompose`. |
| **Idioma** | Idioma del sistema, del menú y de las aplicaciones. |
| **Capturas** | Carpeta, tecla de captura, qué se captura, guardar/copiar, editor y carpeta de grabaciones. |
| **Cambios** | Lista de lo que vas a cambiar antes de aplicarlo. |

Guarda **solo lo que cambias** en `~/.config/hypr/escritorio.lua`, que se carga después de tus
archivos (`looknfeel.lua`, `input.lua`…) y antes de los *toggles* de Omarchy. Al pulsar **Aplicar**
hace una copia de seguridad, escribe de forma atómica y recarga Hyprland; si Hyprland informa un
error, deshace los cambios.

- El **tamaño de la interfaz** se guarda en `monitors.lua`, igual que el atajo de escala de Omarchy.
- **Capturas** lleva el modo, el procesado y el editor al comando de la tecla de captura, y la
  carpeta a `hl.env` (al instante) y a un bloque gestionado de `~/.config/uwsm/env` (para el resto
  de la sesión, desde el próximo inicio).
- Si quedan ajustes de Meca HyprConfig, la app ofrece **importarlos** (`--migrate-meca`).

```bash
lizarbe-escritorio --section capturas   # apariencia, ventanas, pantallas, teclado, mouse, cursor,
                                        # atajos, inicio, luz, texto, idioma, capturas, cambios
```

## Widgets

La barra y el shell **Quickshell**:

| Sección | Qué configura |
| :--- | :--- |
| **Barra** | Posición, transparencia, widget anclado al centro y (avanzado) una barra alternativa. |
| **Widgets** | Vista previa de la barra; añadir, quitar y mover widgets (teclado o arrastre); ajustes de cada widget. En avanzado: módulos de **comando** o **QML** y claves crudas. |
| **Plugins** | Activar/desactivar, instalar desde git, personalizar, actualizar y eliminar (vía `omarchy plugin`). |
| **Inactividad** | Tiempo hasta el salvapantallas y el bloqueo. |
| **Notificaciones** | **No molestar** y si **swaync** puede mostrar avisos. Con swaync bloqueado, todos los avisos salen con el estilo de Omarchy (Quickshell); si swaync arranca primero, algunos salen con otro aspecto. El tiempo en pantalla y el aspecto los fija Omarchy. |
| **Apariencia** | Tamaño de letra, escala, tamaños y opacidades; en avanzado, todas las claves del `shell.toml` del tema. |
| **Cambios** | Lista de lo que vas a cambiar. |

Si mueves widgets en la propia barra con la app abierta, se detecta y se recarga.

```bash
lizarbe-widgets --section plugins       # bar, widgets, plugins, idle, appearance, changes
```

## Bienvenida

Guía de primer inicio para quien llega de Windows: la terminal como interfaz, pero sin miedo. Es
minimalista y se maneja siempre igual: **Enter continúa**, **Espacio hace lo que pide el paso**
(probar, aplicar, abrir) y **←** vuelve; una frase bajo el título dice qué hacer ahora y se pone en
verde (✓) al lograrlo. El arranque muestra el isotipo con efecto de monitor CRT (encendido, rayas de
barrido, rejilla de píxeles, parpadeo). Recorre ocho pasos y termina con un resumen de logros:

1. **La tecla Super** y el «Super + algo = una orden».
2. **Práctica en vivo**: «Pulse Super + Espacio»… y Hyprland (`.socket2.sock`) cuenta que el usuario
   lo hizo de verdad (menú abierto, terminal nueva, ventana cerrada, cambio de escritorio, pantalla
   completa). Una línea «EN VIVO» enseña lo último que se detecta.
3. **La terminal no muerde**: una terminal de prueba donde solo se aceptan órdenes inofensivas
   (`fastfetch`, `lizarbe status`, `date`…), con sugerencia y `Tab`.
4. **Conectar el teléfono**: QR de la descarga de KDE Connect (probado: se decodifica con `zbarimg`),
   pasos, comprobación del cortafuegos y teléfonos detectados en vivo (`kdeconnect-cli`).
5. **Su escritorio, a su gusto**: cambiar el tema del sistema entero, con vuelta al de partida.
6. **Configurar sin editar archivos**: abre Escritorio, Widgets, Notificaciones, Capturas, Crear tema,
   la Tienda y el Centro.
7. **Conceptos clave** con un dibujo animado cada uno, y 8. **Atajos imprescindibles**.

Aparece sola una vez: el hook `60-lizarbe-bienvenida.sh` de `lizarbe-menu` la abre si existe
`~/.config/lizarbe/bienvenida-pendiente` (la siembran `/etc/skel` y la ISO) y la propia app quita la
marca al salir, salvo que se pida volver a verla. Siempre está en Aprender › Bienvenida.

```bash
lizarbe-bienvenida                  # el recorrido completo
lizarbe-bienvenida --capitulo 4     # empezar en un capítulo (0 = arranque, 9 = final)
lizarbe-bienvenida --demo           # sin tocar el sistema (se activa solo con --config-dir)
```

## Tienda

Instalar programas como en una tienda, sin saber el nombre del paquete: categorías, descripciones en
tu idioma, búsqueda por lo que quieres hacer ("música", "editar vídeo") y marcar varias apps para
instalarlas de una vez. Es la pantalla que se abre en Instalar › Paquete (repositorios oficiales) y en
Instalar › AUR (programas de la comunidad, que se compilan en el equipo): el menú de Omarchy no cambia,
solo lo que se abre. Las apps destacadas se muestran como tarjetas (logo a color, nombre, resumen y
etiquetas; el borde grueso marca la elegida). La tecla `a` («Buscar en todo») abre el buscador fzf de la
fuente en la que estás para llegar a cualquier paquete; si ya escribiste una búsqueda, se abre filtrado.

El catálogo es `crates/tienda/catalogo.toml` (se embebe en el binario). Para añadir una app basta una
entrada `[[app]]` (`fuente = "aur"` para las del AUR; por defecto son de los repos).
`cargo test -p lizarbe-tienda -- --ignored` comprueba, con internet, que sus paquetes existen en su fuente.

Teclas, como en Escritorio: el panel con el foco lleva el marcador `▍` en color de acento (en el otro,
atenuado). `↑↓` cambia de categoría; `Enter`, `Espacio` o `→` entran a la lista; `Esc` o `←` vuelven a las
categorías, y `Esc` desde ahí sale. `Tab` alterna de panel, `1`-`9` saltan a una categoría, `/` busca
(`Esc` borra la búsqueda), `Espacio` marca, `Enter` instala, `x` quita, `o` abre, `a` busca en todo.
Mientras corre un comando (la contraseña de sudo, la descarga) **`Esc` lo cancela y vuelve a la tienda**;
por eso `Ctrl+C` no interrumpe ahí, y una flecha o `Alt`+tecla escritas en la contraseña también cancelan.

```bash
lizarbe-tienda --fuente aur             # abrir el modo AUR (por defecto: repos)
lizarbe-tienda --categoria multimedia   # abrir en una categoría (o recomendadas, instaladas)
lizarbe-tienda --app obs                # abrir seleccionando una app
lizarbe-tienda --buscar "editar video"  # abrir con una búsqueda
```

## Estudio de temas

Crea y edita **temas completos** de Omarchy con una maqueta en vivo del escritorio:

| Pestaña | Qué edita |
| :--- | :--- |
| **1 Tema** | Nombre, modo claro u oscuro y tema base. |
| **2 Paleta** | Acento y selección; paleta generada desde el acento (`G`) o desde un fondo (`F`). |
| **3 Interfaz** | Fondos, textos y bordes de ventana (color o degradado con ángulo). |
| **4 Terminal** | Los 16 colores ANSI. |
| **5 Fondos** | Fondos de pantalla con miniatura. Quitar, ordenar y añadir con `D`, `[ ]`, `N`, el menú `M` o clic derecho; los del tema base se marcan y se pueden quitar de golpe. |
| **6 Iconos y apps** | Tema de iconos, Neovim, VS Code y teclado RGB. |
| **7 Barra y menús** | Colores propios de la barra, los menús, el lanzador y el bloqueo. |
| **8 Guardar** | Guardar, activar y **probar en el escritorio**. |

**Probar** (`P`) aplica el tema de verdad con `omarchy-theme-set`; si no lo confirmas en 20 s vuelve
solo al anterior. Mientras tanto se captura `preview.png` con `grim`. Los temas se guardan solo en
`~/.config/omarchy/themes/<nombre>`; nunca se toca `/usr/share/omarchy`.

```bash
lizarbe-temas --new tokyo-night         # empezar un tema a partir de otro
lizarbe-temas --theme mi-tema           # editar un tema propio
```

## Uso común

Las tres aplicaciones se manejan igual, con teclado o ratón:

| Tecla | Acción |
| :--- | :--- |
| `Tab` / `⇧+Tab` | Cambiar de zona (menú → contenido → botones) |
| `↑↓` / `j k` · `← →` / `h l` | Moverse · cambiar el valor |
| `Enter` / `Espacio` | Editar, elegir o activar |
| `r` | Volver al valor por defecto |
| `/` · `Ctrl+F` | **Buscar una opción** en todas las secciones y saltar a ella |
| `o` (Escritorio, Widgets) · `m` (Estudio) | Menú contextual (también con clic derecho) |
| `a` / `c` / `R` | Aplicar / cancelar / restaurar (Escritorio y Widgets) |
| `s` / `p` / `z` | Guardar / probar / deshacer (Estudio) |
| `m` / `i` | Modo simple ⇄ avanzado / español ⇄ inglés (Escritorio y Widgets) |
| `?` · `Esc` · `q` / `Ctrl+W` | Ayuda · volver · cerrar |

Con el ratón: al pasar por encima todo se resalta y el pie explica qué hace; clic para elegir o
cambiar un control (interruptores, `−`/`+`, deslizadores, listas `▾`); doble clic para abrir;
clic derecho para el menú contextual; arrastrar para mover widgets o deslizadores; la rueda
desplaza (sobre el menú izquierdo cambia de sección).

Comunes a todas: `--lang <es|en>`, `--config-dir <dir>` (modo de pruebas: no toca tu escritorio),
`--help` y `--version`. Los colores salen del tema activo de Omarchy y se actualizan solos si lo
cambias con la app abierta.

## Qué archivos toca

| Archivo | Quién | Uso |
| :--- | :--- | :--- |
| `~/.config/hypr/escritorio.lua` | Escritorio | Ajustes de Hyprland, atajos y capturas. |
| `~/.config/hypr/monitors.lua` | Escritorio | Escala general. |
| `~/.config/hypr/autostart.lua`, `~/.XCompose`, `~/.config/hypr/hyprsunset.conf` | Escritorio | Inicio, atajos de texto, luz nocturna. |
| `~/.config/uwsm/env` | Escritorio | Solo el bloque `# >>> lizarbe capturas`. |
| `~/.config/omarchy/shell.json`, `shell.toml` | Widgets | Barra, widgets, plugins, inactividad y apariencia. |
| `~/.local/state/omarchy/notifications.json` | Widgets | Solo la clave `dnd` (No molestar); el resto se conserva. |
| `~/.local/share/dbus-1/services/org.freedesktop.Notifications.service` | Widgets | Bloqueo de swaync (y `systemctl --user mask swaync.service`). Se quita al volver a permitirlo. |
| `~/.config/omarchy/themes/<tema>/` | Estudio | Temas propios. |
| `~/.config/lizarbe/ajustes.toml` | Todas | Preferencias compartidas (idioma, modo). |
| `~/.local/state/lizarbe/backups/` | Todas | Copias de seguridad antes de cada cambio. |

Nunca se modifica `/usr/share/omarchy/`.

## Desarrollo

```bash
./install.sh            # compila y copia las tres apps a ~/.local/bin (solo desarrollo)
./uninstall.sh          # las quita; no toca tu configuración
cargo test              # pruebas de todo el workspace
cargo clippy --workspace --all-targets
cargo run -p lizarbe-escritorio -- --config-dir "$(mktemp -d)"
cargo test -p lizarbe-escritorio dump_screens -- --ignored --nocapture   # pantallas como texto
```

Los paquetes se construyen y publican desde
[Lizarbe-Paquetes](https://github.com/lizarbe513/Lizarbe-Paquetes) a partir de las etiquetas `vX.Y.Z`.

### Estructura

- `crates/core/` — núcleo compartido:
  - `i18n`, `prefs`, `cli` (argumentos comunes), `paths` (rutas de Omarchy);
  - `theme` (paleta y recarga en vivo), `color`, `themes` (modelo de temas de Omarchy);
  - `fsutil` (escritura atómica, copias, transacciones), `ipc`, `hypr` (Hyprland vía `hyprctl`);
  - interfaz: `term` (bucle), `ui`, `view`, `form`, `field`, `popup`, `modal`, `mouse`, `hints`, `search`.
- `crates/escritorio/` — catálogo de opciones, `escritorio.lua`, atajos, capturas, inicio, luz nocturna…
- `crates/widgets/` — `shell.json`/`shell.toml`, plugins y la vista de la barra.
- `crates/temas/` — borrador del tema, maqueta, paletas (acento y k-means), imágenes y pruebas en vivo.
- `crates/tienda/` — catálogo curado (`catalogo.toml`), consultas a pacman y la interfaz de la tienda.
- `crates/bienvenida/` — capítulos, práctica en vivo (`retos`), QR, terminal de prueba, animaciones y marca.
  El núcleo aporta `hypr_events` (eventos de Hyprland), `ansi` (colores ANSI → ratatui) y `frame_interval`.

### Estilo visual

Minimalismo editorial: líneas finas, mucho aire y un solo acento. El estado se marca con `▍`, el
tono y el subrayado, nunca solo con el color. Controles planos (`━━●`, `‹ 6 ›`, `━━━●───`, `valor ▾`)
y botones de texto con su tecla (`[A] Aplicar  [Q] Cerrar`).

---

## English

Four TUI apps (three to configure [Omarchy](https://omarchy.org) without editing files, shipped as the
`lizarbe-ajustes` package and updated with `omarchy update`:

- **Escritorio** (`lizarbe-escritorio`): Hyprland — appearance, windows, monitors, keyboard,
  mouse, cursor, keybindings (recorded), autostart, night light, compose sequences, language and
  **screenshots** (folder, capture key, mode, save/copy, editor). Saves only what you change in
  `~/.config/hypr/escritorio.lua`, with backups and automatic rollback if Hyprland reports an error.
- **Widgets** (`lizarbe-widgets`): the Quickshell bar, widgets, plugins, idle, notifications (Do not disturb; keep swaync from mixing styles) and appearance.
- **Theme studio** (`lizarbe-temas`): create and edit complete Omarchy themes with a live mock-up,
  palettes generated from an accent or a wallpaper, wallpapers, icons, editors and bar colors, and
  a **try on the desktop** mode that reverts after 20 s unless confirmed.
- **Welcome** (`lizarbe-bienvenida`): a first-run tour with the Lizarbe identity. It practices the shortcuts
  live (Hyprland reports what you really did), shows a QR to install KDE Connect and lists nearby phones,
  offers a safe toy terminal, switches themes and opens the settings tools. It appears once on a new
  user's first login and lives in Learn › Welcome.
- **App Store** (`lizarbe-tienda`): browse and install apps by category with descriptions in your
  language, search by what you want to do, and mark several apps to install at once. It is the
  screen opened by Install › Package (official repos) and Install › AUR (community programs built on
  your computer); the Omarchy menu itself is unchanged, only what it opens. Featured apps are cards
  (colored logo, name, summary, tags; a thick border marks the selected one). Key `a` ("Search
  everything") opens the fuzzy finder for the current source to reach any package, pre-filtered with
  what you typed. While a command runs (sudo password, download) `Esc` cancels it and returns to the
  store. The curated catalog is `crates/tienda/catalogo.toml` (`icono` and `color` give each card its
  logo).

All four share the same keys and mouse behaviour: `/` or `Ctrl+F` searches every option and
jumps to it, right-click opens a context menu, `?` shows help. Common options: `--lang <es|en>`,
`--config-dir <dir>` (test mode), `--help`, `--version`. Development: `./install.sh`, `cargo test`.
