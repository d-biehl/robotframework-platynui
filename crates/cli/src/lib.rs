// `platynui_link_providers!` links the real OS platform/provider crates only outside tests and
// the `mock-provider` feature; referencing them here would register them in the test binaries.
#![cfg_attr(any(test, feature = "mock-provider"), allow(unused_crate_dependencies))]

mod commands;
#[cfg(test)]
mod test_support;
mod util;

#[cfg(any(test, feature = "mock-provider"))]
use platynui_platform_mock as _;
#[cfg(any(test, feature = "mock-provider"))]
use platynui_provider_mock as _;

use clap::{Parser, Subcommand, ValueEnum};
use commands::{
    focus::{self, FocusArgs},
    highlight::{self, HighlightArgs},
    info,
    keyboard::{self, KeyboardArgs},
    list_providers,
    pointer::{self, PointerArgs},
    query::{self, QueryArgs},
    screenshot::{self, ScreenshotArgs},
    snapshot::{self, SnapshotArgs},
    watch::{self, WatchArgs},
    window::{self, WindowArgs},
};
use platynui_link::platynui_link_providers;
use platynui_log_filter::LevelFilter;
use platynui_runtime::Runtime;
use util::{CliResult, map_provider_error};

// Link real platform providers in non-test builds so the CLI binary brings the
// OS integrations. Tests link the mock providers explicitly in their modules.
platynui_link_providers!();

#[derive(Parser)]
#[command(author, version, about = "PlatynUI command line interface", long_about = None)]
struct Cli {
    #[arg(
        long = "log-level",
        global = true,
        value_name = "LEVEL",
        value_parser = platynui_log_filter::parse_level,
        help = "Level of the diagnostics on stderr: off, error, warn (the default), info, debug or trace",
        long_help = "Level of the diagnostics on stderr: off, error, warn (the default), info, debug or \
                     trace, in any case; warning, critical and fatal are accepted as well. warn to trace \
                     apply to PlatynUI's own modules, while other crates stay at warn; error and off apply \
                     to every module. Overrides the PLATYNUI_LOG_LEVEL environment variable, which takes \
                     the same names. RUST_LOG overrides both; it takes filter directives and is the only \
                     way to see more from third-party crates."
    )]
    log_level: Option<LevelFilter>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "list-providers", about = "List registered providers (name, version, active status).")]
    ListProviders {
        #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    #[command(name = "info", about = "Show desktop and platform metadata (OS, monitors, bounds).")]
    Info {
        #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    #[command(name = "query", about = "Evaluate XPath expressions, output as table or JSON.")]
    Query(QueryArgs),
    #[command(name = "snapshot", about = "Export UI subtrees as text or XML.")]
    Snapshot(SnapshotArgs),
    #[command(name = "watch", about = "Stream provider events, with an optional XPath follow-up query.")]
    Watch(WatchArgs),
    #[command(name = "highlight", about = "Highlight elements matching an XPath expression.")]
    Highlight(HighlightArgs),
    #[command(name = "screenshot", about = "Capture a screenshot and save it as PNG (--rect for a sub-region).")]
    Screenshot(ScreenshotArgs),
    #[command(name = "focus", about = "Set focus on nodes selected by an XPath expression.")]
    Focus(FocusArgs),
    #[command(
        name = "window",
        about = "List or control windows (activate, bring-to-front, minimize, maximize, restore, close, move, resize)."
    )]
    Window(WindowArgs),
    #[command(
        name = "pointer",
        about = "Control the pointer (move, click, multi-click, press, release, scroll, drag, position)."
    )]
    Pointer(Box<PointerArgs>),
    #[command(name = "keyboard", about = "Send keyboard input (type, press, release, list).")]
    Keyboard(KeyboardArgs),
    #[command(name = "element-at-point", about = "Resolve the UI element at a screen point (hit-test).")]
    ElementAtPoint {
        /// Screen X coordinate.
        x: f64,
        /// Screen Y coordinate.
        y: f64,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Execute the `PlatynUI` CLI using command-line arguments from the environment.
///
/// # Errors
///
/// Returns an error if the runtime cannot be initialized or the selected subcommand fails.
pub fn run() -> CliResult<()> {
    let cli = Cli::parse();

    platynui_log_filter::init_stderr(cli.log_level);

    let mut runtime = Runtime::new().map_err(map_provider_error)?;

    match cli.command {
        Commands::ListProviders { format } => {
            let output = list_providers::run(&runtime, format)?;
            println!("{output}");
        }
        Commands::Info { format } => {
            let output = info::run(&runtime, format)?;
            println!("{output}");
        }
        Commands::Query(args) => {
            let output = query::run(&runtime, &args)?;
            println!("{output}");
        }
        Commands::Snapshot(args) => {
            let output = snapshot::run(&runtime, &args)?;
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Commands::Watch(args) => watch::run(&mut runtime, &args)?,
        Commands::Highlight(args) => {
            let output = highlight::run(&runtime, &args)?;
            println!("{output}");
        }
        Commands::Screenshot(args) => {
            let output = screenshot::run(&runtime, &args)?;
            println!("{output}");
        }
        Commands::Focus(args) => {
            let output = focus::run(&runtime, &args)?;
            println!("{output}");
        }
        Commands::Window(args) => {
            let output = window::run(&runtime, &args)?;
            println!("{output}");
        }
        Commands::Pointer(args) => {
            let output = pointer::run(&runtime, &args)?;
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Commands::Keyboard(args) => {
            let output = keyboard::run(&runtime, &args)?;
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Commands::ElementAtPoint { x, y } => {
            match runtime.element_at_point(platynui_core::types::Point::new(x, y)).map_err(map_provider_error)? {
                Some(node) => {
                    println!(
                        "Resolved: {} '{}' id={:?} runtime_id={}",
                        node.role(),
                        node.name(),
                        node.id(),
                        node.runtime_id().as_str()
                    );
                    let mut current = node.parent().and_then(|weak| weak.upgrade());
                    while let Some(ancestor) = current {
                        println!("  ^ {} '{}'", ancestor.role(), ancestor.name());
                        current = ancestor.parent().and_then(|weak| weak.upgrade());
                    }
                }
                None => println!("No element at ({x}, {y})"),
            }
        }
    }

    runtime.shutdown();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::types::Point;

    #[test]
    fn clap_parsing_defaults_to_text() {
        let cli = Cli::try_parse_from(["platynui", "list-providers"]).expect("parse");
        match cli.command {
            Commands::ListProviders { format } => assert!(matches!(format, OutputFormat::Text)),
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn clap_parsing_info_defaults_to_text() {
        let cli = Cli::try_parse_from(["platynui", "info"]).expect("parse");
        match cli.command {
            Commands::Info { format } => assert!(matches!(format, OutputFormat::Text)),
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn log_level_is_optional_and_takes_the_shared_level_names() {
        assert_eq!(Cli::try_parse_from(["platynui", "info"]).expect("parse").log_level, None);
        let parsed =
            |value: &str| Cli::try_parse_from(["platynui", "--log-level", value, "info"]).expect(value).log_level;
        assert_eq!(parsed("WARNING"), Some(LevelFilter::WARN));
        assert_eq!(parsed("Debug"), Some(LevelFilter::DEBUG));
        assert_eq!(parsed("off"), Some(LevelFilter::OFF));
        assert_eq!(parsed("critical"), Some(LevelFilter::ERROR));
        let after_the_command = Cli::try_parse_from(["platynui", "info", "--log-level", "trace"]).expect("global");
        assert_eq!(after_the_command.log_level, Some(LevelFilter::TRACE));
    }

    #[test]
    fn an_unknown_log_level_exits_with_status_2_and_names_the_levels() {
        let Err(err) = Cli::try_parse_from(["platynui", "--log-level", "verbose", "info"]) else {
            panic!("verbose must be rejected");
        };
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
        assert_eq!(err.exit_code(), 2);
        let message = err.to_string();
        assert!(
            message.contains(
                "unknown log level 'verbose'; expected one of off, error, warn (warning), info, debug, trace, \
                 critical, fatal"
            ),
            "{message}"
        );
    }

    #[test]
    fn clap_parsing_focus_requires_expression() {
        let cli = Cli::try_parse_from(["platynui", "focus", "//control:Button"]).expect("parse");
        match cli.command {
            Commands::Focus(args) => {
                assert_eq!(args.expression, "//control:Button");
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn clap_parsing_window_list() {
        let cli = Cli::try_parse_from(["platynui", "window", "--list"]).expect("parse");
        match cli.command {
            Commands::Window(args) => {
                assert!(args.list);
                assert!(args.expression.is_none());
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn clap_parsing_pointer_move() {
        let cli = Cli::try_parse_from(["platynui", "pointer", "move", "--point", "10,20"]).expect("parse");
        match cli.command {
            Commands::Pointer(args) => match args.command {
                pointer::PointerCommand::Move(move_args) => {
                    assert_eq!(move_args.point, Some(Point::new(10.0, 20.0)));
                    assert!(move_args.expression.is_none());
                }
                _ => panic!("unexpected pointer subcommand"),
            },
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn clap_parsing_keyboard_type() {
        let cli = Cli::try_parse_from(["platynui", "keyboard", "type", "<Ctrl+A>Test"]).expect("parse");
        match cli.command {
            Commands::Keyboard(args) => match args.command {
                keyboard::KeyboardCommand::Type(type_args) => {
                    assert_eq!(type_args.sequence, "<Ctrl+A>Test");
                }
                _ => panic!("unexpected keyboard subcommand"),
            },
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn clap_parsing_keyboard_press() {
        let cli = Cli::try_parse_from(["platynui", "keyboard", "press", "<Ctrl+S>"]).expect("parse");
        match cli.command {
            Commands::Keyboard(args) => match args.command {
                keyboard::KeyboardCommand::Press(press_args) => {
                    assert_eq!(press_args.sequence, "<Ctrl+S>");
                }
                _ => panic!("unexpected keyboard subcommand"),
            },
            _ => panic!("unexpected command"),
        }
    }

    // Exact parse result of integral literals; exact equality is what the test means.
    #[allow(clippy::float_cmp)]
    #[test]
    fn clap_parsing_screenshot_rect_allows_negative() {
        let cli = Cli::try_parse_from(["platynui", "screenshot", "--rect", "-10,-10,200,2000"]).expect("parse");
        match cli.command {
            Commands::Screenshot(args) => {
                let r = args.rect.expect("rect parsed");
                assert_eq!(r.x(), -10.0);
                assert_eq!(r.y(), -10.0);
                assert_eq!(r.width(), 200.0);
                assert_eq!(r.height(), 2000.0);
            }
            _ => panic!("unexpected command"),
        }
    }
}
