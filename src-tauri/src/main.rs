// Prevents an additional console window from appearing alongside release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    aeonic_arcadia_lib::run();
}
