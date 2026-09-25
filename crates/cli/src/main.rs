// Bin target sharing the package's dependency list; everything but the entry point lives in the lib.
#![allow(unused_crate_dependencies)]

use owo_colors::{OwoColorize, Stream};

fn main() {
    if let Err(error) = platynui_cli::run() {
        let prefix = "Error:".if_supports_color(Stream::Stderr, |t| t.red().bold().to_string()).to_string();
        eprintln!("{prefix} {error}");
        std::process::exit(1);
    }
}
