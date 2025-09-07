use pest::iterators::{Pair, Pairs};
use platynui_xpath::parser::{Rule, XPathParser};
use pest::Parser;

// Test-only expression node structure for precedence/shape assertions
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionNode {
    Binary { left: Box<ExpressionNode>, op: BinaryOp, right: Box<ExpressionNode> },
    FunctionCall { name: String, args: Vec<ExpressionNode> },
    Identifier(String),
    Literal(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    And,
    Or,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// Helper function to check if an AST is well-formed (no incomplete structures)
pub fn is_well_formed_ast(node: &ExpressionNode) -> bool {
    match node {
        ExpressionNode::Binary { left, right, .. } => {
            is_well_formed_ast(left) && is_well_formed_ast(right)
        }
        ExpressionNode::FunctionCall { args, .. } => {
            args.iter().all(is_well_formed_ast)
        }
        ExpressionNode::Identifier(_) | ExpressionNode::Literal(_) => true,
    }
}

/// Helper function to parse and extract AST structure with common validation
pub fn parse_and_extract_ast(xpath: &str) -> ExpressionNode {
    let pairs = parse_xpath(xpath)
        .unwrap_or_else(|e| panic!("Expression '{}' should parse successfully: {}", xpath, e));
    let ast = extract_expression_structure(pairs)
        .unwrap_or_else(|| panic!("Should extract AST structure for: '{}'", xpath));
    assert!(is_well_formed_ast(&ast), "AST should be well-formed for: '{}'", xpath);
    ast
}

pub fn parse_xpath(input: &str) -> Result<Pairs<'_, Rule>, pest::error::Error<Rule>> {
    XPathParser::parse(Rule::xpath, input)
}

pub fn extract_expression_structure(pairs: Pairs<'_, Rule>) -> Option<ExpressionNode> {
    for pair in pairs {
        if let Some(node) = parse_expression_node(&pair) {
            return Some(node);
        }
    }
    None
}

fn parse_expression_node(pair: &Pair<Rule>) -> Option<ExpressionNode> {
    match pair.as_rule() {
        Rule::additive_expr => {
            let mut inners = pair.clone().into_inner();
            let left = inners.next()?;
            if let Some(op_pair) = inners.next() {
                let right = inners.next()?;
                let left_node = parse_expression_node(&left)?;
                let right_node = parse_expression_node(&right)?;
                let op = match first_token_rule(&op_pair) {
                    Rule::OP_PLUS => BinaryOp::Add,
                    Rule::OP_MINUS => BinaryOp::Subtract,
                    _ => return None,
                };
                Some(ExpressionNode::Binary { left: Box::new(left_node), op, right: Box::new(right_node) })
            } else {
                parse_expression_node(&left)
            }
        }
        Rule::multiplicative_expr => {
            let mut inners = pair.clone().into_inner();
            let left = inners.next()?;
            if let Some(op_pair) = inners.next() {
                let right = inners.next()?;
                let left_node = parse_expression_node(&left)?;
                let right_node = parse_expression_node(&right)?;
                let op = match first_token_rule(&op_pair) {
                    Rule::OP_STAR => BinaryOp::Multiply,
                    Rule::K_DIV => BinaryOp::Divide,
                    Rule::K_MOD => BinaryOp::Modulo,
                    _ => return None,
                };
                Some(ExpressionNode::Binary { left: Box::new(left_node), op, right: Box::new(right_node) })
            } else {
                parse_expression_node(&left)
            }
        }
        Rule::and_expr => {
            let mut inners = pair.clone().into_inner();
            let left = inners.next()?;
            if let Some(op_pair) = inners.next() {
                let right = inners.next()?;
                let left_node = parse_expression_node(&left)?;
                let right_node = parse_expression_node(&right)?;
                let op = match first_token_rule(&op_pair) { Rule::K_AND => BinaryOp::And, _ => return None };
                Some(ExpressionNode::Binary { left: Box::new(left_node), op, right: Box::new(right_node) })
            } else {
                parse_expression_node(&left)
            }
        }
        Rule::or_expr => {
            let mut inners = pair.clone().into_inner();
            let left = inners.next()?;
            if let Some(op_pair) = inners.next() {
                let right = inners.next()?;
                let left_node = parse_expression_node(&left)?;
                let right_node = parse_expression_node(&right)?;
                let op = match first_token_rule(&op_pair) { Rule::K_OR => BinaryOp::Or, _ => return None };
                Some(ExpressionNode::Binary { left: Box::new(left_node), op, right: Box::new(right_node) })
            } else {
                parse_expression_node(&left)
            }
        }
        Rule::comparison_expr => {
            let mut inners = pair.clone().into_inner();
            let left = inners.next()?;
            if let Some(op_pair) = inners.next() {
                let right = inners.next()?;
                let left_node = parse_expression_node(&left)?;
                let right_node = parse_expression_node(&right)?;
                let token = first_token_rule(&op_pair);
                let op = match token {
                    Rule::OP_EQ | Rule::K_EQ => BinaryOp::Equal,
                    Rule::OP_NE | Rule::K_NE => BinaryOp::NotEqual,
                    Rule::OP_LT | Rule::K_LT => BinaryOp::LessThan,
                    Rule::OP_LTE | Rule::K_LE => BinaryOp::LessThanOrEqual,
                    Rule::OP_GT | Rule::K_GT => BinaryOp::GreaterThan,
                    Rule::OP_GTE | Rule::K_GE => BinaryOp::GreaterThanOrEqual,
                    _ => return None,
                };
                Some(ExpressionNode::Binary { left: Box::new(left_node), op, right: Box::new(right_node) })
            } else {
                parse_expression_node(&left)
            }
        }
        Rule::integer_literal | Rule::decimal_literal => Some(ExpressionNode::Literal(pair.as_str().to_string())),
        Rule::string_literal => {
            // Extract unescaped inner content from string literal
            let mut inners = pair.clone().into_inner();
            if let Some(inner) = inners.next() {
                let raw = inner.as_str();
                let s = match inner.as_rule() {
                    Rule::dbl_string_inner => raw.replace("\"\"", "\""),
                    Rule::sgl_string_inner => raw.replace("''", "'"),
                    _ => raw.to_string(),
                };
                Some(ExpressionNode::Literal(s))
            } else {
                Some(ExpressionNode::Literal(String::new()))
            }
        }
        Rule::dbl_string_inner => Some(ExpressionNode::Literal(pair.as_str().replace("\"\"", "\""))),
        Rule::sgl_string_inner => Some(ExpressionNode::Literal(pair.as_str().replace("''", "'"))),
        Rule::qname => Some(ExpressionNode::Identifier(pair.as_str().to_string())),
        Rule::function_call => {
            let mut inners = pair.clone().into_inner();
            let name = inners.next()?.as_str().to_string();
            let args: Vec<ExpressionNode> = inners.filter_map(|a| parse_expression_node(&a)).collect();
            Some(ExpressionNode::FunctionCall { name, args })
        }
        _ => {
            for inner in pair.clone().into_inner() {
                if let Some(node) = parse_expression_node(&inner) { return Some(node); }
            }
            None
        }
    }
}

fn first_token_rule(pair: &Pair<Rule>) -> Rule {
    let mut current = pair.clone();
    loop {
        let mut inner = current.clone().into_inner();
        if let Some(next) = inner.next() { current = next; } else { return current.as_rule(); }
    }
}

/// Helper function to assert binary expression structure
pub fn assert_binary_expr(ast: ExpressionNode, expected_op: BinaryOp, xpath: &str) -> (Box<ExpressionNode>, Box<ExpressionNode>) {
    let ExpressionNode::Binary { left, op, right } = ast else {
        panic!("Expected binary expression for: '{}'", xpath);
    };
    assert_eq!(op, expected_op, "Expected {:?} operator for: '{}'", expected_op, xpath);
    (left, right)
}

/// Helper function to assert function call structure
pub fn assert_function_call(ast: ExpressionNode, expected_name: &str, expected_arg_count: usize, xpath: &str) -> Vec<ExpressionNode> {
    let ExpressionNode::FunctionCall { name, args } = ast else {
        panic!("Expected function call for: '{}'", xpath);
    };
    assert_eq!(name, expected_name, "Expected function name '{}' for: '{}'", expected_name, xpath);
    assert_eq!(args.len(), expected_arg_count, "Expected {} arguments for: '{}'", expected_arg_count, xpath);
    args
}

/// Helper function to assert literal value
pub fn assert_literal(ast: &ExpressionNode, expected_value: &str, xpath: &str) {
    assert_eq!(*ast, ExpressionNode::Literal(expected_value.to_string()), 
              "Expected literal '{}' for: '{}'", expected_value, xpath);
}

/// Helper function to assert identifier value
pub fn assert_identifier(ast: &ExpressionNode, expected_name: &str, xpath: &str) {
    assert_eq!(*ast, ExpressionNode::Identifier(expected_name.to_string()), 
              "Expected identifier '{}' for: '{}'", expected_name, xpath);
}
