use super::*;

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
    let pairs = XPath2Parser::parse_xpath(xpath)
        .unwrap_or_else(|e| panic!("Expression '{}' should parse successfully: {}", xpath, e));
    let ast = XPath2Parser::extract_expression_structure(pairs)
        .unwrap_or_else(|| panic!("Should extract AST structure for: '{}'", xpath));
    assert!(is_well_formed_ast(&ast), "AST should be well-formed for: '{}'", xpath);
    ast
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
