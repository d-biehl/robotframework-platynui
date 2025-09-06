use pest::Parser;
use pest::error::Error;
use pest::iterators::{Pair, Pairs};

#[derive(pest_derive::Parser)]
#[grammar = "xpath2.pest"]
pub struct XPath2Parser;

impl XPath2Parser {
    /// Parse an XPath expression from a string
    pub fn parse_xpath(input: &str) -> Result<Pairs<'_, Rule>, Error<Rule>> {
        Self::parse(Rule::xpath, input)
    }

    /// Parse and print the parse tree for debugging
    pub fn parse_and_debug(input: &str) -> Result<(), Box<dyn std::error::Error>> {
        let pairs = Self::parse_xpath(input)?;
        for pair in pairs {
            Self::print_pair(&pair, 0);
        }
        Ok(())
    }

    /// Helper function to print parse tree
    fn print_pair(pair: &Pair<Rule>, indent: usize) {
        let indent_str = "  ".repeat(indent);
        println!(
            "{}Rule::{:?} => \"{}\"",
            indent_str,
            pair.as_rule(),
            pair.as_str()
        );

        for inner_pair in pair.clone().into_inner() {
            Self::print_pair(&inner_pair, indent + 1);
        }
    }

    /// Extract the rule hierarchy from a parse tree for testing
    pub fn extract_rule_path(pairs: Pairs<'_, Rule>) -> Vec<Rule> {
        let mut rules = Vec::new();
        for pair in pairs {
            Self::collect_rules(&pair, &mut rules);
        }
        rules
    }

    /// Recursively collect rules from parse tree
    fn collect_rules(pair: &Pair<Rule>, rules: &mut Vec<Rule>) {
        rules.push(pair.as_rule());
        for inner_pair in pair.clone().into_inner() {
            Self::collect_rules(&inner_pair, rules);
        }
    }

    /// Extract AST structure for specific rule types (for precedence testing)
    pub fn extract_expression_structure(pairs: Pairs<'_, Rule>) -> Option<ExpressionNode> {
        for pair in pairs {
            if let Some(node) = Self::parse_expression_node(&pair) {
                return Some(node);
            }
        }
        None
    }

    /// Parse a pair into an expression node for structure validation
    fn parse_expression_node(pair: &Pair<Rule>) -> Option<ExpressionNode> {
        match pair.as_rule() {
            Rule::additive_expr => {
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                if let Some(op_pair) = inners.next() {
                    let right = inners.next()?;
                    let left_node = Self::parse_expression_node(&left)?;
                    let right_node = Self::parse_expression_node(&right)?;
                    let op = match op_pair.as_str() {
                        "+" => BinaryOp::Add,
                        "-" => BinaryOp::Subtract,
                        _ => return None,
                    };
                    Some(ExpressionNode::Binary {
                        left: Box::new(left_node),
                        op,
                        right: Box::new(right_node),
                    })
                } else {
                    Self::parse_expression_node(&left)
                }
            }
            Rule::multiplicative_expr => {
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                if let Some(op_pair) = inners.next() {
                    let right = inners.next()?;
                    let left_node = Self::parse_expression_node(&left)?;
                    let right_node = Self::parse_expression_node(&right)?;
                    let op = match op_pair.as_str() {
                        "*" => BinaryOp::Multiply,
                        "div" => BinaryOp::Divide,
                        "mod" => BinaryOp::Modulo,
                        _ => return None,
                    };
                    Some(ExpressionNode::Binary {
                        left: Box::new(left_node),
                        op,
                        right: Box::new(right_node),
                    })
                } else {
                    Self::parse_expression_node(&left)
                }
            }
            Rule::and_expr => {
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                if let Some(_op_pair) = inners.next() {
                    let right = inners.next()?;
                    let left_node = Self::parse_expression_node(&left)?;
                    let right_node = Self::parse_expression_node(&right)?;
                    Some(ExpressionNode::Binary {
                        left: Box::new(left_node),
                        op: BinaryOp::And,
                        right: Box::new(right_node),
                    })
                } else {
                    Self::parse_expression_node(&left)
                }
            }
            Rule::or_expr => {
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                if let Some(_op_pair) = inners.next() {
                    let right = inners.next()?;
                    let left_node = Self::parse_expression_node(&left)?;
                    let right_node = Self::parse_expression_node(&right)?;
                    Some(ExpressionNode::Binary {
                        left: Box::new(left_node),
                        op: BinaryOp::Or,
                        right: Box::new(right_node),
                    })
                } else {
                    Self::parse_expression_node(&left)
                }
            }
            Rule::comparison_expr => {
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                if let Some(op_pair) = inners.next() {
                    let right = inners.next()?;
                    let left_node = Self::parse_expression_node(&left)?;
                    let right_node = Self::parse_expression_node(&right)?;
                    let op = match op_pair.as_str() {
                        "=" => BinaryOp::Equal,
                        "!=" => BinaryOp::NotEqual,
                        "<" => BinaryOp::LessThan,
                        "<=" => BinaryOp::LessThanOrEqual,
                        ">" => BinaryOp::GreaterThan,
                        ">=" => BinaryOp::GreaterThanOrEqual,
                        _ => return None,
                    };
                    Some(ExpressionNode::Binary {
                        left: Box::new(left_node),
                        op,
                        right: Box::new(right_node),
                    })
                } else {
                    Self::parse_expression_node(&left)
                }
            }
            Rule::integer_literal => Some(ExpressionNode::Literal(pair.as_str().to_string())),
            Rule::decimal_literal => Some(ExpressionNode::Literal(pair.as_str().to_string())),
            Rule::string_literal => Some(ExpressionNode::Literal(pair.as_str().to_string())),
            Rule::qname => Some(ExpressionNode::Identifier(pair.as_str().to_string())),
            Rule::function_call => {
                let mut inners = pair.clone().into_inner();
                let name = inners.next()?.as_str().to_string();
                let args: Vec<ExpressionNode> = inners
                    .filter_map(|arg| Self::parse_expression_node(&arg))
                    .collect();
                Some(ExpressionNode::FunctionCall { name, args })
            }
            _ => {
                // For other rules, try to parse their inner content
                for inner in pair.clone().into_inner() {
                    if let Some(node) = Self::parse_expression_node(&inner) {
                        return Some(node);
                    }
                }
                None
            }
        }
    }
}

/// Simplified AST node for testing expression structure
#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionNode {
    Binary {
        left: Box<ExpressionNode>,
        op: BinaryOp,
        right: Box<ExpressionNode>,
    },
    FunctionCall {
        name: String,
        args: Vec<ExpressionNode>,
    },
    Identifier(String),
    Literal(String),
}

/// Binary operators for expression testing
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
