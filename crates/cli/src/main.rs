mod commands;
mod util;

#[cfg(any(test, feature = "mock-provider"))]
use platynui_platform_mock as _;
#[cfg(any(test, feature = "mock-provider"))]
use platynui_provider_mock as _;

use clap::{Parser, Subcommand, ValueEnum};
use commands::{
    focus::{self, FocusArgs},
    highlight::{self, HighlightArgs},
    info, list_providers,
    pointer::{self, PointerArgs},
    query::{self, QueryArgs},
    screenshot::{self, ScreenshotArgs},
    watch::{self, WatchArgs},
    window::{self, WindowArgs},
};
use platynui_runtime::Runtime;
use util::{CliResult, map_provider_error};

#[derive(Parser)]
#[command(author, version, about = "PlatynUI command line interface", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "list-providers", about = "List registered UI tree providers.")]
    ListProviders {
        #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    #[command(name = "info", about = "Show desktop and platform metadata.")]
    Info {
        #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    #[command(name = "query", about = "Evaluate XPath expressions.")]
    Query(QueryArgs),
    #[command(name = "watch", about = "Watch provider events.")]
    Watch(WatchArgs),
    #[command(name = "highlight", about = "Highlight elements matching an XPath expression.")]
    Highlight(HighlightArgs),
    #[command(name = "screenshot", about = "Capture a screenshot and save it as PNG.")]
    Screenshot(ScreenshotArgs),
    #[command(name = "focus", about = "Set focus on nodes selected by an XPath expression.")]
    Focus(FocusArgs),
    #[command(name = "window", about = "List or control application windows.")]
    Window(WindowArgs),
    #[command(name = "pointer", about = "Control the pointer (move, click, scroll, drag).")]
    Pointer(PointerArgs),
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run() -> CliResult<()> {
    let cli = Cli::parse();
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
            println!("{output}");
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
        };
    }

    #[test]
    fn clap_parsing_info_defaults_to_text() {
        let cli = Cli::try_parse_from(["platynui", "info"]).expect("parse");
        match cli.command {
            Commands::Info { format } => assert!(matches!(format, OutputFormat::Text)),
            _ => panic!("unexpected command"),
        };
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
        let cli = Cli::try_parse_from(["platynui", "pointer", "move", "10,20"]).expect("parse");
        match cli.command {
            Commands::Pointer(args) => match args.command {
                pointer::PointerCommand::Move(move_args) => {
                    assert_eq!(move_args.point, Point::new(10.0, 20.0));
                }
                _ => panic!("unexpected pointer subcommand"),
            },
            _ => panic!("unexpected command"),
        }
    }
}
