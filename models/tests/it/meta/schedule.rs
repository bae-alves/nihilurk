//! The turn schedule's order, read off the real dependency graph.
//!
//! An `.after()` that is dropped or pointed at the wrong step compiles and
//! plays, and only shows up as a bug a turn later. `.githooks/pre-commit`
//! asks for a docs page when an edge changes; this is what checks the edge.
