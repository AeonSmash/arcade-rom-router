//! The Tauri command boundary.
//!
//! SPEC.md section 58 requires this surface to stay narrow and typed. There is
//! deliberately no generic `run_process`, `read_any_file`, or `write_any_file`:
//! the frontend names an intent, and the backend resolves every path itself.

pub mod controllers;
pub mod dats;
pub mod emulators;
pub mod favorites;
pub mod games;
pub mod hotkeys;
pub mod library;
pub mod media;
pub mod rom_roots;
pub mod save_states;
pub mod scan;
pub mod settings;
pub mod system;
