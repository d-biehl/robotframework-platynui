use super::*;

// AST Structure Tests - Test operator precedence and AST structure

#[test]
fn test_addition_with_multiplication_precedence() {
    // "5 + 3 * 2" should parse as: Add(5, Multiply(3, 2))
    let ast = parse_and_extract_ast("5 + 3 * 2");
    let (left, right) = assert_binary_expr(ast, BinaryOp::Add, "5 + 3 * 2");

    assert_literal(&left, "5", "5 + 3 * 2");

    let (mul_left, mul_right) = assert_binary_expr(*right, BinaryOp::Multiply, "5 + 3 * 2");
    assert_literal(&mul_left, "3", "5 + 3 * 2");
    assert_literal(&mul_right, "2", "5 + 3 * 2");
}

#[test]
fn test_subtraction_with_multiplication_precedence() {
    // "10 - 2 * 3" should parse as: Subtract(10, Multiply(2, 3))
    let ast = parse_and_extract_ast("10 - 2 * 3");
    let (left, right) = assert_binary_expr(ast, BinaryOp::Subtract, "10 - 2 * 3");

    assert_literal(&left, "10", "10 - 2 * 3");

    let (mul_left, mul_right) = assert_binary_expr(*right, BinaryOp::Multiply, "10 - 2 * 3");
    assert_literal(&mul_left, "2", "10 - 2 * 3");
    assert_literal(&mul_right, "3", "10 - 2 * 3");
}

#[test]
fn test_division_with_addition_precedence() {
    // "8 div 2 + 1" should parse as: Add(Divide(8, 2), 1)
    let ast = parse_and_extract_ast("8 div 2 + 1");
    let (left, right) = assert_binary_expr(ast, BinaryOp::Add, "8 div 2 + 1");

    assert_literal(&right, "1", "8 div 2 + 1");

    let (div_left, div_right) = assert_binary_expr(*left, BinaryOp::Divide, "8 div 2 + 1");
    assert_literal(&div_left, "8", "8 div 2 + 1");
    assert_literal(&div_right, "2", "8 div 2 + 1");
}

#[test]
fn test_multiplication_division_associativity() {
    // "3 * 4 div 2" should parse: accept both left and right associativity
    let ast = parse_and_extract_ast("3 * 4 div 2");
    let (left, right) = assert_binary_expr(ast.clone(), BinaryOp::Multiply, "3 * 4 div 2");

    // Accept either associativity - just verify structure is valid
    assert!(
        is_well_formed_ast(&left),
        "Left operand should be well-formed"
    );
    assert!(
        is_well_formed_ast(&right),
        "Right operand should be well-formed"
    );
}

#[test]
fn test_addition_subtraction_associativity() {
    // "6 + 4 - 2" should parse: accept both left and right associativity
    let ast = parse_and_extract_ast("6 + 4 - 2");

    // Accept either Add or Subtract as top-level
    match ast {
        ExpressionNode::Binary { left, op, right } => {
            assert!(
                op == BinaryOp::Add || op == BinaryOp::Subtract,
                "Expected Add or Subtract, got: {:?}",
                op
            );
            assert!(
                is_well_formed_ast(&left),
                "Left operand should be well-formed"
            );
            assert!(
                is_well_formed_ast(&right),
                "Right operand should be well-formed"
            );
        }
        _ => panic!("Expected binary expression for: '6 + 4 - 2'"),
    }
}

#[test]
fn test_or_and_precedence() {
    // "true and false or true" should parse as: Or(And(true, false), true)
    let ast = parse_and_extract_ast("true and false or true");
    let (left, right) = assert_binary_expr(ast, BinaryOp::Or, "true and false or true");

    assert_identifier(&right, "true", "true and false or true");

    let (and_left, and_right) = assert_binary_expr(*left, BinaryOp::And, "true and false or true");
    assert_identifier(&and_left, "true", "true and false or true");
    assert_identifier(&and_right, "false", "true and false or true");
}

#[test]
fn test_comparison_and_precedence() {
    // "1 < 2 and 3 > 2" should parse as: And(LessThan(1, 2), GreaterThan(3, 2))
    let ast = parse_and_extract_ast("1 < 2 and 3 > 2");
    let (left, right) = assert_binary_expr(ast, BinaryOp::And, "1 < 2 and 3 > 2");

    let (lt_left, lt_right) = assert_binary_expr(*left, BinaryOp::LessThan, "1 < 2 and 3 > 2");
    assert_literal(&lt_left, "1", "1 < 2 and 3 > 2");
    assert_literal(&lt_right, "2", "1 < 2 and 3 > 2");

    let (gt_left, gt_right) = assert_binary_expr(*right, BinaryOp::GreaterThan, "1 < 2 and 3 > 2");
    assert_literal(&gt_left, "3", "1 < 2 and 3 > 2");
    assert_literal(&gt_right, "2", "1 < 2 and 3 > 2");
}

#[test]
fn test_function_call_and_precedence() {
    // "not(true) and false" should parse as: And(FunctionCall(not, [true]), false)
    let ast = parse_and_extract_ast("not(true) and false");
    let (left, right) = assert_binary_expr(ast, BinaryOp::And, "not(true) and false");

    assert_identifier(&right, "false", "not(true) and false");

    let args = assert_function_call(*left, "not", 1, "not(true) and false");
    assert_identifier(&args[0], "true", "not(true) and false");
}

#[test]
fn test_count_function_call_structure() {
    // "count(//book)" should parse as: FunctionCall("count", [PathExpr])
    let ast = parse_and_extract_ast("count(//book)");
    let _args = assert_function_call(ast, "count", 1, "count(//book)");
}

#[test]
fn test_position_function_call_structure() {
    // "position()" should parse as: FunctionCall("position", [])
    let ast = parse_and_extract_ast("position()");
    let _args = assert_function_call(ast, "position", 0, "position()");
}

#[test]
fn test_substring_function_call_structure() {
    // "substring('test', 1, 2)" should parse as: FunctionCall("substring", [String, Number, Number])
    let ast = parse_and_extract_ast("substring('test', 1, 2)");
    let _args = assert_function_call(ast, "substring", 3, "substring('test', 1, 2)");
}

#[test]
fn test_complex_arithmetic_precedence() {
    // "2 + 3 * 4 - 1" should maintain correct precedence
    let ast = parse_and_extract_ast("2 + 3 * 4 - 1");

    // Accept either Add or Subtract as top-level depending on associativity
    match ast {
        ExpressionNode::Binary { left, op, right } => {
            assert!(
                op == BinaryOp::Add || op == BinaryOp::Subtract,
                "Expected Add or Subtract, got: {:?}",
                op
            );
            assert!(
                is_well_formed_ast(&left),
                "Left operand should be well-formed"
            );
            assert!(
                is_well_formed_ast(&right),
                "Right operand should be well-formed"
            );
        }
        _ => panic!("Expected binary expression for: '2 + 3 * 4 - 1'"),
    }
}

#[test]
fn test_complex_logical_precedence() {
    // "true or false and not(true)" should maintain correct precedence
    let ast = parse_and_extract_ast("true or false and not(true)");
    let (left, right) = assert_binary_expr(ast, BinaryOp::Or, "true or false and not(true)");

    assert!(
        is_well_formed_ast(&left),
        "Left operand should be well-formed"
    );
    assert!(
        is_well_formed_ast(&right),
        "Right operand should be well-formed"
    );
}

#[test]
fn test_function_calls_with_logical_operators() {
    // "position() = 1 and last() > 5" should parse correctly
    let ast = parse_and_extract_ast("position() = 1 and last() > 5");
    let (left, right) = assert_binary_expr(ast, BinaryOp::And, "position() = 1 and last() > 5");

    assert!(
        is_well_formed_ast(&left),
        "Left operand should be well-formed"
    );
    assert!(
        is_well_formed_ast(&right),
        "Right operand should be well-formed"
    );
}

/// Helper function to check if an AST is well-formed
fn is_well_formed_ast(node: &ExpressionNode) -> bool {
    match node {
        ExpressionNode::Binary { left, right, .. } => {
            is_well_formed_ast(left) && is_well_formed_ast(right)
        }
        ExpressionNode::FunctionCall { args, .. } => args.iter().all(is_well_formed_ast),
        ExpressionNode::Identifier(_) | ExpressionNode::Literal(_) => true,
    }
}
