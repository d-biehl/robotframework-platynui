use super::common::{as_string, collapse_whitespace};
use crate::engine::runtime::{CallCtx, Error};
use crate::xdm::{XdmItem, XdmSequence};
use std::collections::HashSet;

fn is_ncname_ascii(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    match chars.next().unwrap() {
        'A'..='Z' | 'a'..='z' | '_' => {}
        _ => return false,
    }
    for ch in chars {
        match ch {
            'A'..='Z' | 'a'..='z' | '_' | '0'..='9' | '-' | '.' => {}
            _ => return false,
        }
    }
    true
}

fn topmost_ancestor<N: crate::model::XdmNode + Clone>(mut n: N) -> N {
    while let Some(p) = n.parent() {
        n = p;
    }
    n
}

pub(super) fn id_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let mut tokens: HashSet<String> = HashSet::new();
    for it in &args[0] {
        let s = match it {
            XdmItem::Atomic(a) => as_string(a),
            XdmItem::Node(n) => n.string_value(),
        };
        let collapsed = collapse_whitespace(&s);
        for t in collapsed.split(' ') {
            if !t.is_empty() && is_ncname_ascii(t) {
                tokens.insert(t.to_string());
            }
        }
    }
    if tokens.is_empty() {
        return Ok(vec![]);
    }
    let start_node_opt = if args.len() == 2 && !args[1].is_empty() {
        Some(args[1][0].clone())
    } else {
        ctx.dyn_ctx.context_item.clone()
    };
    find_elements_with_id(ctx, start_node_opt, &tokens)
}

fn find_elements_with_id<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    start_node_opt: Option<XdmItem<N>>,
    tokens: &HashSet<String>,
) -> Result<XdmSequence<N>, Error> {
    let mut out: XdmSequence<N> = Vec::new();
    let Some(XdmItem::Node(start)) = start_node_opt else {
        return Ok(out);
    };
    let root = topmost_ancestor(start);
    let mut stack: Vec<N> = vec![root.clone()];
    while let Some(node) = stack.pop() {
        let children = node.children();
        for c in children.iter().rev() {
            stack.push(c.clone());
        }
        if matches!(node.kind(), crate::model::NodeKind::Element) {
            let mut has_match = false;
            for a in node.attributes() {
                if let Some(q) = a.name() {
                    let is_xml_id = q.local == "id"
                        && (q.prefix.as_deref() == Some("xml")
                            || q.ns_uri.as_deref() == Some(crate::consts::XML_URI));
                    let is_plain_id = q.local == "id" && q.prefix.is_none() && q.ns_uri.is_none();
                    if is_xml_id || is_plain_id {
                        let v = a.string_value();
                        if tokens.contains(&v) {
                            has_match = true;
                            break;
                        }
                    }
                }
            }
            if has_match {
                out.push(XdmItem::Node(node.clone()));
            }
        }
    }
    Ok(out)
}

pub(super) fn element_with_id_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let mut tokens: HashSet<String> = HashSet::new();
    for it in &args[0] {
        let s = match it {
            XdmItem::Atomic(a) => as_string(a),
            XdmItem::Node(n) => n.string_value(),
        };
        let collapsed = collapse_whitespace(&s);
        for t in collapsed.split(' ') {
            if !t.is_empty() && is_ncname_ascii(t) {
                tokens.insert(t.to_string());
            }
        }
    }
    if tokens.is_empty() {
        return Ok(vec![]);
    }
    let start_node_opt = if args.len() == 2 && !args[1].is_empty() {
        Some(args[1][0].clone())
    } else {
        ctx.dyn_ctx.context_item.clone()
    };
    find_elements_with_id(ctx, start_node_opt, &tokens)
}

pub(super) fn idref_fn<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    let mut ids: HashSet<String> = HashSet::new();
    for it in &args[0] {
        let s = match it {
            XdmItem::Atomic(a) => as_string(a),
            XdmItem::Node(n) => n.string_value(),
        };
        if is_ncname_ascii(&s) {
            ids.insert(s);
        }
    }
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let start_node_opt = if args.len() == 2 && !args[1].is_empty() {
        Some(args[1][0].clone())
    } else {
        ctx.dyn_ctx.context_item.clone()
    };
    let Some(XdmItem::Node(start)) = start_node_opt else {
        return Ok(vec![]);
    };
    let root = topmost_ancestor(start);
    let mut out: XdmSequence<N> = Vec::new();
    let mut stack: Vec<N> = vec![root.clone()];
    while let Some(node) = stack.pop() {
        let children = node.children();
        for c in children.iter().rev() {
            stack.push(c.clone());
        }
        if matches!(node.kind(), crate::model::NodeKind::Element) {
            for a in node.attributes() {
                if let Some(q) = a.name() {
                    let is_xml_id = q.local == "id"
                        && (q.prefix.as_deref() == Some("xml")
                            || q.ns_uri.as_deref() == Some(crate::consts::XML_URI));
                    let is_plain_id = q.local == "id" && q.prefix.is_none() && q.ns_uri.is_none();
                    if is_xml_id || is_plain_id {
                        continue;
                    }
                }
                let v = a.string_value();
                let collapsed = collapse_whitespace(&v);
                for t in collapsed.split(' ') {
                    if !t.is_empty() && ids.contains(t) {
                        out.push(XdmItem::Node(a.clone()));
                        break;
                    }
                }
            }
        }
    }
    Ok(out)
}
