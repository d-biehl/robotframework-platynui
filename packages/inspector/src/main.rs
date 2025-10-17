#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = platynui_inspector::run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
