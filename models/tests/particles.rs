//! The particle layer is pure decoration, but its bookkeeping (delays, frame
//! stepping, culling) still needs to behave so the engine's playback loop
//! terminates and draws the right glyph at the right time.

use crossterm::style::Color;
use models::particles::{BlastPalette, Particles};

#[test]
fn a_blip_is_born_then_culled() {
    let mut fx = Particles::new();
    fx.blip(3, 4, '*', Color::Red);
    assert!(fx.pending, "queuing a particle flags the batch as pending");
    assert!(fx.any_alive());

    // Visible immediately (no delay).
    assert_eq!(fx.live[0].current(), Some(('*', Color::Red)));

    // Age past its lifetime and it should be gone.
    fx.advance(1_000.0);
    assert!(!fx.any_alive(), "an expired particle is culled");
}

#[test]
fn beam_cells_light_up_in_order() {
    let mut fx = Particles::new();
    let line: Vec<(u16, u16)> = (1..=6).map(|x| (x, 5)).collect();
    fx.beam(&line, Color::Cyan);
    assert_eq!(fx.live.len(), 6);

    // One short frame in: the head has started, the tail is still waiting out
    // its travel delay.
    fx.advance(10.0);
    assert!(fx.live[0].current().is_some(), "beam head is drawing");
    assert!(
        fx.live[5].current().is_none(),
        "beam tail has not arrived yet"
    );

    // A horizontal run draws with the horizontal beam glyph.
    assert_eq!(fx.live[0].current().unwrap().0, '-');
}

#[test]
fn an_explosion_ripples_out_and_then_ends() {
    let mut fx = Particles::new();
    let cells = [(10, 10, 0.0), (11, 10, 1.0), (13, 10, 3.0)];
    fx.explosion(&cells, BlastPalette::Fire);

    // Core is lit before the rim.
    fx.advance(10.0);
    assert!(fx.live[0].current().is_some());
    assert!(
        fx.live[2].current().is_none(),
        "the far cell waits for the ripple to reach it"
    );

    // Everything clears within a beat.
    fx.advance(2_000.0);
    assert!(!fx.any_alive());

    fx.clear();
    assert!(!fx.pending);
}
