//! The two `ceil` branches must agree.
//!
//! `particle-core::ceil` is the crate's one `#[cfg(feature = "std")]`: a hosted
//! build lowers it to the `f32::ceil` intrinsic, a bare-metal build takes a
//! hand-rolled branch that needs no libm. A feature flag that changes an answer
//! rather than just an implementation would mean the ESP32 build and the game
//! animate differently, and the bare-metal check in `compat/` would be proving
//! something about code the game does not run.
//!
//! So this walks the whole domain roog actually uses -- a blast radius, in
//! quarter-tile steps, out past any map -- and requires bit-for-bit equality.

#[test]
fn the_portable_ceil_matches_the_intrinsic_over_a_blast_radius() {
    let mut r = 0.0_f32;
    while r <= 40.0 {
        assert_eq!(
            particle_core::ceil_portable(r),
            r.ceil(),
            "ceil({r}) differs between the hand-rolled and intrinsic branches"
        );
        r += 0.25;
    }
}

#[test]
fn they_agree_on_the_exact_integers_too() {
    // The branch most likely to be off by one: a value already whole. The
    // hand-rolled version must not round it up to the next tile, or every
    // secondary burst in the game lands 40 ms late.
    for i in 0..=40 {
        let x = i as f32;
        assert_eq!(particle_core::ceil_portable(x), x.ceil(), "ceil({x})");
    }
}

#[test]
fn whichever_branch_is_compiled_in_is_one_of_the_two() {
    // Guards against a third implementation creeping in under the cfg.
    for i in 0..=160 {
        let x = i as f32 / 4.0;
        assert_eq!(particle_core::ceil(x), x.ceil(), "ceil({x})");
    }
}
