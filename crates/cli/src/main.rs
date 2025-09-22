use clap::{Parser, Subcommand, ValueEnum};
use platynui_core::provider::{ProviderError, ProviderKind};
use platynui_runtime::Runtime;
use serde::Serialize;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::Write;

#[allow(unused_imports)]
use platynui_platform_mock as _;
#[allow(unused_imports)]
use platynui_provider_mock as _;

type CliResult<T> = Result<T, Box<dyn Error>>;

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
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
struct ProviderSummary {
    id: String,
    name: String,
    technology: String,
    kind: String,
    active: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run() -> CliResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::ListProviders { format } => handle_list_providers(format),
    }
}

fn handle_list_providers(format: OutputFormat) -> CliResult<()> {
    let mut runtime = Runtime::new().map_err(map_provider_error)?;
    let summaries = collect_provider_summaries(&runtime);

    let output = match format {
        OutputFormat::Text => render_text(&summaries),
        OutputFormat::Json => render_json(&summaries)?,
    };

    println!("{output}");

    runtime.shutdown();
    Ok(())
}

fn collect_provider_summaries(runtime: &Runtime) -> Vec<ProviderSummary> {
    let active_ids: HashSet<&str> =
        runtime.providers().map(|provider| provider.descriptor().id).collect();

    runtime
        .registry()
        .entries()
        .map(|entry| ProviderSummary {
            id: entry.descriptor.id.to_owned(),
            name: entry.descriptor.display_name.to_owned(),
            technology: entry.descriptor.technology.as_str().to_owned(),
            kind: kind_label(entry.descriptor.kind).to_owned(),
            active: active_ids.contains(entry.descriptor.id),
        })
        .collect()
}

fn render_text(summaries: &[ProviderSummary]) -> String {
    let mut output = String::new();
    let _ = writeln!(
        &mut output,
        "{:<16} {:<12} {:<8} {:<7} {}",
        "ID", "Technology", "Kind", "Active", "Name"
    );

    for summary in summaries {
        let _ = writeln!(
            &mut output,
            "{:<16} {:<12} {:<8} {:<7} {}",
            summary.id,
            summary.technology,
            summary.kind,
            if summary.active { "yes" } else { "no" },
            summary.name
        );
    }

    output.trim_end().to_owned()
}

fn render_json(summaries: &[ProviderSummary]) -> CliResult<String> {
    Ok(serde_json::to_string_pretty(summaries)?)
}

fn kind_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Native => "native",
        ProviderKind::External => "external",
    }
}

fn map_provider_error(err: ProviderError) -> Box<dyn Error> {
    Box::new(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rstest::rstest;

    #[rstest]
    fn summaries_include_mock_provider() {
        let mut runtime = Runtime::new().map_err(map_provider_error).expect("runtime");
        let summaries = collect_provider_summaries(&runtime);
        runtime.shutdown();

        assert!(summaries.iter().any(|summary| summary.id == "mock"));
    }

    #[rstest]
    fn render_text_formats_table() {
        let summaries = vec![ProviderSummary {
            id: "mock".into(),
            name: "Mock Provider".into(),
            technology: "MockTech".into(),
            kind: "native".into(),
            active: true,
        }];

        let rendered = render_text(&summaries);
        let lines: Vec<_> = rendered.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("mock"));
        assert!(lines[1].contains("yes"));
    }

    #[rstest]
    fn render_json_produces_valid_json() {
        let summaries = vec![ProviderSummary {
            id: "mock".into(),
            name: "Mock Provider".into(),
            technology: "MockTech".into(),
            kind: "native".into(),
            active: true,
        }];

        let rendered = render_json(&summaries).expect("json");
        let value: serde_json::Value = serde_json::from_str(&rendered).expect("valid json");
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 1);
    }

    #[rstest]
    fn clap_parsing_defaults_to_text() {
        let cli = Cli::parse_from(["platynui-cli", "list-providers"]);
        match cli.command {
            Commands::ListProviders { format } => assert!(matches!(format, OutputFormat::Text)),
        }
    }
}
