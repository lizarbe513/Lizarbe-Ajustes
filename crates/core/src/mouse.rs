//! Ratón: buscar la zona bajo el puntero, distinguir el doble clic y
//! traducir los eventos de crossterm a lo que las aplicaciones atienden.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use serde_json::Value;

use crate::field;
use crate::form::{FieldRow, Row};
use crate::view::{CoreHit, Sub};

/// Zona bajo `(x, y)`; gana la última registrada (la que está encima).
pub fn hit_at<H: Copy>(hits: &[(Rect, H)], x: u16, y: u16) -> Option<H> {
    let p = Position { x, y };
    hits.iter()
        .rev()
        .find(|(r, _)| r.contains(p))
        .map(|(_, h)| *h)
}

/// Rectángulo donde se dibujó `hit`.
pub fn hit_rect<H: Copy + PartialEq>(hits: &[(Rect, H)], hit: H) -> Option<Rect> {
    hits.iter().rev().find(|(_, h)| *h == hit).map(|(r, _)| *r)
}

/// Recuerda el último clic para reconocer el doble clic.
#[derive(Debug)]
pub struct Clicks<H> {
    last: Option<(Instant, H)>,
}

impl<H> Default for Clicks<H> {
    fn default() -> Self {
        Clicks { last: None }
    }
}

impl<H: Copy + PartialEq> Clicks<H> {
    const DOUBLE: Duration = Duration::from_millis(400);

    /// Anota un clic sobre `hit`; devuelve si es doble.
    pub fn press(&mut self, hit: Option<H>) -> bool {
        let double = self
            .last
            .is_some_and(|(at, h)| Some(h) == hit && at.elapsed() < Self::DOUBLE);
        self.last = hit.map(|h| (Instant::now(), h));
        double
    }
}

/// Lo que significa un evento de ratón para una aplicación.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mouse<H> {
    Hover(Option<H>),
    Scroll {
        down: bool,
        hit: Option<H>,
    },
    Click {
        hit: Option<H>,
        double: bool,
        x: u16,
    },
    /// Clic derecho: menú contextual en `at`.
    Menu {
        hit: Option<H>,
        at: (u16, u16),
    },
    Drag {
        hit: Option<H>,
        x: u16,
    },
    Release {
        hit: Option<H>,
    },
    Other,
}

pub fn read<H: Copy + PartialEq>(
    m: MouseEvent,
    hits: &[(Rect, H)],
    clicks: &mut Clicks<H>,
) -> Mouse<H> {
    let hit = hit_at(hits, m.column, m.row);
    match m.kind {
        MouseEventKind::Moved => Mouse::Hover(hit),
        MouseEventKind::ScrollUp => Mouse::Scroll { down: false, hit },
        MouseEventKind::ScrollDown => Mouse::Scroll { down: true, hit },
        MouseEventKind::Down(MouseButton::Left) => Mouse::Click {
            hit,
            double: clicks.press(hit),
            x: m.column,
        },
        MouseEventKind::Down(MouseButton::Right) => Mouse::Menu {
            hit,
            at: (m.column, m.row),
        },
        MouseEventKind::Drag(MouseButton::Left) => Mouse::Drag { hit, x: m.column },
        MouseEventKind::Up(MouseButton::Left) => Mouse::Release { hit },
        _ => Mouse::Other,
    }
}

/// Valor de la fila `row` si el ratón está en su deslizador, en la columna `x`.
pub fn slider<'a, H: CoreHit, B, A>(
    hits: &[(Rect, H)],
    rows: &'a [Row<B, A>],
    row: usize,
    x: u16,
) -> Option<(&'a FieldRow<B>, Value)> {
    let track = hit_rect(hits, H::ctrl(row, Sub::Slider))?;
    let Some(Row::Field(f)) = rows.get(row) else {
        return None;
    };
    Some((f, field::slider_value(&f.def.kind, track, x)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topmost_zone_wins_and_double_clicks_are_detected() {
        let hits = vec![(Rect::new(0, 0, 10, 10), 1u8), (Rect::new(2, 2, 3, 3), 2u8)];
        assert_eq!(hit_at(&hits, 3, 3), Some(2));
        assert_eq!(hit_at(&hits, 0, 0), Some(1));
        assert_eq!(hit_at(&hits, 20, 0), None);
        assert_eq!(hit_rect(&hits, 2), Some(Rect::new(2, 2, 3, 3)));
        let mut c = Clicks::default();
        assert!(!c.press(Some(1u8)));
        assert!(c.press(Some(1)));
        assert!(!c.press(Some(2)));
    }
}
