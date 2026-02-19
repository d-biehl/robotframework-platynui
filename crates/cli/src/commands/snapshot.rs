use crate::util::CliResult;
use clap::Args;
use owo_colors::{OwoColorize, Stream};
use platynui_core::ui::attribute_names::{activation_target, common, element};
use platynui_core::ui::{Namespace, UiNode, UiValue};
use platynui_runtime::{EvaluationItem, Runtime};
use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Args, Debug, Clone)]
pub struct SnapshotArgs {
    #[arg(value_name = "XPATH", help = "XPath expression selecting root node(s).")]
    pub expression: String,

    #[arg(
        long = "output",
        value_name = "FILE",
        conflicts_with = "split",
        help = "Output file (XML). If multiple roots are selected, they are wrapped in a <snapshot> element."
    )]
    pub output: Option<PathBuf>,

    #[arg(
        long = "split",
        value_name = "PREFIX",
        help = "Write one XML file per root as PREFIX-001.xml, PREFIX-002.xml, ..."
    )]
    pub split: Option<PathBuf>,

    #[arg(long = "max-depth", value_name = "N", help = "Limit recursion depth (0 = only root). Default: unlimited.")]
    pub max_depth: Option<usize>,

    #[arg(long = "attrs", value_enum, default_value_t = AttrMode::All, help = "Attribute selection mode: all (default), default (Name, Id), or list (use --include/--exclude)")]
    pub attrs: AttrMode,

    #[arg(long = "include", value_name = "NS:NAME", num_args=1.., action = clap::ArgAction::Append, help = "Include attributes matching pattern (supports * wildcard). Example: control:Bounds*, app:*")]
    pub include: Vec<String>,

    #[arg(long = "exclude", value_name = "NS:NAME", num_args=1.., action = clap::ArgAction::Append, help = "Exclude attributes matching pattern (supports * wildcard). Applied after --include/--attrs.")]
    pub exclude: Vec<String>,

    #[arg(
        long = "exclude-derived",
        help = "Suppress derived alias attributes like Bounds.X/Y/Width/Height or ActivationPoint.X/Y."
    )]
    pub exclude_derived: bool,

    #[arg(long = "include-runtime-id", help = "Include control:RuntimeId attribute in output.")]
    pub include_runtime_id: bool,

    #[arg(long = "pretty", help = "Pretty print (indentation and newlines).")]
    pub pretty: bool,

    #[arg(long = "format", value_enum, default_value_t = SnapshotFormat::Text, help = "Output format: text (default) or xml. Text prints a readable tree to stdout or file. XML requires --format xml.")]
    pub format: SnapshotFormat,

    #[arg(
        long = "no-attrs",
        help = "Suppress attribute lines in text output (structure only). Has no effect for XML."
    )]
    pub no_attrs: bool,

    #[arg(long = "no-color", help = "Disable ANSI colors in text output.")]
    pub no_color: bool,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum AttrMode {
    Default,
    All,
    List,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotFormat {
    Text,
    Xml,
}

pub fn run(runtime: &Runtime, args: &SnapshotArgs) -> CliResult<String> {
    if args.no_color {
        // Disable colors globally for the process
        owo_colors::set_override(false);
    }
    let results = runtime.evaluate(None, &args.expression)?;
    let mut roots: Vec<Arc<dyn UiNode>> = Vec::new();
    for item in results {
        if let EvaluationItem::Node(n) = item {
            roots.push(n);
        }
    }
    if roots.is_empty() {
        anyhow::bail!("expression `{}` did not match any nodes", args.expression);
    }

    if let Some(prefix) = &args.split {
        // one file per root
        let mut index: usize = 1;
        for node in roots {
            let path = numbered_path(prefix, index, args.format);
            match args.format {
                SnapshotFormat::Xml => write_single_document_xml(&path, &node, args)?,
                SnapshotFormat::Text => write_single_document_text(&path, &node, args)?,
            }
            index += 1;
        }
        let ext = match args.format {
            SnapshotFormat::Xml => "xml",
            SnapshotFormat::Text => "txt",
        };
        return Ok(format!("Saved {} snapshot file(s) with prefix {}-NNN.{}.", index - 1, prefix.display(), ext));
    }

    match &args.output {
        Some(path) => {
            match args.format {
                SnapshotFormat::Xml => write_wrapped_document_xml(path, &roots, args)?,
                SnapshotFormat::Text => write_wrapped_document_text(path, &roots, args)?,
            }
            Ok(format!("Saved snapshot to {} ({} root(s)).", path.display(), roots.len()))
        }
        None => {
            // stdout (default text unless --format xml)
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            match args.format {
                SnapshotFormat::Xml => write_wrapped_xml_to(&mut handle, &roots, args)?,
                SnapshotFormat::Text => write_text_to(&mut handle, &roots, args)?,
            }
            Ok(String::new())
        }
    }
}

fn write_wrapped_document_xml(path: &Path, roots: &[Arc<dyn UiNode>], args: &SnapshotArgs) -> CliResult<()> {
    let file = File::create(path)?;
    let mut writer = Writer::new(BufWriter::new(file));
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    if roots.len() == 1 {
        write_root_element(&mut writer, &roots[0], args, /*inject_namespaces*/ true, /*pretty_prefix*/ 0)?;
    } else {
        let mut start = BytesStart::new("snapshot");
        inject_namespaces(&mut start);
        writer.write_event(Event::Start(start))?;
        if args.pretty {
            writer.write_event(Event::Text(BytesText::new("\n")))?;
        }
        for (i, node) in roots.iter().enumerate() {
            if args.pretty {
                write_indent(&mut writer, 1)?;
            }
            write_root_element(&mut writer, node, args, /*inject_namespaces*/ false, /*pretty_prefix*/ 1)?;
            if args.pretty {
                writer.write_event(Event::Text(BytesText::new("\n")))?;
            }
            if !args.pretty && i + 1 < roots.len() { /* no separator needed */ }
        }
        writer.write_event(Event::End(BytesEnd::new("snapshot")))?;
    }
    Ok(())
}

fn write_wrapped_xml_to<W: Write>(writer: &mut W, roots: &[Arc<dyn UiNode>], args: &SnapshotArgs) -> CliResult<()> {
    let mut writer = Writer::new(writer);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

    if roots.len() == 1 {
        write_root_element(&mut writer, &roots[0], args, /*inject_namespaces*/ true, /*pretty_prefix*/ 0)?;
    } else {
        let mut start = BytesStart::new("snapshot");
        inject_namespaces(&mut start);
        writer.write_event(Event::Start(start))?;
        if args.pretty {
            writer.write_event(Event::Text(BytesText::new("\n")))?;
        }
        for (i, node) in roots.iter().enumerate() {
            if args.pretty {
                write_indent(&mut writer, 1)?;
            }
            write_root_element(&mut writer, node, args, /*inject_namespaces*/ false, /*pretty_prefix*/ 1)?;
            if args.pretty {
                writer.write_event(Event::Text(BytesText::new("\n")))?;
            }
            if !args.pretty && i + 1 < roots.len() { /* no separator */ }
        }
        writer.write_event(Event::End(BytesEnd::new("snapshot")))?;
    }
    Ok(())
}

fn write_single_document_xml(path: &Path, root: &Arc<dyn UiNode>, args: &SnapshotArgs) -> CliResult<()> {
    let file = File::create(path)?;
    let mut writer = Writer::new(BufWriter::new(file));
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    write_root_element(&mut writer, root, args, /*inject_namespaces*/ true, /*pretty_prefix*/ 0)?;
    Ok(())
}

fn write_root_element<W: Write>(
    writer: &mut Writer<W>,
    node: &Arc<dyn UiNode>,
    args: &SnapshotArgs,
    with_ns: bool,
    pretty_prefix: usize,
) -> CliResult<()> {
    write_node(writer, node, args, with_ns, pretty_prefix, 0)
}

fn write_node<W: Write>(
    writer: &mut Writer<W>,
    node: &Arc<dyn UiNode>,
    args: &SnapshotArgs,
    with_ns: bool,
    indent_level: usize,
    depth: usize,
) -> CliResult<()> {
    // Determine element name and namespace prefix
    let ns = node.namespace();
    let role = node.role();
    let tag_name = if ns == Namespace::Control { role.to_string() } else { format!("{}:{}", ns.as_str(), role) };
    let mut start = BytesStart::new(tag_name.as_str());
    if with_ns {
        inject_namespaces(&mut start);
    }

    // Collect attributes per selection
    let mut attrs = collect_attributes(node, args);
    // Sort attributes for stable output (ns, name)
    attrs.sort_by(|a, b| (a.0.as_str(), a.1.as_str()).cmp(&(b.0.as_str(), b.1.as_str())));

    for (ns, name, value) in attrs.into_iter().map(|(ns, name, value)| (ns, name, format_value(&value))) {
        let qname = format!("{}:{}", ns.as_str(), name);
        start.push_attribute((qname.as_str(), value.as_str()));
    }

    writer.write_event(Event::Start(start))?;

    // Recurse into children if depth limit allows
    let has_children = args.max_depth.map(|max| depth < max).unwrap_or(true);

    if has_children {
        let mut first = true;
        for child in node.children() {
            if args.pretty {
                if first {
                    writer.write_event(Event::Text(BytesText::new("\n")))?;
                    first = false;
                }
                write_indent(writer, indent_level + 1)?;
            }
            write_node(writer, &child, args, false, indent_level + 1, depth + 1)?;
        }
        if args.pretty && !first {
            writer.write_event(Event::Text(BytesText::new("\n")))?;
            write_indent(writer, indent_level)?;
        }
    }

    let end = BytesEnd::new(tag_name.as_str());
    writer.write_event(Event::End(end))?;
    Ok(())
}

fn write_indent<W: Write>(writer: &mut Writer<W>, level: usize) -> CliResult<()> {
    let mut buf = String::with_capacity(level * 2);
    for _ in 0..level {
        buf.push_str("  ");
    }
    writer.write_event(Event::Text(BytesText::new(&buf)))?;
    Ok(())
}

fn inject_namespaces(start: &mut BytesStart) {
    // Default namespace for control elements
    start.push_attribute(("xmlns", "urn:platynui:control"));
    start.push_attribute(("xmlns:control", "urn:platynui:control"));
    start.push_attribute(("xmlns:item", "urn:platynui:item"));
    start.push_attribute(("xmlns:app", "urn:platynui:app"));
    start.push_attribute(("xmlns:native", "urn:platynui:native"));
}

fn numbered_path(prefix: &Path, index: usize, format: SnapshotFormat) -> PathBuf {
    let stem = prefix.as_os_str().to_string_lossy().to_string();
    let ext = match format {
        SnapshotFormat::Xml => "xml",
        SnapshotFormat::Text => "txt",
    };
    let filename = format!("{}-{:03}.{}", stem, index, ext);
    PathBuf::from(filename)
}

fn format_value(value: &UiValue) -> String {
    match value {
        UiValue::Null => String::from("null"),
        UiValue::Bool(b) => b.to_string(),
        UiValue::Integer(i) => i.to_string(),
        UiValue::Number(n) => {
            if n.fract().abs() < f64::EPSILON {
                format!("{:.0}", n)
            } else {
                n.to_string()
            }
        }
        UiValue::String(s) => s.clone(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| String::from("<value>")),
    }
}

fn format_value_text(value: &UiValue) -> String {
    match value {
        UiValue::Null => String::from("null"),
        UiValue::Bool(b) => b.to_string(),
        UiValue::Integer(i) => i.to_string(),
        UiValue::Number(n) => {
            if n.fract().abs() < f64::EPSILON {
                format!("{:.0}", n)
            } else {
                n.to_string()
            }
        }
        UiValue::String(s) => serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s)),
        _ => serde_json::to_string(value).unwrap_or_else(|_| String::from("<value>")),
    }
}

#[derive(Clone)]
struct AttrSelector {
    ns: Namespace,
    pattern: String,
}

impl AttrSelector {
    fn matches(&self, ns: Namespace, name: &str) -> bool {
        if self.ns != ns {
            return false;
        }
        wildcard_match(&self.pattern, name)
    }
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    // simple '*' matcher (greedy), no '?' support
    if pattern == "*" {
        return true;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    // ensure order of parts in text
    let mut pos = 0usize;
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() {
            continue;
        }
        if let Some(found) = text[pos..].find(p) {
            if i == 0 && !pattern.starts_with(p) && !pattern.starts_with("*") && found != 0 {
                return false;
            }
            pos += found + p.len();
        } else {
            return false;
        }
    }
    if !pattern.ends_with(parts.last().unwrap_or(&"")) && !pattern.ends_with('*') {
        return false;
    }
    true
}

fn parse_selectors(values: &[String]) -> Vec<AttrSelector> {
    values
        .iter()
        .filter_map(|s| {
            let mut it = s.splitn(2, ':');
            let ns = it.next()?.trim().to_ascii_lowercase();
            let name = it.next().unwrap_or("").trim();
            let ns_parsed = Namespace::from_str(&ns).ok()?;
            Some(AttrSelector { ns: ns_parsed, pattern: name.to_string() })
        })
        .collect()
}

fn collect_attributes(node: &Arc<dyn UiNode>, args: &SnapshotArgs) -> Vec<(Namespace, String, UiValue)> {
    let include = parse_selectors(&args.include);
    let exclude = parse_selectors(&args.exclude);

    let mut out: Vec<(Namespace, String, UiValue)> = Vec::new();

    // Helper to push if selected
    let mut push_if = |ns: Namespace, name: &str, value: UiValue| {
        if selected(ns, name, &value, args, &include, &exclude) {
            out.push((ns, name.to_string(), value));
        }
    };

    // Iterate provided attributes
    for attr in node.attributes() {
        let ns = attr.namespace();
        let name = attr.name().to_string();
        let value = attr.value();
        push_if(ns, &name, value.clone());

        // Derived alias attributes
        if !args.exclude_derived {
            if ns == Namespace::Control && name == element::BOUNDS {
                if let UiValue::Rect(r) = value {
                    push_if(ns, "Bounds.X", UiValue::from(r.x()));
                    push_if(ns, "Bounds.Y", UiValue::from(r.y()));
                    push_if(ns, "Bounds.Width", UiValue::from(r.width()));
                    push_if(ns, "Bounds.Height", UiValue::from(r.height()));
                }
            } else if ns == Namespace::Control
                && name == activation_target::ACTIVATION_POINT
                && let UiValue::Point(p) = value
            {
                push_if(ns, "ActivationPoint.X", UiValue::from(p.x()));
                push_if(ns, "ActivationPoint.Y", UiValue::from(p.y()));
            }
        }
    }

    // Default set additions
    if matches!(args.attrs, AttrMode::Default) {
        // Guarantee presence of Name if available (some providers may omit as Null)
        if let Some(attr) = node.attribute(Namespace::Control, common::NAME) {
            let v = attr.value();
            out.push((Namespace::Control, common::NAME.to_string(), v));
        }
        if let Some(id_attr) = node.attribute(Namespace::Control, common::ID) {
            let v = id_attr.value();
            if !v.is_null() {
                out.push((Namespace::Control, common::ID.to_string(), v));
            }
        }
    }

    if args.include_runtime_id
        && let Some(attr) = node.attribute(Namespace::Control, common::RUNTIME_ID)
    {
        out.push((Namespace::Control, common::RUNTIME_ID.to_string(), attr.value()));
    }

    out
}

// ------------------------- TEXT OUTPUT ---------------------------------------

fn write_wrapped_document_text(path: &Path, roots: &[Arc<dyn UiNode>], args: &SnapshotArgs) -> CliResult<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    write_text_to(&mut writer, roots, args)
}

fn write_single_document_text(path: &Path, root: &Arc<dyn UiNode>, args: &SnapshotArgs) -> CliResult<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    write_tree(&mut writer, root, args, "", true)?;
    Ok(())
}

fn write_text_to<W: Write>(writer: &mut W, roots: &[Arc<dyn UiNode>], args: &SnapshotArgs) -> CliResult<()> {
    let last_idx = roots.len().saturating_sub(1);
    for (i, node) in roots.iter().enumerate() {
        write_tree(writer, node, args, "", i == last_idx)?;
        if i < last_idx {
            writeln!(writer)?;
        }
    }
    Ok(())
}

fn write_tree<W: Write>(
    writer: &mut W,
    node: &Arc<dyn UiNode>,
    args: &SnapshotArgs,
    prefix: &str,
    is_last: bool,
) -> CliResult<()> {
    let connector = if prefix.is_empty() {
        ""
    } else if is_last {
        "└ "
    } else {
        "├ "
    };
    let line_prefix = format!("{}{}", prefix, connector);

    // Determine label and inline extras (Id/RuntimeId)
    let ns = node.namespace();
    let role = node.role();
    let name = node.name();

    let ns_prefix = if ns == Namespace::Control { String::new() } else { format!("{}:", ns.as_str()) };
    let label_plain =
        if name.is_empty() { format!("{}{}", ns_prefix, role) } else { format!("{}{} \"{}\"", ns_prefix, role, name) };
    let label = label_plain.if_supports_color(Stream::Stdout, |t| t.bold().fg_rgb::<79, 166, 255>().to_string());

    let attrs = collect_attributes(node, args);
    let mut id_opt: Option<String> = None;
    let mut rid_opt: Option<String> = None;
    let mut rest: Vec<(Namespace, String, UiValue)> = Vec::new();
    for (ans, aname, aval) in attrs {
        if ans == Namespace::Control
            && aname == common::ID
            && let UiValue::String(s) = &aval
            && !s.is_empty()
        {
            id_opt = Some(s.clone());
        }
        if ans == Namespace::Control
            && aname == common::RUNTIME_ID
            && let UiValue::String(s) = &aval
        {
            rid_opt = Some(s.clone());
        }
        // In Text-Modus werden alle Attribute gelistet (auch Name/Id), wie gewünscht.
        rest.push((ans, aname, aval));
    }

    let mut extras: Vec<String> = Vec::new();
    if let Some(idv) = id_opt {
        extras.push(format!("Id={}", idv));
    }
    if let Some(rid) = rid_opt {
        extras.push(format!("RuntimeId={}", rid));
    }
    if extras.is_empty() {
        writeln!(writer, "{}{}", line_prefix, label)?;
    } else {
        let extras_join = format!("[{}]", extras.join(", "));
        let extras_text = extras_join.if_supports_color(Stream::Stdout, |t| t.dimmed().to_string());
        writeln!(writer, "{}{} {}", line_prefix, label, extras_text)?;
    }

    // print attributes (beyond defaults) as indented lines
    if !args.no_attrs && !rest.is_empty() {
        // Attribute mit Baum-Markern ausgeben: Vor Attributen kommt ein lokaler "│ "
        // und alle Elternebenen behalten ihre "│  " bzw. "   "-Segmente bei.
        let base = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
        let attr_prefix = format!("{}│ ", base);
        for (ans, aname, aval) in rest {
            let qname_plain = format!("{}:{}", ans.as_str(), aname);
            let qname =
                qname_plain.if_supports_color(Stream::Stdout, |t| t.bold().fg_rgb::<241, 149, 255>().to_string());
            let value_plain = format_value_text(&aval);
            let value = value_plain.if_supports_color(Stream::Stdout, |t| t.fg_rgb::<136, 192, 74>().to_string());
            writeln!(writer, "{}@{} = {}", attr_prefix, qname, value)?;
        }
    }

    // recurse
    let proceed = args.max_depth.map(|m| m > 0).unwrap_or(true);
    if proceed {
        // Compute child prefix
        let child_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
        let children: Vec<Arc<dyn UiNode>> = node.children().collect();
        let last_cidx = children.len().saturating_sub(1);
        let next_depth = args.max_depth.map(|d| d.saturating_sub(1));
        let mut child_args = args.clone();
        child_args.max_depth = next_depth;
        for (idx, child) in children.into_iter().enumerate() {
            write_tree(writer, &child, &child_args, &child_prefix, idx == last_cidx)?;
        }
    }
    Ok(())
}

fn selected(
    ns: Namespace,
    name: &str,
    _value: &UiValue,
    args: &SnapshotArgs,
    include: &[AttrSelector],
    exclude: &[AttrSelector],
) -> bool {
    // Determine baseline set
    let mut baseline = match args.attrs {
        AttrMode::Default => name == common::NAME || name == common::ID, // coarse filter, exact enforced later
        AttrMode::All => true,
        AttrMode::List => false,
    };
    if matches!(args.attrs, AttrMode::List) {
        baseline = include.iter().any(|s| s.matches(ns, name));
    }
    if !include.is_empty() && !matches!(args.attrs, AttrMode::List) {
        // If include is set in other modes, treat it as an additive allowlist
        baseline = baseline || include.iter().any(|s| s.matches(ns, name));
    }
    // Apply exclude last
    if exclude.iter().any(|s| s.matches(ns, name)) {
        return false;
    }
    baseline
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::runtime;
    use rstest::rstest;
    use serial_test::serial;
    use std::fs;
    use tempfile::tempdir;

    #[rstest]
    #[serial]
    fn snapshot_writes_single_file(runtime: Runtime) {
        let dir = tempdir().expect("temp");
        let path = dir.path().join("snapshot.xml");
        let args = SnapshotArgs {
            expression: "//control:Window".into(),
            output: Some(path.clone()),
            split: None,
            max_depth: Some(1),
            attrs: AttrMode::Default,
            include: vec![],
            exclude: vec![],
            exclude_derived: false,
            include_runtime_id: true,
            pretty: true,
            format: SnapshotFormat::Xml,
            no_attrs: false,
            no_color: true,
        };
        let msg = run(&runtime, &args).expect("snapshot run");
        assert!(msg.contains("Saved snapshot"));
        let xml = fs::read_to_string(&path).expect("read xml");
        assert!(xml.contains("<snapshot"));
        // Default namespace for control → element without prefix
        assert!(xml.contains("<Window "));
        assert!(xml.contains("control:Name"));
    }

    #[rstest]
    #[serial]
    fn snapshot_split_writes_multiple_files(runtime: Runtime) {
        let dir = tempdir().expect("temp");
        let prefix = dir.path().join("roots");
        let args = SnapshotArgs {
            expression: "//control:Button".into(),
            output: None,
            split: Some(prefix.clone()),
            max_depth: Some(0),
            attrs: AttrMode::All,
            include: vec![],
            exclude: vec![],
            exclude_derived: true,
            include_runtime_id: false,
            pretty: false,
            format: SnapshotFormat::Xml,
            no_attrs: false,
            no_color: true,
        };
        let msg = run(&runtime, &args).expect("snapshot run");
        assert!(msg.contains("Saved"));
        let f1 = prefix.with_file_name("roots-001.xml");
        let f2 = prefix.with_file_name("roots-002.xml");
        assert!(f1.exists() || f2.exists());
    }
}
