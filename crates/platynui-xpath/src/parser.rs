use pest::Parser;
use pest::error::Error;
use pest::iterators::Pair;

pub mod ast;

#[derive(pest_derive::Parser)]
#[grammar = "xpath2.pest"]
pub struct XPathParser;

impl XPathParser {
    // Test-only parse wrapper moved to tests/parser/test_utils.rs

    /// Build the internal AST for evaluation from the XPath input.
    pub fn parse_to_ast(input: &str) -> Result<ast::Expr, Error<Rule>> {
        let mut pairs = Self::parse(Rule::xpath, input)?;
        let pair = pairs.next().expect("xpath root");
        debug_assert_eq!(pair.as_rule(), Rule::xpath);
        let inner = pair.into_inner().next().expect("expr root");
        if let Some(e) = Self::build_expr(&inner) { return Ok(e); }
        Err(Error::new_from_span(pest::error::ErrorVariant::CustomError { message: "unsupported expression for AST builder".into() }, inner.as_span()))
    }

    // Test-only AST extractors moved to tests/parser/test_utils.rs

    // Test-only parse_expression_node moved to tests/parser/test_utils.rs

    /// Walk down a pair to the first terminal token rule (e.g., OP_PLUS, K_AND)
    fn first_token_rule(pair: &Pair<Rule>) -> Rule {
        let mut current = pair.clone();
        loop {
            let mut inner = current.clone().into_inner();
            if let Some(next) = inner.next() {
                current = next;
            } else {
                return current.as_rule();
            }
        }
    }

    // ====== Internal AST builder (subset) ======
    fn build_expr(pair: &Pair<Rule>) -> Option<ast::Expr> {
        match pair.as_rule() {
            Rule::expr_single | Rule::or_expr | Rule::and_expr | Rule::comparison_expr
            | Rule::range_expr | Rule::additive_expr | Rule::multiplicative_expr | Rule::unary_expr
            | Rule::union_expr | Rule::intersect_except_expr
            | Rule::instanceof_expr | Rule::treat_expr | Rule::castable_expr | Rule::cast_expr
            | Rule::value_expr | Rule::path_expr | Rule::relative_path_expr | Rule::postfix_expr
            | Rule::primary_expr | Rule::parenthesized_expr => {
                Self::build_binary_chain(pair)
            }
            Rule::expr => {
                for p in pair.clone().into_inner() {
                    if let Some(e) = Self::build_expr(&p) { return Some(e); }
                }
                None
            }
            Rule::string_literal => {
                let mut inners = pair.clone().into_inner();
                if let Some(content) = inners.next() {
                    let raw = content.as_str();
                    let s = match content.as_rule() {
                        Rule::dbl_string_inner => raw.replace("\"\"", "\""),
                        Rule::sgl_string_inner => raw.replace("''", "'"),
                        _ => raw.to_string(),
                    };
                    return Some(ast::Expr::Literal(ast::Literal::String(s)));
                }
                Some(ast::Expr::Literal(ast::Literal::String(String::new())))
            }
            Rule::integer_literal => {
                let s = pair.as_str();
                let v = s.parse::<i64>().ok()?;
                Some(ast::Expr::Literal(ast::Literal::Integer(v)))
            }
            Rule::decimal_literal | Rule::double_literal => {
                let s = pair.as_str();
                let v = s.parse::<f64>().ok()?;
                Some(ast::Expr::Literal(ast::Literal::Double(v)))
            }
            Rule::function_call => {
                let mut inners = pair.clone().into_inner();
                let name_pair = inners.next()?;
                let qn = ast_qname_from_str(name_pair.as_str());
                let mut args: Vec<ast::Expr> = Vec::new();
                for a in inners { if let Some(e) = Self::build_expr(&a) { args.push(e); } }
                Some(ast::Expr::FunctionCall { name: qn, args })
            }
            Rule::context_item_expr => Some(ast::Expr::ContextItem),
            Rule::var_ref => {
                let mut inners = pair.clone().into_inner();
                let q = inners.next()?; // var_name -> qname
                let qn = ast_qname_from_str(q.as_str());
                Some(ast::Expr::VarRef(qn))
            }
            _ => None,
        }
    }

    fn build_binary_chain(pair: &Pair<Rule>) -> Option<ast::Expr> {
        match pair.as_rule() {
            Rule::path_expr => Self::build_path_expr(pair),
            Rule::value_expr => {
                // value_expr = { path_expr }
                let mut inn = pair.clone().into_inner();
                let inner = inn.next()?;
                Self::build_expr(&inner)
            }
            // Rule::postfix_expr is processed in path building or via recursion
            Rule::absolute_path => Self::build_absolute_path(pair),
            Rule::relative_path_expr => Self::build_relative_path(pair),
            Rule::union_expr => {
                // intersect_except_expr (union_op intersect_except_expr)*
                let mut inn = pair.clone().into_inner();
                let mut expr = Self::build_expr(&inn.next()?)?;
                while let Some(op_or_next) = inn.next() {
                    let token = Self::first_token_rule(&op_or_next);
                    if token == Rule::K_UNION || token == Rule::OP_PIPE {
                        let right = inn.next()?;
                        let r = Self::build_expr(&right)
                            .or_else(|| Self::find_rule(&right, Rule::path_expr).and_then(|p| Self::build_path_expr(&p)))
                            .or_else(|| Self::find_rule(&right, Rule::absolute_path).and_then(|p| Self::build_absolute_path(&p)))
                            .or_else(|| Self::find_rule(&right, Rule::relative_path_expr).and_then(|p| Self::build_relative_path(&p)))?;
                        expr = ast::Expr::SetOp { left: Box::new(expr), op: ast::SetOp::Union, right: Box::new(r) };
                    }
                }
                Some(expr)
            }
            Rule::intersect_except_expr => {
                // instanceof_expr (intersect_except_op instanceof_expr)*
                let mut inn = pair.clone().into_inner();
                let mut expr = Self::build_expr(&inn.next()?)?;
                while let Some(op_or_next) = inn.next() {
                    let token = Self::first_token_rule(&op_or_next);
                    let right = inn.next()?;
                    let r = Self::build_expr(&right)
                        .or_else(|| Self::find_rule(&right, Rule::path_expr).and_then(|p| Self::build_path_expr(&p)))
                        .or_else(|| Self::find_rule(&right, Rule::absolute_path).and_then(|p| Self::build_absolute_path(&p)))
                        .or_else(|| Self::find_rule(&right, Rule::relative_path_expr).and_then(|p| Self::build_relative_path(&p)))?;
                    expr = match token {
                        Rule::K_INTERSECT => ast::Expr::SetOp { left: Box::new(expr), op: ast::SetOp::Intersect, right: Box::new(r) },
                        Rule::K_EXCEPT => ast::Expr::SetOp { left: Box::new(expr), op: ast::SetOp::Except, right: Box::new(r) },
                        _ => expr,
                    };
                }
                Some(expr)
            }
            Rule::parenthesized_expr => {
                let mut inners = pair.clone().into_inner();
                if let Some(inner) = inners.next() { return Self::build_expr(&inner); }
                Some(ast::Expr::Literal(ast::Literal::EmptySequence))
            }
            Rule::or_expr => Self::fold_chain(pair, Rule::and_expr, |op| match op { Rule::K_OR => Some(ast::BinaryOp::Or), _ => None }),
            Rule::and_expr => Self::fold_chain(pair, Rule::comparison_expr, |op| match op { Rule::K_AND => Some(ast::BinaryOp::And), _ => None }),
            Rule::comparison_expr => {
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                if let Some(op_pair) = inners.next() {
                    let right = inners.next()?;
                    let l = Self::build_expr(&left)?;
                    let r = Self::build_expr(&right)?;
                    let token = Self::first_token_rule(&op_pair);
                    use ast::{Expr, GeneralComp as GC, NodeComp as NC, ValueComp as VC};
                    let e = match token {
                        Rule::OP_EQ => Expr::GeneralComparison { left: Box::new(l), op: GC::Eq, right: Box::new(r) },
                        Rule::OP_NE => Expr::GeneralComparison { left: Box::new(l), op: GC::Ne, right: Box::new(r) },
                        Rule::OP_LT => Expr::GeneralComparison { left: Box::new(l), op: GC::Lt, right: Box::new(r) },
                        Rule::OP_LTE => Expr::GeneralComparison { left: Box::new(l), op: GC::Le, right: Box::new(r) },
                        Rule::OP_GT => Expr::GeneralComparison { left: Box::new(l), op: GC::Gt, right: Box::new(r) },
                        Rule::OP_GTE => Expr::GeneralComparison { left: Box::new(l), op: GC::Ge, right: Box::new(r) },
                        Rule::K_EQ => Expr::ValueComparison { left: Box::new(l), op: VC::Eq, right: Box::new(r) },
                        Rule::K_NE => Expr::ValueComparison { left: Box::new(l), op: VC::Ne, right: Box::new(r) },
                        Rule::K_LT => Expr::ValueComparison { left: Box::new(l), op: VC::Lt, right: Box::new(r) },
                        Rule::K_LE => Expr::ValueComparison { left: Box::new(l), op: VC::Le, right: Box::new(r) },
                        Rule::K_GT => Expr::ValueComparison { left: Box::new(l), op: VC::Gt, right: Box::new(r) },
                        Rule::K_GE => Expr::ValueComparison { left: Box::new(l), op: VC::Ge, right: Box::new(r) },
                        Rule::K_IS => Expr::NodeComparison { left: Box::new(l), op: NC::Is, right: Box::new(r) },
                        Rule::OP_PRECEDES => Expr::NodeComparison { left: Box::new(l), op: NC::Precedes, right: Box::new(r) },
                        Rule::OP_FOLLOWS => Expr::NodeComparison { left: Box::new(l), op: NC::Follows, right: Box::new(r) },
                        _ => return None,
                    };
                    Some(e)
                } else {
                    Self::build_expr(&left)
                }
            }
            Rule::range_expr => {
                // additive_expr (K_TO additive_expr)?
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                let l = Self::build_expr(&left)?;
                if let Some(_to_tok) = inners.next() {
                    let right = inners.next()?;
                    let r = Self::build_expr(&right)?;
                    Some(ast::Expr::Range { start: Box::new(l), end: Box::new(r) })
                } else {
                    Some(l)
                }
            }
            Rule::additive_expr => Self::fold_chain(pair, Rule::multiplicative_expr, |op| match op { Rule::OP_PLUS => Some(ast::BinaryOp::Add), Rule::OP_MINUS => Some(ast::BinaryOp::Sub), _ => None }),
            Rule::multiplicative_expr => Self::fold_chain(pair, Rule::union_expr, |op| match op { Rule::OP_STAR => Some(ast::BinaryOp::Mul), Rule::K_DIV => Some(ast::BinaryOp::Div), Rule::K_IDIV => Some(ast::BinaryOp::IDiv), Rule::K_MOD => Some(ast::BinaryOp::Mod), _ => None }),
            Rule::unary_expr => {
                let mut inners = pair.clone().into_inner();
                let mut signs: Vec<ast::UnarySign> = Vec::new();
                loop {
                    if let Some(n) = inners.clone().next() {
                        match n.as_rule() {
                            Rule::OP_MINUS => { signs.push(ast::UnarySign::Minus); let _ = inners.next(); continue; }
                            Rule::OP_PLUS => { signs.push(ast::UnarySign::Plus); let _ = inners.next(); continue; }
                            _ => {}
                        }
                    }
                    break;
                }
                let value = inners.next()?;
                let mut expr = Self::build_expr(&value)?;
                for s in signs { if matches!(s, ast::UnarySign::Minus) { expr = ast::Expr::Binary { left: Box::new(ast::Expr::Literal(ast::Literal::Integer(0))), op: ast::BinaryOp::Sub, right: Box::new(expr) }; } }
                Some(expr)
            }
            _ => {
                for inner in pair.clone().into_inner() { if let Some(e) = Self::build_expr(&inner) { return Some(e); } }
                None
            }
        }
    }

    fn fold_chain<F>(pair: &Pair<Rule>, expected_left_rule: Rule, map_op: F) -> Option<ast::Expr>
    where
        F: Fn(Rule) -> Option<ast::BinaryOp>,
    {
        let mut inners = pair.clone().into_inner();
        let mut expr = Self::build_expr(&inners.next()?)?;
        while let Some(op_or_next) = inners.next() {
            let token = Self::first_token_rule(&op_or_next);
            if let Some(op) = map_op(token) {
                let right = inners.next()?;
                let right_expr = Self::build_expr(&right)?;
                expr = ast::Expr::Binary { left: Box::new(expr), op, right: Box::new(right_expr) };
            } else if op_or_next.as_rule() == expected_left_rule {
                if let Some(inner_e) = Self::build_expr(&op_or_next) { expr = inner_e; }
            }
        }
        Some(expr)
    }
}

fn ast_qname_from_str(s: &str) -> ast::QName {
    if let Some(idx) = s.find(':') {
        ast::QName { prefix: Some(s[..idx].to_string()), local: s[idx + 1..].to_string(), ns_uri: None }
    } else {
        ast::QName { prefix: None, local: s.to_string(), ns_uri: None }
    }
}

impl XPathParser {
    fn find_rule<'a>(pair: &Pair<'a, Rule>, rule: Rule) -> Option<Pair<'a, Rule>> {
        if pair.as_rule() == rule { return Some(pair.clone()); }
        for inner in pair.clone().into_inner() {
            if let Some(p) = Self::find_rule(&inner, rule) { return Some(p); }
        }
        None
    }
    pub(crate) fn build_path_expr(pair: &Pair<Rule>) -> Option<ast::Expr> {
        debug_assert_eq!(pair.as_rule(), Rule::path_expr);
        // debug: eprintln!("build_path_expr: '{}'", pair.as_str());
        let mut inners = pair.clone().into_inner();
        let first = inners.next()?;
        match first.as_rule() {
            Rule::absolute_path => Self::build_absolute_path(&first),
            Rule::relative_path_expr => {
                // debug: eprintln!("  relative_path_expr: '{}'", first.as_str());
                // Special-case: a relative_path_expr consisting of a single postfix_expr
                // (e.g., a literal like 2, a function call, a variable, or ".") should be
                // treated as that primary expression rather than a Path.
                let mut rel_in = first.clone().into_inner();
                let only = rel_in.next()?; // step_expr
                if rel_in.next().is_none() {
                    let mut step_in = only.clone().into_inner();
                    if let Some(step_first) = step_in.next() {
                        if step_first.as_rule() == Rule::postfix_expr {
                            // Try to extract the primary literal/var/parens directly
                            if let Some(lit) = Self::find_rule(&step_first, Rule::string_literal)
                                .or_else(|| Self::find_rule(&step_first, Rule::integer_literal))
                                .or_else(|| Self::find_rule(&step_first, Rule::decimal_literal))
                                .or_else(|| Self::find_rule(&step_first, Rule::double_literal))
                                .or_else(|| Self::find_rule(&step_first, Rule::var_ref))
                                .or_else(|| Self::find_rule(&step_first, Rule::parenthesized_expr))
                                .or_else(|| Self::find_rule(&step_first, Rule::context_item_expr))
                            {
                                if let Some(expr) = Self::build_expr(&lit) {
                                    // debug: eprintln!("  special-case postfix as primary: '{}'", step_first.as_str());
                                    return Some(expr);
                                }
                            } else if let Some(expr) = Self::build_expr(&step_first) {
                                // Fallback: try generic builder on postfix
                                // debug: eprintln!("  fallback build on postfix: '{}'", step_first.as_str());
                                return Some(expr);
                            }
                        }
                    }
                }
                Self::build_relative_path(&first)
            }
            _ => None,
        }
    }

    pub(crate) fn build_absolute_path(pair: &Pair<Rule>) -> Option<ast::Expr> {
        // absolute_path = { (OP_DSLASH ~ relative_path_expr) | (OP_SLASH ~ relative_path_expr?) }
        let mut inners = pair.clone().into_inner();
        let first = inners.next()?;
        match first.as_rule() {
            Rule::OP_DSLASH => {
                let rel = inners.next()?;
                let steps = Self::collect_steps_from_relative(&rel)?;
                let mut all_steps = vec![ast::Step { axis: ast::Axis::DescendantOrSelf, test: ast::NodeTest::Kind(ast::KindTest::AnyKind), predicates: vec![] }];
                all_steps.extend(steps);
                Some(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Root, steps: all_steps }))
            }
            Rule::OP_SLASH => {
                if let Some(rel) = inners.next() {
                    let steps = Self::collect_steps_from_relative(&rel)?;
                    Some(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Root, steps }))
                } else {
                    Some(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Root, steps: vec![] }))
                }
            }
            _ => None,
        }
    }

    pub(crate) fn build_relative_path(pair: &Pair<Rule>) -> Option<ast::Expr> {
        let steps = Self::collect_steps_from_relative(pair)?;
        Some(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Relative, steps }))
    }

    fn collect_steps_from_relative(pair: &Pair<Rule>) -> Option<Vec<ast::Step>> {
        debug_assert_eq!(pair.as_rule(), Rule::relative_path_expr);
        let mut out: Vec<ast::Step> = Vec::new();
        let mut inners = pair.clone().into_inner();
        let first_step_pair = inners.next()?; // step_expr
        let mut step = Self::build_step(&first_step_pair)?;
        out.push(step);
        while let Some(op) = inners.next() {
            let op_rule = op.as_rule();
            let next_step_pair = inners.next()?;
            if op_rule == Rule::OP_DSLASH {
                out.push(ast::Step { axis: ast::Axis::DescendantOrSelf, test: ast::NodeTest::Kind(ast::KindTest::AnyKind), predicates: vec![] });
            }
            step = Self::build_step(&next_step_pair)?;
            out.push(step);
        }
        Some(out)
    }

    fn build_step(pair: &Pair<Rule>) -> Option<ast::Step> {
        debug_assert_eq!(pair.as_rule(), Rule::step_expr);
        let mut inners = pair.clone().into_inner();
        let inner = inners.next()?; // axis_step | postfix_expr
        match inner.as_rule() {
            Rule::axis_step => {
                let mut ainners = inner.clone().into_inner();
                let first = ainners.next()?; // reverse_step | forward_step
                let (axis, test) = match first.as_rule() {
                    Rule::forward_step => Self::build_forward_step(&first)?,
                    Rule::reverse_step => Self::build_reverse_step(&first)?,
                    _ => return None,
                };
                let mut preds: Vec<ast::Expr> = Vec::new();
                for pl in ainners {
                    if pl.as_rule() == Rule::predicate_list {
                        let mut built = Self::collect_predicate_list(pl)?;
                        preds.append(&mut built);
                    }
                }
                Some(ast::Step { axis, test, predicates: preds })
            }
            Rule::postfix_expr => {
                // Handle postfix expressions on '.' (self) and also on implicit child name tests like 'a[2]'
                let mut pin = inner.clone().into_inner();
                let prim = pin.next()?; // primary_expr (could conceal '.' or a name in some parses)
                // Try context item '.'
                if let Some(first) = prim.clone().into_inner().next() {
                    if first.as_rule() == Rule::context_item_expr {
                        let preds = if let Some(pl) = pin.next() { if pl.as_rule() == Rule::predicate_list { Self::collect_predicate_list(pl)? } else { vec![] } } else { vec![] };
                        return Some(ast::Step { axis: ast::Axis::SelfAxis, test: ast::NodeTest::Kind(ast::KindTest::AnyKind), predicates: preds });
                    }
                }
                // Try to detect an embedded name_test/qname and treat as child::name()
                if let Some(nt) = Self::find_rule(&prim, Rule::name_test) {
                    let test = Self::build_name_test(&nt)?;
                    let preds = if let Some(pl) = pin.next() { if pl.as_rule() == Rule::predicate_list { Self::collect_predicate_list(pl)? } else { vec![] } } else { vec![] };
                    return Some(ast::Step { axis: ast::Axis::Child, test: ast::NodeTest::Name(test), predicates: preds });
                }
                // Fallback: unsupported postfix target
                None
            }
            _ => None,
        }
    }

    fn collect_predicate_list(pl: Pair<Rule>) -> Option<Vec<ast::Expr>> {
        debug_assert_eq!(pl.as_rule(), Rule::predicate_list);
        let mut preds = Vec::new();
        for pr in pl.into_inner() {
            debug_assert_eq!(pr.as_rule(), Rule::predicate);
            // predicate = LBRACK ~ expr ~ RBRACK
            // Skip bracket tokens (they are captured in the grammar) and find the expr child
            let mut pin = pr.into_inner();
            while let Some(inner) = pin.next() {
                if inner.as_rule() == Rule::expr {
                    let e = Self::build_expr(&inner)?;
                    preds.push(e);
                    break;
                }
            }
        }
        Some(preds)
    }

    // build_attribute_path_from_abbrev/build_comparison_fallback entfernt –
    // comparison_expr und Pfade werden vollständig in build_expr abgebildet.

    // removed build_literal_fallback; literals are handled directly in build_expr



    fn build_forward_step(pair: &Pair<Rule>) -> Option<(ast::Axis, ast::NodeTest)> {
        debug_assert_eq!(pair.as_rule(), Rule::forward_step);
        let mut inners = pair.clone().into_inner();
        let first = inners.next()?;
        match first.as_rule() {
            Rule::forward_axis => {
                let axis = match Self::first_token_rule(&first) {
                    Rule::K_CHILD => ast::Axis::Child,
                    Rule::K_DESCENDANT => ast::Axis::Descendant,
                    Rule::K_ATTRIBUTE => ast::Axis::Attribute,
                    Rule::K_SELF => ast::Axis::SelfAxis,
                    Rule::K_DESCENDANT_OR_SELF => ast::Axis::DescendantOrSelf,
                    Rule::K_FOLLOWING_SIBLING => ast::Axis::FollowingSibling,
                    Rule::K_FOLLOWING => ast::Axis::Following,
                    Rule::K_NAMESPACE => ast::Axis::Namespace,
                    _ => return None,
                };
                let nt_pair = inners.next()?;
                let test = Self::build_node_test(&nt_pair)?;
                Some((axis, test))
            }
            Rule::abbrev_forward_step => {
                let mut s_in = first.clone().into_inner();
                let first_in = s_in.next()?;
                if first_in.as_rule() == Rule::OP_AT {
                    let name_test = s_in.next()?;
                    let name = Self::build_name_test(&name_test)?;
                    Some((ast::Axis::Attribute, ast::NodeTest::Name(name)))
                } else {
                    // node_test
                    let test = Self::build_node_test(&first_in)?;
                    Some((ast::Axis::Child, test))
                }
            }
            _ => None,
        }
    }

    fn build_reverse_step(pair: &Pair<Rule>) -> Option<(ast::Axis, ast::NodeTest)> {
        debug_assert_eq!(pair.as_rule(), Rule::reverse_step);
        let mut inners = pair.clone().into_inner();
        let first = inners.next()?; // reverse_axis
        let axis = match Self::first_token_rule(&first) {
            Rule::K_PARENT => ast::Axis::Parent,
            Rule::K_ANCESTOR => ast::Axis::Ancestor,
            Rule::K_ANCESTOR_OR_SELF => ast::Axis::AncestorOrSelf,
            Rule::K_PRECEDING_SIBLING => ast::Axis::PrecedingSibling,
            Rule::K_PRECEDING => ast::Axis::Preceding,
            _ => return None,
        };
        let nt_pair = inners.next()?;
        let test = Self::build_node_test(&nt_pair)?;
        Some((axis, test))
    }

    fn build_node_test(pair: &Pair<Rule>) -> Option<ast::NodeTest> {
        debug_assert_eq!(pair.as_rule(), Rule::node_test);
        let inner = pair.clone().into_inner().next()?;
        match inner.as_rule() {
            Rule::name_test => Self::build_name_test(&inner).map(ast::NodeTest::Name),
            Rule::kind_test => Self::build_kind_test(&inner).map(ast::NodeTest::Kind),
            _ => None,
        }
    }

    fn build_name_test(pair: &Pair<Rule>) -> Option<ast::NameTest> {
        debug_assert_eq!(pair.as_rule(), Rule::name_test);
        let inner = pair.clone().into_inner().next()?;
        match inner.as_rule() {
            Rule::qname => Some(ast::NameTest::QName(ast_qname_from_str(inner.as_str()))),
            Rule::wildcard_name => {
                let s = inner.as_str();
                if s == "*" {
                    Some(ast::NameTest::Wildcard(ast::WildcardName::Any))
                } else if let Some(rest) = s.strip_prefix("*:") {
                    Some(ast::NameTest::Wildcard(ast::WildcardName::LocalWildcard(rest.to_string())))
                } else if let Some(prefix) = s.strip_suffix(":*") {
                    Some(ast::NameTest::Wildcard(ast::WildcardName::NsWildcard(prefix.to_string())))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn build_kind_test(pair: &Pair<Rule>) -> Option<ast::KindTest> {
        debug_assert_eq!(pair.as_rule(), Rule::kind_test);
        let kind = pair.clone().into_inner().next()?;
        match kind.as_rule() {
            Rule::any_kind_test => Some(ast::KindTest::AnyKind),
            Rule::text_test => Some(ast::KindTest::Text),
            Rule::comment_test => Some(ast::KindTest::Comment),
            Rule::pi_test => Some(ast::KindTest::ProcessingInstruction(None)),
            Rule::element_test => Some(ast::KindTest::Element { name: None, ty: None, nillable: false }),
            Rule::attribute_test => Some(ast::KindTest::Attribute { name: None, ty: None }),
            _ => None,
        }
    }
}

// Test-only AST shape types moved to tests/parser/test_utils.rs
