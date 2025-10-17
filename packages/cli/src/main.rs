fn main() {
    if let Err(error) = platynui_cli::run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
