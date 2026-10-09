//! El celular con la app de KDE Connect, dibujado con caracteres. El lado
//! derecho y la base llevan sombra para darle volumen.

/// Ancho del celular sin sombra.
pub const ANCHO: u16 = 32;
/// Alto del celular sin la sombra de la base.
pub const ALTO: u16 = 27;
/// Hora de la barra de estado: se sustituye por la real.
pub const HORA: &str = "00:00";

pub const CELULAR: [&str; 27] = [
    "╭──────────────────────────────╮",
    "│ 00:00          ▂▄▆  ▮▮▮▯     │",
    "│                              │",
    "│ ≡  lizarbe                 ⋮ │",
    "│                              │",
    "│ ╭────────────╮╭────────────╮ │",
    "│ │ ▤          ││ ▣          │ │",
    "│ │ Enviar     ││ Enviar al  │ │",
    "│ │ archivos   ││ portapap.  │ │",
    "│ ╰────────────╯╰────────────╯ │",
    "│ ╭────────────╮╭────────────╮ │",
    "│ │ ▲          ││ ▶          │ │",
    "│ │ Controles  ││ Control    │ │",
    "│ │ presentac. ││ multimedia │ │",
    "│ ╰────────────╯╰────────────╯ │",
    "│ ╭────────────╮╭────────────╮ │",
    "│ │ ◎          ││ ›          │ │",
    "│ │ Entrada    ││ Ejecutar   │ │",
    "│ │ remota     ││ orden      │ │",
    "│ ╰────────────╯╰────────────╯ │",
    "│                              │",
    "│ Algunos complementos         │",
    "│ necesitan permisos…          │",
    "│                              │",
    "├──────────────────────────────┤",
    "│     │││       ○       ‹      │",
    "╰──────────────────────────────╯",
];

/// Mosaico «Enviar archivos»: primera fila, filas, columna y ancho.
pub const MOSAICO: (usize, usize, usize, usize) = (5, 5, 2, 14);
