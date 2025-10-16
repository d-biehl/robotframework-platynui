use crate::OutputFormat;
use crate::util::{CliResult, map_evaluate_error};
use clap::Args;
use owo_colors::{OwoColorize, Stream};
use platynui_core::ui::{Namespace, PatternId, UiNode, UiValue};
use platynui_runtime::{EvaluationItem, Runtime};
use serde::Serialize;
use std::fmt::Write;
use std::sync::Arc;

#[derive(Args, Debug, Clone)]
pub struct QueryArgs {
    #[arg(value_name = "XPATH")]
    pub expression: String,
    #[arg(long = "format", value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub(crate) struct AttributeSummary {
    namespace: String,
    name: String,
    value: UiValue,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum QueryItemSummary {
    Node {
        runtime_id: String,
        namespace: String,
        role: String,
        name: String,
        supported_patterns: Vec<String>,
        attributes: Vec<AttributeSummary>,
    },
    Attribute {
        owner_runtime_id: String,
        owner_namespace: String,
        owner_role: String,
        owner_name: String,
        namespace: String,
        name: String,
        value: UiValue,
    },
    Value {
        value: UiValue,
    },
}

pub fn run(runtime: &Runtime, args: &QueryArgs) -> CliResult<String> {
    match args.format {
        OutputFormat::Text => {
            // Stream results directly to stdout as they arrive (no pre‑materialization, no sorting, no pattern lookup)
            let expr = args.expression.trim();
            let iter = runtime.evaluate_iter(None, expr).map_err(map_evaluate_error)?;
            for item in iter {
                match item {
                    EvaluationItem::Node(node) => print_node_stream_text(&node),
                    EvaluationItem::Attribute(attr) => {
                        print_attribute_stream_text(&attr.owner, &attr.namespace, &attr.name, &attr.value)
                    }
                    EvaluationItem::Value(value) => {
                        let plain = format_attribute_value(&value);
                        let colored = colorize_attribute_value(&plain);
                        println!("{colored}");
                    }
                }
            }
            Ok(String::new())
        }
        OutputFormat::Json => {
            // JSON pretty print still materializes to ensure valid JSON array
            let results = runtime.evaluate(None, &args.expression).map_err(map_evaluate_error)?;
            let summaries = summarize_query_results(results);
            render_query_json(&summaries)
        }
    }
}

pub(crate) fn summarize_query_results(results: Vec<EvaluationItem>) -> Vec<QueryItemSummary> {
    results
        .into_iter()
        .map(|item| match item {
            EvaluationItem::Node(node) => {
                let patterns = node.supported_patterns();
                node_to_query_summary(node, patterns)
            }
            EvaluationItem::Attribute(attr) => QueryItemSummary::Attribute {
                owner_runtime_id: attr.owner.runtime_id().as_str().to_owned(),
                owner_namespace: attr.owner.namespace().as_str().to_owned(),
                owner_role: attr.owner.role().to_owned(),
                owner_name: attr.owner.name().to_owned(),
                namespace: attr.namespace.as_str().to_owned(),
                name: attr.name.clone(),
                value: attr.value.clone(),
            },
            EvaluationItem::Value(value) => QueryItemSummary::Value { value },
        })
        .collect()
}

fn node_to_query_summary(node: Arc<dyn UiNode>, patterns: Vec<PatternId>) -> QueryItemSummary {
    let namespace = node.namespace();
    let supported_patterns = patterns.into_iter().map(|id| id.as_str().to_owned()).collect();

    let mut attributes: Vec<AttributeSummary> = node
        .attributes()
        .map(|attribute| AttributeSummary {
            namespace: attribute.namespace().as_str().to_owned(),
            name: attribute.name().to_owned(),
            value: attribute.value(),
        })
        .collect();
    attributes.sort_by(|lhs, rhs| {
        (lhs.namespace.as_str(), lhs.name.as_str()).cmp(&(rhs.namespace.as_str(), rhs.name.as_str()))
    });

    QueryItemSummary::Node {
        runtime_id: node.runtime_id().as_str().to_owned(),
        namespace: namespace.as_str().to_owned(),
        role: node.role().to_owned(),
        name: node.name().to_owned(),
        supported_patterns,
        attributes,
    }
}

fn format_attribute_value(value: &UiValue) -> String {
    match value {
        UiValue::Null => "null".to_string(),
        UiValue::Bool(b) => b.to_string(),
        UiValue::Integer(i) => i.to_string(),
        UiValue::Number(n) => format!("{n}"),
        UiValue::String(text) => {
            serde_json::to_string(text).unwrap_or_else(|_| format!("\"{}\"", text.replace('"', "\\\"")))
        }
        _ => serde_json::to_string(value).unwrap_or_else(|_| String::from("<value>")),
    }
}

fn format_namespace_prefix(namespace: &str) -> String {
    if namespace == Namespace::Control.as_str() { String::new() } else { format!("{namespace}:") }
}

fn format_node_label(namespace: &str, role: &str, name: &str) -> String {
    let prefix = format_namespace_prefix(namespace);
    if name.is_empty() { format!("{prefix}{role}") } else { format!("{prefix}{role} \"{name}\"") }
}

fn colorize_node_label(label: &str) -> String {
    label.if_supports_color(Stream::Stdout, |text| text.bold().fg_rgb::<79, 166, 255>().to_string()).to_string()
}

fn colorize_attribute_name(namespace_prefix: &str, name: &str) -> String {
    let rendered = format!("@{namespace_prefix}{name}");
    rendered.if_supports_color(Stream::Stdout, |text| text.bold().fg_rgb::<241, 149, 255>().to_string()).to_string()
}

fn colorize_attribute_value(value: &str) -> String {
    value.if_supports_color(Stream::Stdout, |text| text.fg_rgb::<136, 192, 74>().to_string()).to_string()
}

fn colorize_owner_label(label: &str) -> String {
    label.if_supports_color(Stream::Stdout, |text| text.dimmed().to_string()).to_string()
}

pub(crate) fn render_query_text(items: &[QueryItemSummary]) -> String {
    let mut output = String::new();
    for item in items {
        match item {
            QueryItemSummary::Node { runtime_id: _, namespace, role, name, supported_patterns: _, attributes } => {
                let node_label = format_node_label(namespace.as_str(), role.as_str(), name.as_str());
                let colored_node_label = colorize_node_label(&node_label);
                let _ = writeln!(&mut output, "{colored_node_label}");
                for attribute in attributes {
                    let value = format_attribute_value(&attribute.value);
                    let attribute_namespace = format_namespace_prefix(attribute.namespace.as_str());
                    let colored_name = colorize_attribute_name(&attribute_namespace, &attribute.name);
                    let colored_value = colorize_attribute_value(&value);
                    let _ = writeln!(&mut output, "    {colored_name} = {colored_value}",);
                }
            }
            QueryItemSummary::Attribute {
                owner_runtime_id: _,
                owner_namespace,
                owner_role,
                owner_name,
                namespace,
                name,
                value,
            } => {
                let attribute_namespace = format_namespace_prefix(namespace.as_str());
                let owner_label = format_node_label(owner_namespace.as_str(), owner_role.as_str(), owner_name.as_str());
                let colored_owner = colorize_owner_label(&owner_label);
                let colored_name = colorize_attribute_name(&attribute_namespace, name);
                let value_text = format_attribute_value(value);
                let colored_value = colorize_attribute_value(&value_text);
                let _ = writeln!(&mut output, "{colored_name} = {colored_value} ({colored_owner})",);
            }
            QueryItemSummary::Value { value } => {
                let plain = format_attribute_value(value);
                let colored = colorize_attribute_value(&plain);
                let _ = writeln!(&mut output, "{colored}");
            }
        }
    }

    output.trim_end().to_owned()
}

fn print_node_stream_text(node: &Arc<dyn UiNode>) {
    let namespace = node.namespace();
    let node_label = format_node_label(namespace.as_str(), node.role(), node.name().as_str());
    println!("{}", colorize_node_label(&node_label));
    for attribute in node.attributes() {
        let value = format_attribute_value(&attribute.value());
        let attribute_namespace = format_namespace_prefix(attribute.namespace().as_str());
        let colored_name = colorize_attribute_name(&attribute_namespace, attribute.name());
        let colored_value = colorize_attribute_value(&value);
        println!("    {colored_name} = {colored_value}");
    }
}

fn print_attribute_stream_text(owner: &Arc<dyn UiNode>, namespace: &Namespace, name: &str, value: &UiValue) {
    let attribute_namespace = format_namespace_prefix(namespace.as_str());
    let owner_label = format_node_label(owner.namespace().as_str(), owner.role(), owner.name().as_str());
    let colored_owner = colorize_owner_label(&owner_label);
    let colored_name = colorize_attribute_name(&attribute_namespace, name);
    let value_text = format_attribute_value(value);
    let colored_value = colorize_attribute_value(&value_text);
    println!("{colored_name} = {colored_value} ({colored_owner})");
}

pub(crate) fn render_query_json(items: &[QueryItemSummary]) -> CliResult<String> {
    Ok(serde_json::to_string_pretty(items)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::runtime;
    use platynui_runtime::Runtime;
    use rstest::rstest;
    use std::borrow::Cow;

    fn strip_ansi(input: &str) -> Cow<'_, str> {
        if !input.contains('\u{1b}') {
            return Cow::Borrowed(input);
        }

        let mut result = String::with_capacity(input.len());
        let mut chars = input.chars();
        loop {
            match chars.next() {
                Some('\u{1b}') => {
                    for next in chars.by_ref() {
                        if next == 'm' {
                            break;
                        }
                    }
                }
                Some(ch) => result.push(ch),
                None => break,
            }
        }
        Cow::Owned(result)
    }

    #[rstest]
    fn query_text_returns_nodes(runtime: Runtime) {
        let args = QueryArgs { expression: "//control:Button".into(), format: OutputFormat::Text };
        // Capture stdout by rendering a single item using helper
        let results = runtime.evaluate(None, &args.expression).expect("eval");
        let summaries = summarize_query_results(results);
        let output = render_query_text(&summaries);
        let plain = strip_ansi(&output);
        assert!(!plain.contains("control:Button"));
        assert!(!plain.contains("mock://desktop"));
    }

    #[rstest]
    fn query_attribute_text_omits_default_namespace(runtime: Runtime) {
        let args = QueryArgs { expression: "//control:Button/@Name".into(), format: OutputFormat::Text };
        let results = runtime.evaluate(None, &args.expression).expect("eval");
        let output = render_query_text(&summaries_for(results));
        let plain = strip_ansi(&output);
        assert!(plain.contains("@Name = \""));
        assert!(plain.contains("(Button \""));
        assert!(!plain.contains("@control:Name"));
        assert!(!plain.contains("mock://desktop"));
    }

    #[rstest]
    fn query_json_produces_valid_payload(runtime: Runtime) {
        let args = QueryArgs { expression: "//control:Button".into(), format: OutputFormat::Json };
        let output = run(&runtime, &args).expect("query");
        let payload = output.trim();
        let json: serde_json::Value = serde_json::from_str(payload).expect("json");
        assert_eq!(json[0]["type"], "Node");
    }

    fn summaries_for(results: Vec<EvaluationItem>) -> Vec<QueryItemSummary> {
        summarize_query_results(results)
    }
}
