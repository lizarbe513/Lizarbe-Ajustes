//! Animación basada en el tiempo transcurrido (no en contar fotogramas, que
//! llegan de forma irregular). Todo son funciones puras de `t` en segundos.

/// Suavizado de 0 a 1 (arranca y termina despacio).
pub fn smooth(t: f32) -> f32 {
    let x = t.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Número pseudoaleatorio estable en [0, 1) para una celda.
pub fn hash2(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA6B) ^ 0xC2B2_AE35;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h % 10_000) as f32 / 10_000.0
}

/// Pulso suave entre 0 y 1 con el periodo dado (segundos).
pub fn pulso(t: f32, periodo: f32) -> f32 {
    0.5 - 0.5 * (t / periodo * std::f32::consts::TAU).cos()
}

/// Texto escrito a máquina: lo que se ve a los `t` segundos, a `cps` letras por segundo.
pub fn maquina(texto: &str, t: f32, cps: f32) -> String {
    let n = (t.max(0.0) * cps) as usize;
    texto.chars().take(n).collect()
}

/// Carácter de la trama según el avance de una celda (0 = vacío, 1 = completo).
pub fn trama(avance: f32, final_: char) -> char {
    if final_ == ' ' || avance <= 0.0 {
        ' '
    } else if avance < 0.25 {
        '░'
    } else if avance < 0.5 {
        '▒'
    } else if avance < 0.75 {
        '▓'
    } else {
        final_
    }
}

/// Avance de una celda (x, y) cuando el avance global es `global`: cada celda
/// arranca en un instante distinto (efecto «ruido que se aclara»).
pub fn avance_celda(global: f32, x: u32, y: u32) -> f32 {
    let inicio = hash2(x, y) * 0.6;
    ((global - inicio) / 0.4).clamp(0.0, 1.0)
}

/// Barra de progreso con tramas: `frac` de 0 a 1 en `ancho` celdas.
pub fn barra(frac: f32, ancho: usize) -> String {
    let total = frac.clamp(0.0, 1.0) * ancho as f32;
    let llenas = total.floor() as usize;
    let resto = total - llenas as f32;
    let mut s = "█".repeat(llenas.min(ancho));
    if llenas < ancho {
        s.push(match resto {
            r if r > 0.66 => '▓',
            r if r > 0.33 => '▒',
            r if r > 0.0 => '░',
            _ => '░',
        });
        s.push_str(&"░".repeat(ancho - llenas - 1));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typewriter_reveals_progressively() {
        assert_eq!(maquina("hola", 0.0, 10.0), "");
        assert_eq!(maquina("hola", 0.25, 10.0), "ho");
        assert_eq!(maquina("hola", 9.0, 10.0), "hola");
    }

    #[test]
    fn cells_resolve_through_the_shades() {
        assert_eq!(trama(0.0, '█'), ' ');
        assert_eq!(trama(0.1, '█'), '░');
        assert_eq!(trama(0.4, '█'), '▒');
        assert_eq!(trama(0.6, '█'), '▓');
        assert_eq!(trama(1.0, '▀'), '▀');
        assert_eq!(trama(1.0, ' '), ' ');
    }

    #[test]
    fn global_progress_completes_every_cell() {
        for x in 0..30 {
            for y in 0..10 {
                assert_eq!(avance_celda(1.0, x, y), 1.0);
                assert_eq!(avance_celda(0.0, x, y), 0.0);
                let h = hash2(x, y);
                assert!((0.0..1.0).contains(&h));
            }
        }
    }

    #[test]
    fn progress_bar_has_the_requested_width() {
        for frac in [0.0, 0.3, 0.55, 1.0] {
            assert_eq!(barra(frac, 10).chars().count(), 10, "{frac}");
        }
        assert_eq!(barra(1.0, 4), "████");
        assert_eq!(barra(0.0, 4), "░░░░");
    }

    #[test]
    fn pulse_stays_in_range() {
        for i in 0..50 {
            let p = pulso(i as f32 * 0.07, 1.2);
            assert!((0.0..=1.0).contains(&p));
        }
    }
}
