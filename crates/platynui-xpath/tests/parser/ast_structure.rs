use super::ast;
use super::*;
use rstest::rstest;

// AST Structure Tests - Test operator precedence and AST structure

#[rstest]
fn test_addition_with_multiplication_precedence() {
    // "5 + 3 * 2" should parse as: Add(5, Multiply(3, 2))
    let ast = parse_ast("5 + 3 * 2");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::Add);
    expect_literal_text(left, "5");
    let (mul_left, mul_right) = expect_binary(right, ast::BinaryOp::Mul);
    expect_literal_text(mul_left, "3");
    expect_literal_text(mul_right, "2");
}

#[rstest]
fn test_subtraction_with_multiplication_precedence() {
    // "10 - 2 * 3" should parse as: Subtract(10, Multiply(2, 3))
    let ast = parse_ast("10 - 2 * 3");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::Sub);
    expect_literal_text(left, "10");
    let (mul_left, mul_right) = expect_binary(right, ast::BinaryOp::Mul);
    expect_literal_text(mul_left, "2");
    expect_literal_text(mul_right, "3");
}

#[rstest]
fn test_division_with_addition_precedence() {
    // "8 div 2 + 1" should parse as: Add(Divide(8, 2), 1)
    let ast = parse_ast("8 div 2 + 1");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::Add);
    expect_literal_text(right, "1");
    let (div_left, div_right) = expect_binary(left, ast::BinaryOp::Div);
    expect_literal_text(div_left, "8");
    expect_literal_text(div_right, "2");
}

#[rstest]
fn test_multiplication_division_associativity() {
    // "3 * 4 div 2" should parse: accept both left and right associativity
    let ast = parse_ast("3 * 4 div 2");
    match ast {
        ast::Expr::Binary {
            ref left,
            ref right,
            ..
        } => {
            assert!(
                is_well_formed_ast_expr(left),
                "Left operand should be well-formed"
            );
            assert!(
                is_well_formed_ast_expr(right),
                "Right operand should be well-formed"
            );
        }
        _ => panic!("Expected binary expression for: '3 * 4 div 2'"),
    }
}

#[rstest]
fn test_addition_subtraction_associativity() {
    // "6 + 4 - 2" should parse: accept both left and right associativity
    let ast = parse_ast("6 + 4 - 2");
    match ast {
        ast::Expr::Binary {
            ref left,
            ref op,
            ref right,
        } => {
            assert!(
                *op == ast::BinaryOp::Add || *op == ast::BinaryOp::Sub,
                "Expected Add or Subtract, got: {:?}",
                op
            );
            assert!(
                is_well_formed_ast_expr(left),
                "Left operand should be well-formed"
            );
            assert!(
                is_well_formed_ast_expr(right),
                "Right operand should be well-formed"
            );
        }
        _ => panic!("Expected binary expression for: '6 + 4 - 2'"),
    }
}

#[rstest]
fn test_or_and_precedence() {
    // "true and false or true" should parse as: Or(And(true, false), true)
    let ast = parse_ast("true and false or true");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::Or);
    expect_name_identifier(right, "true");
    let (and_left, and_right) = expect_binary(left, ast::BinaryOp::And);
    expect_name_identifier(and_left, "true");
    expect_name_identifier(and_right, "false");
}

#[rstest]
fn test_comparison_and_precedence() {
    // "1 < 2 and 3 > 2" should parse as: And(LessThan(1, 2), GreaterThan(3, 2))
    let ast = parse_ast("1 < 2 and 3 > 2");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::And);
    let (lt_left, lt_right) = expect_general_cmp(left, ast::GeneralComp::Lt);
    expect_literal_text(lt_left, "1");
    expect_literal_text(lt_right, "2");
    let (gt_left, gt_right) = expect_general_cmp(right, ast::GeneralComp::Gt);
    expect_literal_text(gt_left, "3");
    expect_literal_text(gt_right, "2");
}

#[rstest]
fn test_function_call_and_precedence() {
    // "not(true) and false" should parse as: And(FunctionCall(not, [true]), false)
    let ast = parse_ast("not(true) and false");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::And);
    expect_name_identifier(right, "false");
    let args = expect_function_call(left, "not", 1);
    expect_name_identifier(args[0], "true");
}

#[rstest]
fn test_count_function_call_structure() {
    // "count(//book)" should parse as: FunctionCall("count", [PathExpr])
    let ast = parse_ast("count(//book)");
    let _args = expect_function_call(&ast, "count", 1);
}

#[rstest]
fn test_position_function_call_structure() {
    // "position()" should parse as: FunctionCall("position", [])
    let ast = parse_ast("position()");
    let _args = expect_function_call(&ast, "position", 0);
}

#[rstest]
fn test_substring_function_call_structure() {
    // "substring('test', 1, 2)" should parse as: FunctionCall("substring", [String, Number, Number])
    let ast = parse_ast("substring('test', 1, 2)");
    let _args = expect_function_call(&ast, "substring", 3);
}

#[rstest]
fn test_complex_arithmetic_precedence() {
    // "2 + 3 * 4 - 1" should maintain correct precedence
    let ast = parse_ast("2 + 3 * 4 - 1");
    match ast {
        ast::Expr::Binary {
            ref left,
            ref op,
            ref right,
        } => {
            assert!(
                *op == ast::BinaryOp::Add || *op == ast::BinaryOp::Sub,
                "Expected Add or Subtract, got: {:?}",
                op
            );
            assert!(
                is_well_formed_ast_expr(left),
                "Left operand should be well-formed"
            );
            assert!(
                is_well_formed_ast_expr(right),
                "Right operand should be well-formed"
            );
        }
        _ => panic!("Expected binary expression for: '2 + 3 * 4 - 1'"),
    }
}

#[rstest]
fn test_complex_logical_precedence() {
    // "true or false and not(true)" should maintain correct precedence
    let ast = parse_ast("true or false and not(true)");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::Or);
    assert!(
        is_well_formed_ast_expr(left),
        "Left operand should be well-formed"
    );
    assert!(
        is_well_formed_ast_expr(right),
        "Right operand should be well-formed"
    );
}

#[rstest]
fn test_function_calls_with_logical_operators() {
    // "position() = 1 and last() > 5" should parse correctly
    let ast = parse_ast("position() = 1 and last() > 5");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::And);
    let (le_l, le_r) = expect_general_cmp(left, ast::GeneralComp::Eq);
    let _pos_args = expect_function_call(le_l, "position", 0);
    expect_literal_text(le_r, "1");
    let (gt_l, gt_r) = expect_general_cmp(right, ast::GeneralComp::Gt);
    let _last_args = expect_function_call(gt_l, "last", 0);
    expect_literal_text(gt_r, "5");
}

#[rstest]
fn test_value_comparison_eq() {
    let ast = parse_ast("1 eq 1");
    if let ast::Expr::ValueComparison { left, op, right } = ast {
        assert!(matches!(*left, ast::Expr::Literal(_)));
        assert!(matches!(*right, ast::Expr::Literal(_)));
        assert!(matches!(op, ast::ValueComp::Eq));
    } else {
        panic!("Expected ValueComparison for '1 eq 1'");
    }
}

#[rstest]
fn test_node_comparisons_is_precedes_follows() {
    // $a is $b
    let ast = parse_ast("$a is $b");
    if let ast::Expr::NodeComparison { left, op, right } = ast {
        assert!(matches!(*left, ast::Expr::VarRef(_)));
        assert!(matches!(*right, ast::Expr::VarRef(_)));
        assert!(matches!(op, ast::NodeComp::Is));
    } else {
        panic!("Expected NodeComparison 'is'");
    }

    // $a << $b
    let ast = parse_ast("$a << $b");
    if let ast::Expr::NodeComparison {
        left: _,
        op,
        right: _,
    } = ast
    {
        assert!(matches!(op, ast::NodeComp::Precedes));
    } else {
        panic!("Expected NodeComparison '<<'");
    }

    // $a >> $b
    let ast = parse_ast("$a >> $b");
    if let ast::Expr::NodeComparison {
        left: _,
        op,
        right: _,
    } = ast
    {
        assert!(matches!(op, ast::NodeComp::Follows));
    } else {
        panic!("Expected NodeComparison '>>'");
    }
}

#[rstest]
fn test_range_operator_to() {
    let ast = parse_ast("1 to 5");
    if let ast::Expr::Range { start, end } = ast {
        expect_literal_text(start.as_ref(), "1");
        expect_literal_text(end.as_ref(), "5");
    } else {
        panic!("Expected Range expression for '1 to 5'");
    }
}

#[rstest]
fn test_set_operations_union_intersect_except() {
    let ast = parse_ast("$a union $b");
    if let ast::Expr::SetOp {
        left: _,
        op,
        right: _,
    } = ast
    {
        assert!(matches!(op, ast::SetOp::Union));
    } else {
        panic!("Expected Union");
    }
    let ast = parse_ast("$a intersect $b");
    if let ast::Expr::SetOp {
        left: _,
        op,
        right: _,
    } = ast
    {
        assert!(matches!(op, ast::SetOp::Intersect));
    } else {
        panic!("Expected Intersect");
    }
    let ast = parse_ast("$a except $b");
    if let ast::Expr::SetOp {
        left: _,
        op,
        right: _,
    } = ast
    {
        assert!(matches!(op, ast::SetOp::Except));
    } else {
        panic!("Expected Except");
    }
}

#[rstest]
fn test_idiv_binary_operator() {
    let ast = parse_ast("8 idiv 2");
    let (left, right) = expect_binary(&ast, ast::BinaryOp::IDiv);
    expect_literal_text(left, "8");
    expect_literal_text(right, "2");
}

#[rstest]
fn test_sequence_construction() {
    let ast = parse_ast("1, 2, 3");
    if let ast::Expr::Sequence(items) = ast {
        assert_eq!(items.len(), 3, "Expected 3 items in sequence");
        expect_literal_text(&items[0], "1");
        expect_literal_text(&items[1], "2");
        expect_literal_text(&items[2], "3");
    } else {
        panic!("Expected Sequence expression for '1, 2, 3'");
    }
}

#[rstest]
fn test_postfix_context_item_with_predicates() {
    // self::node()[1]
    let ast = parse_ast("self::node()[1]");
    let path = match ast { ast::Expr::Path(p) => p, other => panic!("Expected path, got {:?}", other) };
    assert_eq!(path.steps.len(), 1);
    let step = &path.steps[0];
    assert!(matches!(step.axis, ast::Axis::SelfAxis));
    assert!(matches!(step.test, ast::NodeTest::Kind(ast::KindTest::AnyKind)));
    assert_eq!(step.predicates.len(), 1);

    // self::node()[@id]
    let ast = parse_ast("self::node()[@id]");
    let path = match ast { ast::Expr::Path(p) => p, other => panic!("Expected path, got {:?}", other) };
    assert_eq!(path.steps[0].predicates.len(), 1);
}

#[rstest]
fn test_attribute_wildcard_step() {
    let ast = parse_ast("@*");
    let path = match ast { ast::Expr::Path(p) => p, other => panic!("Expected path, got {:?}", other) };
    let step = &path.steps[0];
    assert!(matches!(step.axis, ast::Axis::Attribute));
    match &step.test {
        ast::NodeTest::Name(ast::NameTest::Wildcard(ast::WildcardName::Any)) => {}
        other => panic!("Expected attribute wildcard any, got {:?}", other),
    }
}

#[rstest]
fn test_double_slash_expansion_absolute() {
    let ast = parse_ast("//book");
    let path = match ast { ast::Expr::Path(p) => p, other => panic!("Expected path, got {:?}", other) };
    assert!(matches!(path.start, ast::PathStart::Root));
    assert!(path.steps.len() >= 2, "Expected dslash expansion to two steps");
    // First step must be descendant-or-self::node()
    let first = &path.steps[0];
    assert!(matches!(first.axis, ast::Axis::DescendantOrSelf));
    assert!(matches!(first.test, ast::NodeTest::Kind(ast::KindTest::AnyKind)));
    // Second step should be the name test 'book'
    let second = &path.steps[1];
    match &second.test {
        ast::NodeTest::Name(ast::NameTest::QName(qn)) => assert_eq!(qn.local, "book"),
        other => panic!("Expected name test 'book', got {:?}", other),
    }
}

#[rstest]
fn test_name_with_predicate() {
    let ast = parse_ast("a[2]");
    let path = match ast { ast::Expr::Path(p) => p, other => panic!("Expected path, got {:?}", other) };
    let step = &path.steps[0];
    match &step.test {
        ast::NodeTest::Name(ast::NameTest::QName(qn)) => assert_eq!(qn.local, "a"),
        other => panic!("Expected QName 'a', got {:?}", other),
    }
    assert_eq!(step.predicates.len(), 1);
}

// (helper removed; use is_well_formed_ast_expr directly)
