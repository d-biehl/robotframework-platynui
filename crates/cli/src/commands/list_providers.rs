use crate::OutputFormat;
use crate::util::CliResult;
use platynui_core::provider::ProviderKind;
use platynui_runtime::Runtime;
use serde::Serialize;
use std::collections::HashSet;
use std::fmt::Write;

#[derive(Serialize, Debug, PartialEq, Eq)]
struct ProviderSummary {
    id: String,
    name: String,
    technology: String,
    kind: String,
    active: bool,
}

pub fn run(runtime: &Runtime, format: OutputFormat) -> CliResult<String> {
    let summaries = collect_provider_summaries(runtime);
    let output = match format {
        OutputFormat::Text => render_provider_text(&summaries),
        OutputFormat::Json => render_provider_json(&summaries)?,
    };
    Ok(output)
}

fn collect_provider_summaries(runtime: &Runtime) -> Vec<ProviderSummary> {
    let active: HashSet<&str> = runtime.providers().map(|provider| provider.descriptor().id).collect();
    runtime
        .registry()
        .entries()
        .map(|entry| ProviderSummary {
            id: entry.descriptor.id.to_owned(),
            name: entry.descriptor.display_name.to_owned(),
            technology: entry.descriptor.technology.as_str().to_owned(),
            kind: kind_label(entry.descriptor.kind).to_owned(),
            active: active.contains(entry.descriptor.id),
        })
        .collect()
}

fn render_provider_text(summaries: &[ProviderSummary]) -> String {
    let mut output = String::new();
    let _ = writeln!(&mut output, "{:<16} {:<12} {:<8} {:<7} {:<}", "Id", "Technology", "Kind", "Active", "Name");

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

fn render_provider_json(summaries: &[ProviderSummary]) -> CliResult<String> {
    Ok(serde_json::to_string_pretty(summaries)?)
}

fn kind_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Native => "native",
        ProviderKind::External => "external",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::runtime;
    use rstest::rstest;

    #[rstest]
    fn summaries_include_mock_provider(runtime: platynui_runtime::Runtime) {
        let summaries = collect_provider_summaries(&runtime);

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

        let output = render_provider_text(&summaries);
        assert!(output.contains("Mock Provider"));
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

        let json = render_provider_json(&summaries).expect("json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed[0]["id"], "mock");
    }
}
