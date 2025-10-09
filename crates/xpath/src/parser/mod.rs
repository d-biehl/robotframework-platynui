use crate::engine::runtime::{Error, ErrorCode};
use compact_str::CompactString;
use pest::Parser;
use pest::iterators::Pair;
use smallvec::SmallVec; // Optimized small, frequently short-lived collections
use std::borrow::Cow;

pub mod ast;

#[derive(pest_derive::Parser)]
#[grammar = "parser/xpath2.pest"]
pub struct XPathParser;

pub fn parse(input: &str) -> Result<ast::Expr, Error> {
    let mut pairs =
        XPathParser::parse(Rule::xpath, input).map_err(|e| Error::from_code(ErrorCode::XPST0003, format!("{}", e)))?;

    let pair = pairs.next().ok_or_else(|| Error::from_code(ErrorCode::XPST0003, "empty parse"))?;

    debug_assert_eq!(pair.as_rule(), Rule::xpath);

    // xpath = SOI ~ expr ~ EOI
    let mut inner = pair.into_inner();
    let expr_pair = inner.next().ok_or_else(|| Error::from_code(ErrorCode::XPST0003, "missing expr"))?;

    build_expr(expr_pair).map_err(|e| Error::from_code(ErrorCode::XPST0003, e.to_string()))
}

type AstResult<T> = Result<T, ParseAstError>;

// Internal, lightweight AST conversion error
#[derive(Debug)]
struct ParseAstError(String);

impl ParseAstError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl std::fmt::Display for ParseAstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseAstError {}

// --------------
// Rule converters
// --------------

fn extract_qname_deep(pair: Pair<Rule>) -> AstResult<ast::QName> {
    if pair.as_rule() == Rule::qname {
        return build_qname_from_parts(pair);
    }
    let mut stack: Vec<Pair<Rule>> = vec![pair];
    while let Some(p) = stack.pop() {
        if p.as_rule() == Rule::qname {
            return build_qname_from_parts(p);
        }
        stack.extend(p.into_inner());
    }
    Err(ParseAstError::new("expected qname in structure"))
}

fn build_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    match pair.as_rule() {
        Rule::expr => build_comma_sequence(pair),
        Rule::expr_single => {
            build_expr(pair.into_inner().next().ok_or_else(|| ParseAstError::new("missing expr_single inner"))?)
        }
        Rule::for_expr => build_for_expr(pair),
        Rule::let_expr => build_let_expr(pair),
        Rule::quantified_expr => build_quantified_expr(pair),
        Rule::if_expr => build_if_expr(pair),
        Rule::or_expr => build_or_expr(pair),
        Rule::comparison_expr => build_comparison_expr(pair),
        Rule::range_expr => build_range_expr(pair),
        Rule::additive_expr => build_additive_expr(pair),
        Rule::multiplicative_expr => build_multiplicative_expr(pair),
        Rule::union_expr => build_union_like(pair),
        Rule::intersect_except_expr => build_intersect_except_like(pair),
        Rule::instanceof_expr => build_instanceof_like(pair),
        Rule::treat_expr => build_treat_like(pair),
        Rule::castable_expr => build_castable_like(pair),
        Rule::cast_expr => build_cast_like(pair),
        Rule::unary_expr => build_unary_expr(pair),
        Rule::value_expr => {
            build_expr(pair.into_inner().next().ok_or_else(|| ParseAstError::new("missing value_expr inner"))?)
        }
        Rule::path_expr => build_path_expr(pair),
        Rule::absolute_path => build_path_expr_from_absolute(pair),
        Rule::relative_path_expr => build_path_expr_from_relative(pair, None),
        Rule::filter_expr => build_filter_expr(pair),
        Rule::axis_step => build_axis_step_as_expr(pair),
        Rule::primary_expr => build_primary_expr(pair),
        Rule::var_ref => build_var_ref(pair),
        Rule::function_call => build_function_call(pair),
        Rule::literal => build_literal(pair),
        Rule::parenthesized_expr => build_parenthesized(pair),
        Rule::context_item_expr => Ok(ast::Expr::ContextItem),
        _ => Err(ParseAstError::new(format!("unhandled rule in build_expr: {:?}", pair.as_rule()))),
    }
}

fn build_qname_from_parts(pair: Pair<Rule>) -> AstResult<ast::QName> {
    // Accept a qname or wrappers like var_name/element_name/attribute_name/type_name
    let qpair = if pair.as_rule() == Rule::qname {
        pair
    } else {
        pair.into_inner()
            .find(|p| p.as_rule() == Rule::qname)
            .ok_or_else(|| ParseAstError::new("expected qname inside wrapper"))?
    };
    let mut prefix: Option<String> = None;
    let mut local: Option<String> = None;
    for p in qpair.into_inner() {
        match p.as_rule() {
            Rule::qname_prefix => prefix = Some(p.as_str().to_string()),
            Rule::qname_local => local = Some(p.as_str().to_string()),
            _ => {}
        }
    }
    let local = local.ok_or_else(|| ParseAstError::new("qname missing local part"))?;
    Ok(ast::QName { prefix, local, ns_uri: None })
}

fn build_qname_from_function_qname(s: &str) -> ast::QName {
    if let Some((pre, loc)) = s.split_once(':') {
        ast::QName { prefix: Some(pre.to_string()), local: loc.to_string(), ns_uri: None }
    } else {
        ast::QName { prefix: None, local: s.to_string(), ns_uri: None }
    }
}

fn build_literal(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    debug_assert_eq!(pair.as_rule(), Rule::literal);
    let inner = pair.into_inner().next().ok_or_else(|| ParseAstError::new("missing literal inner"))?;
    let lit = match inner.as_rule() {
        Rule::numeric_literal => build_numeric_literal(inner)?,
        Rule::string_literal => ast::Literal::String(CompactString::from(unescape_string_literal(inner))),
        _ => return Err(ParseAstError::new("unknown literal")),
    };
    Ok(ast::Expr::Literal(lit))
}

fn unescape_string_literal(pair: Pair<Rule>) -> Cow<str> {
    // Prefer inner capture when present; otherwise handle atomic string rules
    let mut node = pair;
    loop {
        let mut it = node.clone().into_inner();
        if let Some(child) = it.next() {
            node = child;
        } else {
            break;
        }
    }
    match node.as_rule() {
        Rule::dbl_string_inner => {
            let s = node.as_str();
            if s.contains("\"\"") { Cow::Owned(s.replace("\"\"", "\"")) } else { Cow::Borrowed(s) }
        }
        Rule::sgl_string_inner => {
            let s = node.as_str();
            if s.contains("''") { Cow::Owned(s.replace("''", "'")) } else { Cow::Borrowed(s) }
        }
        Rule::dbl_string => {
            let s = node.as_str();
            if s.len() >= 2 {
                let inner = &s[1..s.len() - 1];
                if inner.contains("\"\"") { Cow::Owned(inner.replace("\"\"", "\"")) } else { Cow::Borrowed(inner) }
            } else {
                Cow::Borrowed("")
            }
        }
        Rule::sgl_string => {
            let s = node.as_str();
            if s.len() >= 2 {
                let inner = &s[1..s.len() - 1];
                if inner.contains("''") { Cow::Owned(inner.replace("''", "'")) } else { Cow::Borrowed(inner) }
            } else {
                Cow::Borrowed("")
            }
        }
        _ => Cow::Borrowed(""),
    }
}

fn build_numeric_literal(pair: Pair<Rule>) -> AstResult<ast::Literal> {
    debug_assert_eq!(pair.as_rule(), Rule::numeric_literal);
    let inner = pair.into_inner().next().ok_or_else(|| ParseAstError::new("missing numeric literal inner"))?;
    let text = inner.as_str();
    match inner.as_rule() {
        Rule::integer_literal => {
            let v: i64 = text.parse().map_err(|e| ParseAstError::new(format!("bad integer: {e}")))?;
            Ok(ast::Literal::Integer(v))
        }
        Rule::decimal_literal => {
            let v: f64 = text.parse().map_err(|e| ParseAstError::new(format!("bad decimal: {e}")))?;
            Ok(ast::Literal::Decimal(v))
        }
        Rule::double_literal => {
            let v: f64 = text.parse().map_err(|e| ParseAstError::new(format!("bad double: {e}")))?;
            Ok(ast::Literal::Double(v))
        }
        _ => Err(ParseAstError::new("unknown numeric literal")),
    }
}

fn build_parenthesized(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // parenthesized_expr = LPAR ~ expr? ~ RPAR
    let mut inner = pair.into_inner();
    // Skip LPAR if present
    let first = inner.next();
    let maybe_expr = match first {
        Some(p) if p.as_rule() == Rule::LPAR => inner.next(),
        other => other,
    };
    if let Some(e) = maybe_expr {
        if e.as_rule() == Rule::expr { build_expr(e) } else { Ok(ast::Expr::Sequence(vec![])) }
    } else {
        Ok(ast::Expr::Sequence(vec![]))
    }
}

fn build_var_ref(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let q = pair.into_inner().next().ok_or_else(|| ParseAstError::new("var_ref without name"))?;
    debug_assert_eq!(q.as_rule(), Rule::var_name);
    let qname =
        build_qname_from_parts(q.into_inner().next().ok_or_else(|| ParseAstError::new("var_name missing qname"))?)?; // var_name -> qname
    Ok(ast::Expr::VarRef(qname))
}

fn build_function_call(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let mut it = pair.into_inner();
    let fn_name = it.next().ok_or_else(|| ParseAstError::new("function_call missing name"))?; // function_qname
    let qname = build_qname_from_function_qname(fn_name.as_str());
    // Next token is LPAR; but Pest groups arguments directly as expr_single
    let mut args = Vec::with_capacity(4); // Most functions have 1-4 args
    for p in it {
        if p.as_rule() == Rule::expr_single {
            args.push(build_expr(p)?);
        }
    }
    Ok(ast::Expr::FunctionCall { name: qname, args })
}

fn build_unary_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // Unary plus/minus chains are almost always <= 2, keep inline on stack
    let mut signs: SmallVec<[ast::UnarySign; 2]> = SmallVec::new();
    let mut value: Option<Pair<Rule>> = None;
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::value_expr => {
                value = Some(p);
            }
            Rule::OP_PLUS => signs.push(ast::UnarySign::Plus),
            Rule::OP_MINUS => signs.push(ast::UnarySign::Minus),
            _ => {}
        }
    }
    let mut expr = build_expr(value.ok_or_else(|| ParseAstError::new("unary missing value"))?)?;
    for s in signs.into_iter().rev() {
        // apply last sign closest to value
        expr = ast::Expr::Unary { sign: s, expr: Box::new(expr) };
    }
    Ok(expr)
}

fn fold_left(
    mut items: impl Iterator<Item = ast::Expr>,
    ops: impl Iterator<Item = ast::BinaryOp>,
) -> AstResult<ast::Expr> {
    let mut left = items.next().ok_or_else(|| ParseAstError::new("empty expression list for fold"))?;
    for (op, right) in ops.zip(items) {
        left = ast::Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
    }
    Ok(left)
}

fn build_or_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // and_expr (or and_expr)*
    // Typically 2 operands; keep inline
    let mut exprs: SmallVec<[ast::Expr; 2]> = SmallVec::new();
    let mut ops: SmallVec<[ast::BinaryOp; 2]> = SmallVec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::and_expr => exprs.push(build_and_expr(p)?),
            Rule::or_op => ops.push(ast::BinaryOp::Or),
            Rule::K_OR => ops.push(ast::BinaryOp::Or),
            _ => {}
        }
    }
    fold_left(exprs.into_iter(), ops.into_iter())
}

fn build_and_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // comparison_expr (and comparison_expr)*
    let mut exprs: SmallVec<[ast::Expr; 2]> = SmallVec::new();
    let mut ops: SmallVec<[ast::BinaryOp; 2]> = SmallVec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::comparison_expr => exprs.push(build_comparison_expr(p)?),
            Rule::and_op => ops.push(ast::BinaryOp::And),
            Rule::K_AND => ops.push(ast::BinaryOp::And),
            _ => {}
        }
    }
    fold_left(exprs.into_iter(), ops.into_iter())
}

fn build_comparison_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let mut it = pair.into_inner();
    let left_pair = it.next().ok_or_else(|| ParseAstError::new("missing left operand in comparison_expr"))?;
    if let Some(op_pair) = it.next() {
        let right_pair = it.next().ok_or_else(|| ParseAstError::new("missing right operand in comparison_expr"))?;
        let left = build_range_expr(left_pair)?;
        let right = build_range_expr(right_pair)?;
        // Flatten to terminal token for operator
        let mut token = op_pair;
        loop {
            let mut it2 = token.clone().into_inner();
            if let Some(n) = it2.next() {
                token = n;
            } else {
                break;
            }
        }
        match token.as_rule() {
            // Node comps
            Rule::OP_PRECEDES => Ok(ast::Expr::NodeComparison {
                left: Box::new(left),
                op: ast::NodeComp::Precedes,
                right: Box::new(right),
            }),
            Rule::OP_FOLLOWS => Ok(ast::Expr::NodeComparison {
                left: Box::new(left),
                op: ast::NodeComp::Follows,
                right: Box::new(right),
            }),
            Rule::K_IS => {
                Ok(ast::Expr::NodeComparison { left: Box::new(left), op: ast::NodeComp::Is, right: Box::new(right) })
            }
            // Value comps
            Rule::K_EQ => {
                Ok(ast::Expr::ValueComparison { left: Box::new(left), op: ast::ValueComp::Eq, right: Box::new(right) })
            }
            Rule::K_NE => {
                Ok(ast::Expr::ValueComparison { left: Box::new(left), op: ast::ValueComp::Ne, right: Box::new(right) })
            }
            Rule::K_LT => {
                Ok(ast::Expr::ValueComparison { left: Box::new(left), op: ast::ValueComp::Lt, right: Box::new(right) })
            }
            Rule::K_LE => {
                Ok(ast::Expr::ValueComparison { left: Box::new(left), op: ast::ValueComp::Le, right: Box::new(right) })
            }
            Rule::K_GT => {
                Ok(ast::Expr::ValueComparison { left: Box::new(left), op: ast::ValueComp::Gt, right: Box::new(right) })
            }
            Rule::K_GE => {
                Ok(ast::Expr::ValueComparison { left: Box::new(left), op: ast::ValueComp::Ge, right: Box::new(right) })
            }
            // General comps
            Rule::OP_EQ => Ok(ast::Expr::GeneralComparison {
                left: Box::new(left),
                op: ast::GeneralComp::Eq,
                right: Box::new(right),
            }),
            Rule::OP_NE => Ok(ast::Expr::GeneralComparison {
                left: Box::new(left),
                op: ast::GeneralComp::Ne,
                right: Box::new(right),
            }),
            Rule::OP_LT => Ok(ast::Expr::GeneralComparison {
                left: Box::new(left),
                op: ast::GeneralComp::Lt,
                right: Box::new(right),
            }),
            Rule::OP_LTE => Ok(ast::Expr::GeneralComparison {
                left: Box::new(left),
                op: ast::GeneralComp::Le,
                right: Box::new(right),
            }),
            Rule::OP_GT => Ok(ast::Expr::GeneralComparison {
                left: Box::new(left),
                op: ast::GeneralComp::Gt,
                right: Box::new(right),
            }),
            Rule::OP_GTE => Ok(ast::Expr::GeneralComparison {
                left: Box::new(left),
                op: ast::GeneralComp::Ge,
                right: Box::new(right),
            }),
            _ => Err(ParseAstError::new("unexpected comparison op")),
        }
    } else {
        build_range_expr(left_pair)
    }
}

fn build_range_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let mut it = pair.into_inner();
    let start = build_additive_expr(it.next().ok_or_else(|| ParseAstError::new("missing start of range"))?)?;
    if it.next().is_some() {
        // K_TO
        let end = build_additive_expr(it.next().ok_or_else(|| ParseAstError::new("missing end of range"))?)?;
        Ok(ast::Expr::Range { start: Box::new(start), end: Box::new(end) })
    } else {
        Ok(start)
    }
}

fn build_additive_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let mut exprs: SmallVec<[ast::Expr; 2]> = SmallVec::new();
    let mut ops: SmallVec<[ast::BinaryOp; 3]> = SmallVec::new(); // chains like a+b-c
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::multiplicative_expr => exprs.push(build_multiplicative_expr(p)?),
            // add_op is silent; we may see OP_PLUS/OP_MINUS directly
            Rule::OP_PLUS => ops.push(ast::BinaryOp::Add),
            Rule::OP_MINUS => ops.push(ast::BinaryOp::Sub),
            Rule::add_op => {
                // Fallback if non-silent: still unwrap to terminal
                let token = p.into_inner().next().ok_or_else(|| ParseAstError::new("add_op missing token"))?;
                let op = match token.as_rule() {
                    Rule::OP_PLUS => ast::BinaryOp::Add,
                    Rule::OP_MINUS => ast::BinaryOp::Sub,
                    _ => return Err(ParseAstError::new("unknown add op")),
                };
                ops.push(op);
            }
            _ => {}
        }
    }
    fold_left(exprs.into_iter(), ops.into_iter())
}

fn build_multiplicative_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let mut exprs: SmallVec<[ast::Expr; 2]> = SmallVec::new();
    let mut ops: SmallVec<[ast::BinaryOp; 3]> = SmallVec::new();
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::union_expr => exprs.push(build_union_like(p)?),
            // mult_op is silent; terminals may appear directly
            Rule::OP_STAR => ops.push(ast::BinaryOp::Mul),
            Rule::K_DIV => ops.push(ast::BinaryOp::Div),
            Rule::K_IDIV => ops.push(ast::BinaryOp::IDiv),
            Rule::K_MOD => ops.push(ast::BinaryOp::Mod),
            Rule::mult_op => {
                let token = p.into_inner().next().ok_or_else(|| ParseAstError::new("mult_op missing token"))?;
                let op = match token.as_rule() {
                    Rule::OP_STAR => ast::BinaryOp::Mul,
                    Rule::K_DIV => ast::BinaryOp::Div,
                    Rule::K_IDIV => ast::BinaryOp::IDiv,
                    Rule::K_MOD => ast::BinaryOp::Mod,
                    _ => return Err(ParseAstError::new("unknown mult op")),
                };
                ops.push(op);
            }
            _ => {}
        }
    }
    fold_left(exprs.into_iter(), ops.into_iter())
}

fn build_union_like(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // intersect_except_expr (union_op intersect_except_expr)*
    let mut it = pair.into_inner();
    let first = it.next().ok_or_else(|| ParseAstError::new("union_expr missing lhs"))?;
    let mut exprs: SmallVec<[ast::Expr; 2]> = SmallVec::new();
    exprs.push(build_intersect_except_like(first)?);
    let mut ops: SmallVec<[ast::SetOp; 2]> = SmallVec::new();
    while let Some(p) = it.next() {
        match p.as_rule() {
            // Some pest versions flatten the alternation, yielding OP_PIPE directly instead of union_op
            Rule::union_op | Rule::K_UNION | Rule::OP_PIPE => {
                ops.push(ast::SetOp::Union);
                let rhs = it.next().ok_or_else(|| ParseAstError::new("union_expr missing rhs"))?;
                exprs.push(build_intersect_except_like(rhs)?);
            }
            Rule::intersect_except_expr => {
                // Defensive: some pest versions may flatten as expr, union_op, expr; handle extra expr
                exprs.push(build_intersect_except_like(p)?);
            }
            _ => {}
        }
    }
    fold_set_ops(exprs.to_vec(), ops.to_vec())
}

fn build_intersect_except_like(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // instanceof_expr (intersect_except_op instanceof_expr)*
    let mut it = pair.into_inner();
    let first = it.next().ok_or_else(|| ParseAstError::new("intersect_except_expr missing lhs"))?;
    let mut exprs: SmallVec<[ast::Expr; 2]> = SmallVec::new();
    exprs.push(build_instanceof_like(first)?);
    let mut ops: SmallVec<[ast::SetOp; 2]> = SmallVec::new();
    while let Some(p) = it.next() {
        match p.as_rule() {
            Rule::intersect_except_op => {
                let token =
                    p.into_inner().next().ok_or_else(|| ParseAstError::new("intersect_except_op missing token"))?;
                let op = match token.as_rule() {
                    Rule::K_INTERSECT => ast::SetOp::Intersect,
                    Rule::K_EXCEPT => ast::SetOp::Except,
                    _ => return Err(ParseAstError::new("unknown set op")),
                };
                ops.push(op);
                let rhs = it.next().ok_or_else(|| ParseAstError::new("intersect_except_expr missing rhs"))?;
                exprs.push(build_instanceof_like(rhs)?);
            }
            Rule::K_INTERSECT => {
                ops.push(ast::SetOp::Intersect);
                let rhs = it.next().ok_or_else(|| ParseAstError::new("intersect missing rhs"))?;
                exprs.push(build_instanceof_like(rhs)?);
            }
            Rule::K_EXCEPT => {
                ops.push(ast::SetOp::Except);
                let rhs = it.next().ok_or_else(|| ParseAstError::new("except missing rhs"))?;
                exprs.push(build_instanceof_like(rhs)?);
            }
            Rule::instanceof_expr => {
                // Defensive
                exprs.push(build_instanceof_like(p)?);
            }
            _ => {}
        }
    }
    fold_set_ops(exprs.to_vec(), ops.to_vec())
}

fn fold_set_ops(mut exprs: Vec<ast::Expr>, mut ops: Vec<ast::SetOp>) -> AstResult<ast::Expr> {
    let mut it = exprs.drain(..);
    let mut left = it.next().ok_or_else(|| ParseAstError::new("empty set operation"))?;
    for (op, right) in ops.drain(..).zip(it) {
        left = ast::Expr::SetOp { left: Box::new(left), op, right: Box::new(right) };
    }
    Ok(left)
}

fn build_instanceof_like(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // treat_expr (K_INSTANCE K_OF sequence_type)?
    let mut it = pair.into_inner();
    let base = build_treat_like(it.next().ok_or_else(|| ParseAstError::new("missing base in instanceof_expr"))?)?;
    if let Some(_op) = it.next() {
        let ty = build_sequence_type(
            it.next().ok_or_else(|| ParseAstError::new("missing sequence_type in instanceof_expr"))?,
        )?;
        Ok(ast::Expr::InstanceOf { expr: Box::new(base), ty })
    } else {
        Ok(base)
    }
}

fn build_treat_like(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // castable_expr (K_TREAT K_AS sequence_type)?
    let mut it = pair.into_inner();
    let base = build_castable_like(it.next().ok_or_else(|| ParseAstError::new("missing base in treat_expr"))?)?;
    if let Some(_op) = it.next() {
        let ty =
            build_sequence_type(it.next().ok_or_else(|| ParseAstError::new("missing sequence_type in treat_expr"))?)?;
        Ok(ast::Expr::TreatAs { expr: Box::new(base), ty })
    } else {
        Ok(base)
    }
}

fn build_castable_like(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // cast_expr (K_CASTABLE K_AS single_type)?
    let mut it = pair.into_inner();
    let base = build_cast_like(it.next().ok_or_else(|| ParseAstError::new("missing base in castable_expr"))?)?;
    if let Some(_op) = it.next() {
        let ty =
            build_single_type(it.next().ok_or_else(|| ParseAstError::new("missing single_type in castable_expr"))?)?;
        Ok(ast::Expr::CastableAs { expr: Box::new(base), ty })
    } else {
        Ok(base)
    }
}

fn build_cast_like(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // unary_expr (K_CAST K_AS single_type)?
    let mut it = pair.into_inner();
    let base = build_unary_expr(it.next().ok_or_else(|| ParseAstError::new("missing base in cast_expr"))?)?;
    if let Some(_op) = it.next() {
        let ty = build_single_type(it.next().ok_or_else(|| ParseAstError::new("missing single_type in cast_expr"))?)?;
        Ok(ast::Expr::CastAs { expr: Box::new(base), ty })
    } else {
        Ok(base)
    }
}

fn build_sequence_type(pair: Pair<Rule>) -> AstResult<ast::SequenceType> {
    match pair.as_rule() {
        Rule::sequence_type => {
            let mut it = pair.into_inner();
            let first = it.next().ok_or_else(|| ParseAstError::new("empty sequence_type"))?;
            match first.as_rule() {
                Rule::K_EMPTY_SEQUENCE => {
                    // (K_EMPTY_SEQUENCE LPAR RPAR)
                    Ok(ast::SequenceType::EmptySequence)
                }
                Rule::item_type => {
                    let item = build_item_type(first)?;
                    let occ =
                        if let Some(occ_pair) = it.next() { build_occurrence(occ_pair)? } else { ast::Occurrence::One };
                    Ok(ast::SequenceType::Typed { item, occ })
                }
                _ => Err(ParseAstError::new("unexpected sequence_type child")),
            }
        }
        _ => Err(ParseAstError::new("expected sequence_type")),
    }
}

fn build_single_type(pair: Pair<Rule>) -> AstResult<ast::SingleType> {
    debug_assert_eq!(pair.as_rule(), Rule::single_type);
    let mut it = pair.into_inner();
    let atomic =
        build_qname_from_parts(it.next().ok_or_else(|| ParseAstError::new("single_type missing atomic_type"))?)?; // atomic_type -> qname
    let mut optional = false;
    if let Some(next) = it.next() {
        // QMARK
        debug_assert_eq!(next.as_rule(), Rule::QMARK);
        optional = true;
    }
    Ok(ast::SingleType { atomic, optional })
}

fn build_item_type(pair: Pair<Rule>) -> AstResult<ast::ItemType> {
    let mut it = pair.into_inner();
    let first = it.next().ok_or_else(|| ParseAstError::new("empty item_type"))?;
    match first.as_rule() {
        Rule::kind_test => Ok(ast::ItemType::Kind(build_kind_test(first)?)),
        Rule::K_ITEM => Ok(ast::ItemType::Item),
        Rule::atomic_type => Ok(ast::ItemType::Atomic(build_qname_from_parts(
            first.into_inner().next().ok_or_else(|| ParseAstError::new("atomic_type missing qname"))?,
        )?)),
        _ => Err(ParseAstError::new("unexpected item_type child")),
    }
}

fn build_occurrence(pair: Pair<Rule>) -> AstResult<ast::Occurrence> {
    let token = pair.into_inner().next().ok_or_else(|| ParseAstError::new("occurrence_indicator inner expected"))?;
    let occ = match token.as_rule() {
        Rule::QMARK => ast::Occurrence::ZeroOrOne,
        Rule::OP_STAR => ast::Occurrence::ZeroOrMore,
        Rule::OP_PLUS => ast::Occurrence::OneOrMore,
        _ => return Err(ParseAstError::new("unknown occurrence indicator")),
    };
    Ok(occ)
}

fn build_kind_test(pair: Pair<Rule>) -> AstResult<ast::KindTest> {
    let mut it = pair.into_inner();
    let first = it.next().ok_or_else(|| ParseAstError::new("empty kind_test"))?;
    let res = match first.as_rule() {
        Rule::any_kind_test => ast::KindTest::AnyKind,
        Rule::document_test => {
            // document_test = K_DOCUMENT_NODE LPAR (element_test | schema_element_test)? RPAR
            let inner = first.into_inner();
            let mut found: Option<ast::KindTest> = None;
            for p in inner {
                match p.as_rule() {
                    Rule::element_test => {
                        found = Some(build_kind_test_element(p)?);
                        break;
                    }
                    Rule::schema_element_test => {
                        let q = extract_qname_deep(p)?;
                        found = Some(ast::KindTest::SchemaElement(q));
                        break;
                    }
                    _ => {}
                }
            }
            if let Some(k) = found { ast::KindTest::Document(Some(Box::new(k))) } else { ast::KindTest::Document(None) }
        }
        Rule::text_test => ast::KindTest::Text,
        Rule::comment_test => ast::KindTest::Comment,
        Rule::pi_test => {
            // pi_test = K_PROCESSING_INSTRUCTION LPAR (ncname | string_literal)? RPAR
            let mut arg: Option<String> = None;
            for c in first.into_inner() {
                match c.as_rule() {
                    Rule::ncname => {
                        arg = Some(c.as_str().to_string());
                    }
                    Rule::string_literal => {
                        arg = Some(unescape_string_literal(c).into_owned());
                    }
                    _ => {}
                }
            }
            ast::KindTest::ProcessingInstruction(arg)
        }
        Rule::element_test => build_kind_test_element(first)?,
        Rule::attribute_test => build_kind_test_attribute(first)?,
        Rule::schema_element_test => {
            let q = extract_qname_deep(first)?;
            ast::KindTest::SchemaElement(q)
        }
        Rule::schema_attribute_test => {
            let q = extract_qname_deep(first)?;
            ast::KindTest::SchemaAttribute(q)
        }
        _ => return Err(ParseAstError::new("unknown kind_test")),
    };
    Ok(res)
}

fn build_kind_test_element(pair: Pair<Rule>) -> AstResult<ast::KindTest> {
    // element_test = K_ELEMENT LPAR (element_name_or_wildcard (COMMA type_name QMARK?)?)? RPAR
    let inner = pair.into_inner();
    let mut name: Option<ast::ElementNameOrWildcard> = None;
    let mut ty: Option<ast::TypeName> = None;
    let mut nillable = false;
    for p in inner {
        match p.as_rule() {
            Rule::element_name_or_wildcard => {
                let mut inn = p.into_inner();
                if let Some(ww) = inn.next() {
                    name = Some(match ww.as_rule() {
                        Rule::element_name => ast::ElementNameOrWildcard::Name(build_qname_from_parts(
                            ww.into_inner().next().ok_or_else(|| ParseAstError::new("element_name missing qname"))?,
                        )?),
                        Rule::OP_STAR => ast::ElementNameOrWildcard::Any,
                        _ => {
                            return Err(ParseAstError::new("unexpected element_name_or_wildcard"));
                        }
                    });
                } else {
                    // literal "*" alternative yields no inner pairs
                    name = Some(ast::ElementNameOrWildcard::Any);
                }
            }
            Rule::type_name => {
                let q = build_qname_from_parts(
                    p.into_inner()
                        .next()
                        .ok_or_else(|| ParseAstError::new("type_name missing qname in attribute_test"))?,
                )?;
                ty = Some(ast::TypeName(q));
            }
            Rule::QMARK => nillable = true,
            _ => {}
        }
    }
    Ok(ast::KindTest::Element { name, ty, nillable })
}

fn build_kind_test_attribute(pair: Pair<Rule>) -> AstResult<ast::KindTest> {
    // attribute_test = K_ATTRIBUTE LPAR (attrib_name_or_wildcard (COMMA type_name)?)? RPAR
    let inner = pair.into_inner();
    let mut name: Option<ast::AttributeNameOrWildcard> = None;
    let mut ty: Option<ast::TypeName> = None;
    for p in inner {
        match p.as_rule() {
            Rule::attrib_name_or_wildcard => {
                let mut inn = p.into_inner();
                if let Some(ww) = inn.next() {
                    name = Some(match ww.as_rule() {
                        Rule::attribute_name => ast::AttributeNameOrWildcard::Name(build_qname_from_parts(
                            ww.into_inner().next().ok_or_else(|| ParseAstError::new("attribute_name missing qname"))?,
                        )?),
                        Rule::OP_STAR => ast::AttributeNameOrWildcard::Any,
                        _ => {
                            return Err(ParseAstError::new("unexpected attrib_name_or_wildcard"));
                        }
                    });
                } else {
                    // literal "*" alternative yields no inner pairs
                    name = Some(ast::AttributeNameOrWildcard::Any);
                }
            }
            Rule::type_name => {
                let q = build_qname_from_parts(
                    p.into_inner()
                        .next()
                        .ok_or_else(|| ParseAstError::new("type_name missing qname in element_test"))?,
                )?;
                ty = Some(ast::TypeName(q));
            }
            _ => {}
        }
    }
    Ok(ast::KindTest::Attribute { name, ty })
}

fn build_name_test(pair: Pair<Rule>) -> AstResult<ast::NameTest> {
    let first = pair.into_inner().next().ok_or_else(|| ParseAstError::new("empty name_test"))?;
    match first.as_rule() {
        Rule::qname => Ok(ast::NameTest::QName(build_qname_from_parts(first)?)),
        Rule::wildcard_name => Ok(ast::NameTest::Wildcard(build_wildcard_name(first.as_str()))),
        _ => Err(ParseAstError::new("unexpected name_test")),
    }
}

fn build_wildcard_name(s: &str) -> ast::WildcardName {
    if s == "*" {
        return ast::WildcardName::Any;
    }
    if let Some(rest) = s.strip_prefix("*:") {
        return ast::WildcardName::LocalWildcard(rest.to_string());
    }
    if let Some(pre) = s.strip_suffix(":*") {
        return ast::WildcardName::NsWildcard(pre.to_string());
    }
    ast::WildcardName::Any
}

fn build_node_test(pair: Pair<Rule>) -> AstResult<ast::NodeTest> {
    let first = pair.into_inner().next().ok_or_else(|| ParseAstError::new("empty node_test"))?;
    match first.as_rule() {
        Rule::kind_test => Ok(ast::NodeTest::Kind(build_kind_test(first)?)),
        Rule::name_test => Ok(ast::NodeTest::Name(build_name_test(first)?)),
        _ => Err(ParseAstError::new("unexpected node_test")),
    }
}

fn build_axis(axis_pair: Pair<Rule>) -> AstResult<ast::Axis> {
    match axis_pair.as_rule() {
        Rule::forward_axis => {
            let token = axis_pair.into_inner().next().ok_or_else(|| ParseAstError::new("empty forward_axis"))?;
            let axis = match token.as_rule() {
                Rule::K_CHILD => ast::Axis::Child,
                Rule::K_DESCENDANT => ast::Axis::Descendant,
                Rule::K_ATTRIBUTE => ast::Axis::Attribute,
                Rule::K_SELF => ast::Axis::SelfAxis,
                Rule::K_DESCENDANT_OR_SELF => ast::Axis::DescendantOrSelf,
                Rule::K_FOLLOWING_SIBLING => ast::Axis::FollowingSibling,
                Rule::K_FOLLOWING => ast::Axis::Following,
                Rule::K_NAMESPACE => ast::Axis::Namespace,
                _ => return Err(ParseAstError::new("unknown forward axis")),
            };
            Ok(axis)
        }
        Rule::reverse_axis => {
            let token = axis_pair.into_inner().next().ok_or_else(|| ParseAstError::new("empty reverse_axis"))?;
            let axis = match token.as_rule() {
                Rule::K_PARENT => ast::Axis::Parent,
                Rule::K_ANCESTOR => ast::Axis::Ancestor,
                Rule::K_PRECEDING_SIBLING => ast::Axis::PrecedingSibling,
                Rule::K_PRECEDING => ast::Axis::Preceding,
                Rule::K_ANCESTOR_OR_SELF => ast::Axis::AncestorOrSelf,
                _ => return Err(ParseAstError::new("unknown reverse axis")),
            };
            Ok(axis)
        }
        _ => Err(ParseAstError::new("expected axis rule")),
    }
}

fn build_predicate_list(pair: Pair<Rule>) -> AstResult<Vec<ast::Expr>> {
    let mut preds = Vec::with_capacity(2); // Most predicates have 1-2 conditions
    for p in pair.into_inner() {
        if p.as_rule() == Rule::predicate {
            // predicate = LBRACK ~ expr ~ RPAR
            let mut inn = p.into_inner();
            let first = inn.next(); // LBRACK
            let maybe_expr =
                if let Some(f) = first { if f.as_rule() == Rule::LBRACK { inn.next() } else { Some(f) } } else { None };
            if let Some(e) = maybe_expr {
                preds.push(build_expr(e)?);
            }
        }
    }
    Ok(preds)
}

fn build_axis_step(pair: Pair<Rule>) -> AstResult<ast::Step> {
    // (reverse_step | forward_step) ~ predicate_list
    let mut it = pair.into_inner();
    let step = it.next().ok_or_else(|| ParseAstError::new("axis_step missing step"))?;
    let (axis, test) = match step.as_rule() {
        Rule::reverse_step => {
            let mut inn = step.into_inner();
            let axis = build_axis(inn.next().ok_or_else(|| ParseAstError::new("reverse_step missing axis"))?)?;
            let test =
                build_node_test(inn.next().ok_or_else(|| ParseAstError::new("reverse_step missing node_test"))?)?;
            (axis, test)
        }
        Rule::forward_step => {
            let mut inn = step.into_inner();
            let first = inn.next().ok_or_else(|| ParseAstError::new("forward_step missing inner"))?;
            match first.as_rule() {
                Rule::forward_axis => {
                    let axis = build_axis(first)?;
                    let test = build_node_test(
                        inn.next().ok_or_else(|| ParseAstError::new("forward_step missing node_test"))?,
                    )?;
                    (axis, test)
                }
                Rule::abbrev_forward_step => {
                    let mut ainn = first.into_inner();
                    let a = ainn.next().ok_or_else(|| ParseAstError::new("abbrev_forward_step missing inner"))?;
                    match a.as_rule() {
                        Rule::OP_AT => {
                            let nt = ainn.next().ok_or_else(|| ParseAstError::new("@ without name_test"))?;
                            debug_assert_eq!(nt.as_rule(), Rule::name_test);
                            (ast::Axis::Attribute, ast::NodeTest::Name(build_name_test(nt)?))
                        }
                        Rule::node_test => (ast::Axis::Child, build_node_test(a)?),
                        _ => {
                            return Err(ParseAstError::new("unexpected abbrev_forward_step"));
                        }
                    }
                }
                _ => {
                    return Err(ParseAstError::new("unexpected forward_step child"));
                }
            }
        }
        _ => return Err(ParseAstError::new("unexpected axis_step child")),
    };
    let preds = build_predicate_list(it.next().ok_or_else(|| ParseAstError::new("axis_step missing predicate_list"))?)?;
    Ok(ast::Step::Axis { axis, test, predicates: preds })
}

fn build_filter_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let mut it = pair.into_inner();
    let primary = it.next().ok_or_else(|| ParseAstError::new("filter_expr missing primary"))?;
    let input = build_primary_expr(primary)?;
    let preds =
        build_predicate_list(it.next().ok_or_else(|| ParseAstError::new("filter_expr missing predicate_list"))?)?;
    if preds.is_empty() { Ok(input) } else { Ok(ast::Expr::Filter { input: Box::new(input), predicates: preds }) }
}

fn build_axis_step_as_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let step = build_axis_step(pair)?;
    // Represent a single-step relative path starting at context as Path(Relative)
    Ok(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Relative, steps: vec![step] }))
}

fn build_primary_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let first = pair.into_inner().next().ok_or_else(|| ParseAstError::new("empty primary_expr"))?;
    match first.as_rule() {
        Rule::var_ref => build_var_ref(first),
        Rule::function_call => build_function_call(first),
        Rule::literal => build_literal(first),
        Rule::parenthesized_expr => build_parenthesized(first),
        Rule::abbrev_reverse_step => {
            // ".." -> parent::node()
            let step = ast::Step::Axis {
                axis: ast::Axis::Parent,
                test: ast::NodeTest::Kind(ast::KindTest::AnyKind),
                predicates: vec![],
            };
            Ok(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Relative, steps: vec![step] }))
        }
        Rule::context_item_expr => Ok(ast::Expr::ContextItem),
        _ => Err(ParseAstError::new("unexpected primary_expr")),
    }
}

fn build_path_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let first = pair.into_inner().next().ok_or_else(|| ParseAstError::new("empty path_expr"))?;
    match first.as_rule() {
        Rule::absolute_path => build_path_expr_from_absolute(first),
        Rule::relative_path_expr => build_path_expr_from_relative(first, None),
        _ => Err(ParseAstError::new("unexpected path_expr child")),
    }
}

fn build_path_expr_from_absolute(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // absolute_path = ("//" ~ relative_path_expr) | ("/" ~ relative_path_expr?)
    let mut it = pair.into_inner();
    let first = it.next().ok_or_else(|| ParseAstError::new("empty absolute_path"))?;
    match first.as_rule() {
        Rule::OP_DSLASH => {
            let rel = it.next().ok_or_else(|| ParseAstError::new("// without relative_path_expr"))?;
            let mut steps = vec![ast::Step::Axis {
                axis: ast::Axis::DescendantOrSelf,
                test: ast::NodeTest::Kind(ast::KindTest::AnyKind),
                predicates: vec![],
            }];
            steps.extend(build_relative_steps(rel)?);
            Ok(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Root, steps: steps.to_vec() }))
        }
        Rule::OP_SLASH => {
            if let Some(rel) = it.next() {
                let steps = build_relative_steps(rel)?;
                Ok(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Root, steps: steps.to_vec() }))
            } else {
                Ok(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Root, steps: vec![] }))
            }
        }
        _ => Err(ParseAstError::new("unexpected absolute_path")),
    }
}

fn build_path_expr_from_relative(pair: Pair<Rule>, base: Option<ast::Expr>) -> AstResult<ast::Expr> {
    // If the first step is a filter_expr whose primary is not context item, treat as PathFrom base
    let mut it = pair.into_inner();
    let first_step = it.next().ok_or_else(|| ParseAstError::new("empty relative_path_expr"))?;
    // Path step counts are small; inline first 4 steps before spilling to heap
    let mut steps: SmallVec<[ast::Step; 4]> = SmallVec::new();
    let mut base_expr: Option<ast::Expr> = base;

    let step_variant = if first_step.as_rule() == Rule::step_expr {
        first_step.into_inner().next().ok_or_else(|| ParseAstError::new("step_expr missing inner"))?
    } else {
        first_step
    };
    match step_variant.as_rule() {
        Rule::axis_step => steps.push(build_axis_step(step_variant)?),
        Rule::filter_expr => {
            let f = build_filter_expr(step_variant)?;
            base_expr = Some(f);
        }
        _ => {
            return Err(ParseAstError::new("unexpected relative first step"));
        }
    }

    // Process following (path_operator, step_expr) pairs
    while let Some(op) = it.next() {
        // op can be path_operator (wrapper) or a direct OP_SLASH/OP_DSLASH depending on context
        let step_expr = it.next().ok_or_else(|| ParseAstError::new("missing step_expr after path operator"))?;
        // Insert implicit descendant-or-self::node() for //
        let token = if op.as_rule() == Rule::path_operator {
            op.into_inner().next().ok_or_else(|| ParseAstError::new("empty path_operator"))?
        } else {
            op
        };
        if token.as_rule() == Rule::OP_DSLASH {
            steps.push(ast::Step::Axis {
                axis: ast::Axis::DescendantOrSelf,
                test: ast::NodeTest::Kind(ast::KindTest::AnyKind),
                predicates: vec![],
            });
        }
        let step_variant = if step_expr.as_rule() == Rule::step_expr {
            step_expr.into_inner().next().ok_or_else(|| ParseAstError::new("step_expr missing inner after operator"))?
        } else {
            step_expr
        };
        match step_variant.as_rule() {
            Rule::axis_step => steps.push(build_axis_step(step_variant)?),
            Rule::filter_expr => {
                let f = build_filter_expr(step_variant)?;
                if base_expr.is_none() && steps.is_empty() {
                    base_expr = Some(f);
                } else {
                    steps.push(ast::Step::FilterExpr(Box::new(f)));
                }
            }
            _ => {
                return Err(ParseAstError::new("unexpected step_expr after operator"));
            }
        }
    }

    if let Some(base_e) = base_expr {
        if steps.is_empty() {
            Ok(base_e)
        } else {
            Ok(ast::Expr::PathFrom { base: Box::new(base_e), steps: steps.to_vec() })
        }
    } else {
        Ok(ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Relative, steps: steps.to_vec() }))
    }
}

fn build_relative_steps(pair: Pair<Rule>) -> AstResult<Vec<ast::Step>> {
    let mut it = pair.into_inner();
    // Typical relative path length is short (<=4)
    let mut steps: SmallVec<[ast::Step; 4]> = SmallVec::new();
    let first = it.next().ok_or_else(|| ParseAstError::new("empty relative steps"))?;
    let first_variant = if first.as_rule() == Rule::step_expr {
        first.into_inner().next().ok_or_else(|| ParseAstError::new("step_expr missing inner in relative steps"))?
    } else {
        first
    };
    match first_variant.as_rule() {
        Rule::axis_step => steps.push(build_axis_step(first_variant)?),
        Rule::filter_expr => {
            // This means path starts from an arbitrary base expression
            return Err(ParseAstError::new(
                "relative path starting with expression must be handled via build_path_expr_from_relative",
            ));
        }
        _ => return Err(ParseAstError::new("unexpected relative step")),
    }
    while let Some(op) = it.next() {
        let token = if op.as_rule() == Rule::path_operator {
            op.clone().into_inner().next().ok_or_else(|| ParseAstError::new("empty path_operator in relative steps"))?
        } else {
            op.clone()
        };
        if token.as_rule() == Rule::OP_DSLASH {
            steps.push(ast::Step::Axis {
                axis: ast::Axis::DescendantOrSelf,
                test: ast::NodeTest::Kind(ast::KindTest::AnyKind),
                predicates: vec![],
            });
        }
        let step_expr = it.next().ok_or_else(|| ParseAstError::new("missing step_expr in relative steps"))?;
        let step_variant = if step_expr.as_rule() == Rule::step_expr {
            step_expr
                .into_inner()
                .next()
                .ok_or_else(|| ParseAstError::new("step_expr missing inner in relative steps"))?
        } else {
            step_expr
        };
        match step_variant.as_rule() {
            Rule::axis_step => steps.push(build_axis_step(step_variant)?),
            Rule::filter_expr => {
                let f = build_filter_expr(step_variant)?;
                if steps.is_empty() {
                    return Err(ParseAstError::new(
                        "relative path starting with expression must be handled via build_path_expr_from_relative",
                    ));
                }
                steps.push(ast::Step::FilterExpr(Box::new(f)));
            }
            _ => {
                return Err(ParseAstError::new("unexpected step in relative steps"));
            }
        }
    }
    Ok(steps.to_vec())
}

fn build_for_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // K_FOR ~ "$" ~ var_name ~ K_IN ~ expr_single ~ (COMMA ~ "$" ~ var_name ~ K_IN ~ expr_single)* ~ K_RETURN ~ expr_single
    let mut it = pair.into_inner();
    // For expressions commonly have few bindings (1-3)
    let mut bindings: SmallVec<[ast::ForBinding; 4]> = SmallVec::new();
    while let Some(p) = it.next() {
        match p.as_rule() {
            Rule::var_name => {
                let var = build_qname_from_parts(p)?;
                let _in_kw = it.next(); // K_IN
                let in_expr = build_expr(
                    it.next().ok_or_else(|| ParseAstError::new("for_expr missing expr_single after K_IN"))?,
                )?; // expr_single
                bindings.push(ast::ForBinding { var, in_expr });
            }
            Rule::K_RETURN => {
                let return_expr =
                    build_expr(it.next().ok_or_else(|| ParseAstError::new("for_expr missing return expr_single"))?)?;
                return Ok(ast::Expr::ForExpr { bindings: bindings.to_vec(), return_expr: Box::new(return_expr) });
            }
            _ => {}
        }
    }
    Err(ParseAstError::new("malformed for_expr"))
}

fn build_let_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // K_LET ~ "$" ~ var_name ~ OP_ASSIGN ~ expr_single ~ (COMMA ~ "$" ~ var_name ~ OP_ASSIGN ~ expr_single)* ~ K_RETURN ~ expr_single
    let mut it = pair.into_inner();
    let mut bindings: SmallVec<[ast::LetBinding; 4]> = SmallVec::new();
    while let Some(p) = it.next() {
        match p.as_rule() {
            Rule::var_name => {
                let var = build_qname_from_parts(p)?;
                match it.next() {
                    Some(assign) if assign.as_rule() == Rule::OP_ASSIGN => {}
                    _ => {
                        return Err(ParseAstError::new("let_expr missing := after variable name"));
                    }
                }
                let value_expr =
                    build_expr(it.next().ok_or_else(|| ParseAstError::new("let_expr missing expr_single after :="))?)?;
                bindings.push(ast::LetBinding { var, value: value_expr });
            }
            Rule::K_RETURN => {
                let return_expr =
                    build_expr(it.next().ok_or_else(|| ParseAstError::new("let_expr missing return expr_single"))?)?;
                return Ok(ast::Expr::LetExpr { bindings: bindings.to_vec(), return_expr: Box::new(return_expr) });
            }
            Rule::COMMA | Rule::OP_ASSIGN => {
                // already handled implicitly by loop
            }
            _ => {}
        }
    }
    Err(ParseAstError::new("malformed let_expr"))
}

fn build_quantified_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // (K_SOME | K_EVERY) ~ "$" ~ var_name ~ K_IN ~ expr_single ~ (COMMA ~ "$" ~ var_name ~ K_IN ~ expr_single)* ~ K_SATISFIES ~ expr_single
    let mut it = pair.into_inner();
    let first = it.next().ok_or_else(|| ParseAstError::new("quantified_expr missing quantifier"))?;
    let kind = match first.as_rule() {
        Rule::K_SOME => ast::Quantifier::Some,
        Rule::K_EVERY => ast::Quantifier::Every,
        _ => return Err(ParseAstError::new("expected quantifier")),
    };
    let mut bindings: SmallVec<[ast::QuantifiedBinding; 4]> = SmallVec::new();
    // After quantifier, we expect sequences of var_name, K_IN, expr_single ... until K_SATISFIES
    loop {
        let p = it.next().ok_or_else(|| ParseAstError::new("incomplete quantified bindings"))?;
        match p.as_rule() {
            Rule::var_name => {
                let var = build_qname_from_parts(p)?;
                let _in_kw = it.next();
                let in_expr = build_expr(
                    it.next().ok_or_else(|| ParseAstError::new("quantified_expr missing expr_single after K_IN"))?,
                )?;
                bindings.push(ast::QuantifiedBinding { var, in_expr });
                // Next could be COMMA or K_SATISFIES
                if let Some(peek) = it.clone().next() {
                    match peek.as_rule() {
                        Rule::COMMA => {
                            it.next(); /* consume comma */
                        }
                        Rule::K_SATISFIES => {}
                        _ => {}
                    }
                }
            }
            Rule::K_SATISFIES => {
                let satisfies = build_expr(
                    it.next().ok_or_else(|| ParseAstError::new("quantified_expr missing satisfies expr_single"))?,
                )?;
                return Ok(ast::Expr::Quantified { kind, bindings: bindings.to_vec(), satisfies: Box::new(satisfies) });
            }
            _ => {
                return Err(ParseAstError::new("unexpected token in quantified_expr"));
            }
        }
    }
}

fn build_if_expr(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    let mut it = pair.into_inner();
    let _if = it.next(); // K_IF
    let _lpar = it.next();
    let cond = build_expr(it.next().ok_or_else(|| ParseAstError::new("if_expr missing condition"))?)?; // expr
    let _rpar = it.next();
    let _then = it.next();
    let then_expr = build_expr(it.next().ok_or_else(|| ParseAstError::new("if_expr missing then expr_single"))?)?; // expr_single
    let _else = it.next();
    let else_expr = build_expr(it.next().ok_or_else(|| ParseAstError::new("if_expr missing else expr_single"))?)?; // expr_single
    Ok(ast::Expr::IfThenElse { cond: Box::new(cond), then_expr: Box::new(then_expr), else_expr: Box::new(else_expr) })
}

fn build_comma_sequence(pair: Pair<Rule>) -> AstResult<ast::Expr> {
    // expr = expr_single (COMMA expr_single)*
    let mut items = Vec::new();
    for p in pair.into_inner() {
        if p.as_rule() == Rule::expr_single {
            items.push(build_expr(p)?);
        }
    }
    if items.len() == 1 { Ok(items.remove(0)) } else { Ok(ast::Expr::Sequence(items)) }
}
