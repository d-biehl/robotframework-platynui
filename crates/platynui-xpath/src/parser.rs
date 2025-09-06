use pest::Parser;
use pest::error::Error;
use pest::iterators::{Pair, Pairs};

#[derive(pest_derive::Parser)]
#[grammar = "xpath2.pest"]
pub struct XPathParser;

impl XPathParser {
    /// Parse an XPath expression from a string
    pub fn parse_xpath(input: &str) -> Result<Pairs<'_, Rule>, Error<Rule>> {
        Self::parse(Rule::xpath, input)
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
                    let op = match Self::first_token_rule(&op_pair) {
                        Rule::OP_PLUS => BinaryOp::Add,
                        Rule::OP_MINUS => BinaryOp::Subtract,
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
                    let op = match Self::first_token_rule(&op_pair) {
                        Rule::OP_STAR => BinaryOp::Multiply,
                        Rule::K_DIV => BinaryOp::Divide,
                        Rule::K_MOD => BinaryOp::Modulo,
                        // Note: K_IDIV not represented in BinaryOp test model
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
                if let Some(op_pair) = inners.next() {
                    let right = inners.next()?;
                    let left_node = Self::parse_expression_node(&left)?;
                    let right_node = Self::parse_expression_node(&right)?;
                    let op = match Self::first_token_rule(&op_pair) {
                        Rule::K_AND => BinaryOp::And,
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
            Rule::or_expr => {
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                if let Some(op_pair) = inners.next() {
                    let right = inners.next()?;
                    let left_node = Self::parse_expression_node(&left)?;
                    let right_node = Self::parse_expression_node(&right)?;
                    let op = match Self::first_token_rule(&op_pair) {
                        Rule::K_OR => BinaryOp::Or,
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
            Rule::comparison_expr => {
                let mut inners = pair.clone().into_inner();
                let left = inners.next()?;
                if let Some(op_pair) = inners.next() {
                    let right = inners.next()?;
                    let left_node = Self::parse_expression_node(&left)?;
                    let right_node = Self::parse_expression_node(&right)?;
                    let token = Self::first_token_rule(&op_pair);
                    let op = match token {
                        // General comparisons
                        Rule::OP_EQ => BinaryOp::Equal,
                        Rule::OP_NE => BinaryOp::NotEqual,
                        Rule::OP_LT => BinaryOp::LessThan,
                        Rule::OP_LTE => BinaryOp::LessThanOrEqual,
                        Rule::OP_GT => BinaryOp::GreaterThan,
                        Rule::OP_GTE => BinaryOp::GreaterThanOrEqual,
                        // Value comparisons
                        Rule::K_EQ => BinaryOp::Equal,
                        Rule::K_NE => BinaryOp::NotEqual,
                        Rule::K_LT => BinaryOp::LessThan,
                        Rule::K_LE => BinaryOp::LessThanOrEqual,
                        Rule::K_GT => BinaryOp::GreaterThan,
                        Rule::K_GE => BinaryOp::GreaterThanOrEqual,
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
            Rule::string_literal => {
                // With silent wrappers, the only child is *_inner; normalize doubled quotes
                let mut inners = pair.clone().into_inner();
                if let Some(content) = inners.next() {
                    let mut s = content.as_str().to_string();
                    match content.as_rule() {
                        Rule::dbl_string_inner => {
                            s = s.replace("\"\"", "\"");
                        }
                        Rule::sgl_string_inner => {
                            s = s.replace("''", "'");
                        }
                        _ => {}
                    }
                    return Some(ExpressionNode::Literal(s));
                }
                Some(ExpressionNode::Literal(pair.as_str().to_string()))
            }
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
