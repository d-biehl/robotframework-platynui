use std::collections::HashMap;

use platynui_core::ui::{Namespace as UiNamespace, UiNode, UiValue, attribute_names};
use platynui_xpath::compiler;
use platynui_xpath::engine::evaluator;
use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::model::NodeKind;
use platynui_xpath::simple_node::{self, SimpleNode};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{self, XdmNode};
use serde_json;
use thiserror::Error;

const CONTROL_NS_URI: &str = "urn:platynui:control";
const ITEM_NS_URI: &str = "urn:platynui:item";
const APP_NS_URI: &str = "urn:platynui:app";
const NATIVE_NS_URI: &str = "urn:platynui:native";

/// Options passed to the XPath evaluator.
#[derive(Default)]
pub struct EvaluateOptions<'a> {
    pub context_node: Option<&'a UiNode>,
}

impl<'a> EvaluateOptions<'a> {
    pub fn with_context_node(mut self, node: &'a UiNode) -> Self {
        self.context_node = Some(node);
        self
    }
}

#[derive(Debug, Error)]
pub enum EvaluateError {
    #[error("XPath evaluation failed: {0}")]
    XPath(#[from] platynui_xpath::engine::runtime::Error),
    #[error("context node not part of snapshot (runtime id: {0})")]
    ContextNodeUnknown(String),
}

#[derive(Debug, Default, PartialEq)]
pub struct EvaluationResult {
    pub nodes: Vec<UiNode>,
    pub atomics: Vec<UiValue>,
}

pub fn evaluate_xpath(
    expr: &str,
    root: &UiNode,
    options: EvaluateOptions<'_>,
) -> Result<EvaluationResult, EvaluateError> {
    let mut source_map = HashMap::new();
    let mut simple_map = HashMap::new();
    let simple_root = build_simple_node(root, true, &mut source_map, &mut simple_map);
    let _document = simple_node::doc().child(simple_root).build();

    let static_ctx = StaticContextBuilder::new()
        .with_default_element_namespace(CONTROL_NS_URI)
        .with_namespace("control", CONTROL_NS_URI)
        .with_namespace("item", ITEM_NS_URI)
        .with_namespace("app", APP_NS_URI)
        .with_namespace("native", NATIVE_NS_URI)
        .build();

    let compiled = compiler::compile_with_context(expr, &static_ctx)?;

    let mut dyn_builder = DynamicContextBuilder::new();
    if let Some(ctx_node) = options.context_node {
        let runtime_id = ctx_node.runtime_id().as_str();
        let simple_node = simple_map
            .get(runtime_id)
            .cloned()
            .ok_or_else(|| EvaluateError::ContextNodeUnknown(runtime_id.to_string()))?;
        dyn_builder = dyn_builder.with_context_item(simple_node);
    } else {
        let root_id = root.runtime_id().as_str();
        if let Some(simple_root) = simple_map.get(root_id).cloned() {
            dyn_builder = dyn_builder.with_context_item(simple_root);
        }
    }
    let dyn_ctx = dyn_builder.build();

    let sequence = evaluator::evaluate(&compiled, &dyn_ctx)?;

    let mut result = EvaluationResult::default();
    for item in sequence {
        match item {
            XdmItem::Node(node) => match node.kind() {
                NodeKind::Element => {
                    if let Some(id) = extract_runtime_id(&node) {
                        if let Some(original) = source_map.get(&id) {
                            result.nodes.push(original.clone());
                        }
                    }
                }
                NodeKind::Attribute | NodeKind::Text => {
                    result.atomics.push(UiValue::String(node.string_value()));
                }
                _ => {}
            },
            XdmItem::Atomic(atom) => {
                result.atomics.push(atomic_to_ui_value(&atom));
            }
        }
    }

    Ok(result)
}

fn build_simple_node(
    node: &UiNode,
    is_root: bool,
    source_map: &mut HashMap<String, UiNode>,
    simple_map: &mut HashMap<String, SimpleNode>,
) -> SimpleNode {
    let qname = element_qname(node.namespace(), node.role());
    let mut builder = simple_node::elem(&qname);

    if is_root {
        builder = builder
            .namespace(simple_node::ns("", CONTROL_NS_URI))
            .namespace(simple_node::ns("control", CONTROL_NS_URI))
            .namespace(simple_node::ns("item", ITEM_NS_URI))
            .namespace(simple_node::ns("app", APP_NS_URI))
            .namespace(simple_node::ns("native", NATIVE_NS_URI));
    }

    for (key, value) in node.attributes().iter() {
        if let Some(attr_node) = convert_attribute(key, value) {
            builder = builder.attr(attr_node);
        }
    }

    for child in node.children() {
        let simple_child = build_simple_node(child, false, source_map, simple_map);
        builder = builder.child(simple_child);
    }

    let built = builder.build();
    let runtime_id = node.runtime_id().as_str().to_string();
    source_map.insert(runtime_id.clone(), node.clone());
    simple_map.insert(runtime_id, built.clone());
    built
}

fn convert_attribute(
    key: &platynui_core::ui::node::AttributeKey,
    value: &UiValue,
) -> Option<SimpleNode> {
    let name = match key.namespace() {
        UiNamespace::Control => key.name().to_string(),
        other => {
            let prefix = namespace_prefix(other);
            format!("{}:{}", prefix, key.name())
        }
    };

    let text = ui_value_to_string(value)?;
    Some(simple_node::attr(&name, &text))
}

fn ui_value_to_string(value: &UiValue) -> Option<String> {
    match value {
        UiValue::Null => None,
        UiValue::Bool(b) => Some(b.to_string()),
        UiValue::Integer(i) => Some(i.to_string()),
        UiValue::Number(n) => Some(trim_float(*n)),
        UiValue::String(s) => Some(s.clone()),
        UiValue::Array(items) => serde_json::to_string(items).ok(),
        UiValue::Object(map) => serde_json::to_string(map).ok(),
        UiValue::Point(p) => serde_json::to_string(p).ok(),
        UiValue::Size(s) => serde_json::to_string(s).ok(),
        UiValue::Rect(r) => serde_json::to_string(r).ok(),
    }
}

fn trim_float(value: f64) -> String {
    let s = format!("{}", value);
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

fn element_qname(ns: UiNamespace, role: &str) -> String {
    match ns {
        UiNamespace::Control => role.to_string(),
        _ => format!("{}:{}", namespace_prefix(ns), role),
    }
}

fn namespace_prefix(ns: UiNamespace) -> &'static str {
    match ns {
        UiNamespace::Control => "control",
        UiNamespace::Item => "item",
        UiNamespace::App => "app",
        UiNamespace::Native => "native",
    }
}

fn extract_runtime_id(node: &SimpleNode) -> Option<String> {
    for attr in node.attributes() {
        if let Some(name) = attr.name()
            && name.local == attribute_names::RUNTIME_ID
        {
            return Some(attr.string_value());
        }
    }
    None
}

fn atomic_to_ui_value(value: &XdmAtomicValue) -> UiValue {
    use XdmAtomicValue::*;
    match value {
        Boolean(b) => UiValue::Bool(*b),
        String(s) | UntypedAtomic(s) | AnyUri(s) | NormalizedString(s) | Token(s) | Language(s)
        | Name(s) | NCName(s) | NMTOKEN(s) | Id(s) | IdRef(s) | Entity(s) | Notation(s) => {
            UiValue::String(s.clone())
        }
        Integer(i) | Long(i) | NonPositiveInteger(i) | NegativeInteger(i) => UiValue::Integer(*i),
        Decimal(d) | Double(d) => UiValue::Number(*d),
        Float(f) => UiValue::Number(*f as f64),
        UnsignedLong(u) | NonNegativeInteger(u) | PositiveInteger(u) => UiValue::Integer(*u as i64),
        UnsignedInt(u) => UiValue::Integer(*u as i64),
        UnsignedShort(u) => UiValue::Integer(*u as i64),
        UnsignedByte(u) => UiValue::Integer(*u as i64),
        Int(i) => UiValue::Integer(*i as i64),
        Short(i) => UiValue::Integer(*i as i64),
        Byte(i) => UiValue::Integer(*i as i64),
        QName { ns_uri, prefix, local } => {
            let mut map = std::collections::BTreeMap::new();
            if let Some(ns) = ns_uri {
                map.insert("ns_uri".to_string(), UiValue::String(ns.clone()));
            }
            if let Some(pref) = prefix {
                map.insert("prefix".to_string(), UiValue::String(pref.clone()));
            }
            map.insert("local".to_string(), UiValue::String(local.clone()));
            UiValue::Object(map)
        }
        DateTime(dt) => UiValue::String(dt.to_rfc3339()),
        Date { date, tz } => UiValue::String(match tz {
            Some(offset) => format!("{}{}", date, offset),
            None => date.to_string(),
        }),
        Time { time, tz } => UiValue::String(match tz {
            Some(offset) => format!("{}{}", time, offset),
            None => time.to_string(),
        }),
        YearMonthDuration(months) => UiValue::String(format!("P{}M", months)),
        DayTimeDuration(secs) => UiValue::String(format!("PT{}S", secs)),
        Base64Binary(data) | HexBinary(data) => UiValue::String(data.clone()),
        GYear { year, tz } => {
            UiValue::String(format!("{}{}", year, tz.map_or("".to_string(), |o| o.to_string())))
        }
        GYearMonth { year, month, tz } => UiValue::String(format!(
            "{}-{:02}{}",
            year,
            month,
            tz.map_or("".to_string(), |o| o.to_string())
        )),
        GMonth { month, tz } => {
            UiValue::String(format!("{:02}{}", month, tz.map_or("".to_string(), |o| o.to_string())))
        }
        GMonthDay { month, day, tz } => UiValue::String(format!(
            "{:02}-{:02}{}",
            month,
            day,
            tz.map_or("".to_string(), |o| o.to_string())
        )),
        GDay { day, tz } => {
            UiValue::String(format!("{:02}{}", day, tz.map_or("".to_string(), |o| o.to_string())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::types::Rect;
    use platynui_core::ui::UiNode;

    fn sample_tree() -> UiNode {
        let window = UiNode::builder(
            UiNamespace::Control,
            "Window",
            "Main",
            Rect::new(0.0, 0.0, 800.0, 600.0),
            "window-1",
            "Mock",
        )
        .with_attribute(
            UiNamespace::Control,
            attribute_names::NAME,
            UiValue::String("Main Window".into()),
        )
        .build();

        UiNode::builder(
            UiNamespace::Control,
            "Desktop",
            "Desktop",
            Rect::new(0.0, 0.0, 1920.0, 1080.0),
            "desktop",
            "Runtime",
        )
        .with_attribute(
            UiNamespace::Control,
            attribute_names::OS_NAME,
            UiValue::String("Linux".into()),
        )
        .with_children(vec![window])
        .build()
    }

    #[test]
    fn evaluates_node_selection() {
        let tree = sample_tree();
        let result = evaluate_xpath("//Window", &tree, EvaluateOptions::default()).unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].runtime_id().as_str(), "window-1");
    }

    #[test]
    fn evaluates_count_function() {
        let tree = sample_tree();
        let result = evaluate_xpath("count(//Window)", &tree, EvaluateOptions::default()).unwrap();
        assert!(result.nodes.is_empty());
        assert_eq!(result.atomics.len(), 1);
        assert_eq!(result.atomics[0], UiValue::Integer(1));
    }
}
