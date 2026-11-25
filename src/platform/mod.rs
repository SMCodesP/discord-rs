//! Platform integration helpers.
//!
//! The clipboard module centralises text/bitmap access and provides an explicit
//! Linux backend selection (Wayland, X11) with size limits and injectable
//! implementations for tests.

pub mod clipboard;

pub use clipboard::*;
