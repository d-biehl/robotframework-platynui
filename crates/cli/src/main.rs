fn main() {
    if let Err(error) = platynui_cli::run() {
        // Tracing is initialized inside run() after argument parsing.
        // If we get here, the subscriber is active — use it.
        tracing::error!(%error, "CLI execution failed");
        std::process::exit(1);
    }
}
