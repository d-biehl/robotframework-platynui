use super::*;
use super::ast;
use rstest::rstest;

// XPath 2.0 Types & Sequence Types (see W3C XPath 2.0 2.5.*)

#[rstest]
fn test_treat_as_item_star_occurrence() {
    let ast = parse_ast("$x treat as item()*");
    if let ast::Expr::TreatAs { expr, ty } = ast {
        assert!(matches!(*expr, ast::Expr::VarRef(_)));
        match ty {
            ast::SequenceType::Typed { item, occ } => {
                assert!(matches!(item, ast::ItemType::Item));
                assert!(matches!(occ, ast::Occurrence::ZeroOrMore));
            }
            other => panic!("Expected Typed sequence type, got: {:?}", other),
        }
    } else {
        panic!("Expected TreatAs expression");
    }
}

#[rstest]
fn test_instance_of_empty_sequence() {
    let ast = parse_ast("() instance of empty-sequence()");
    if let ast::Expr::InstanceOf { expr, ty } = ast {
        // empty parenthesized expr produces EmptySequence literal
        if let ast::Expr::Literal(ast::Literal::EmptySequence) = *expr {} else { panic!("Expected EmptySequence literal"); }
        assert!(matches!(ty, ast::SequenceType::EmptySequence));
    } else {
        panic!("Expected InstanceOf expression");
    }
}

#[rstest]
fn test_cast_as_single_type_optional() {
    let ast = parse_ast("1 cast as xs:integer?");
    if let ast::Expr::CastAs { expr, ty } = ast {
        expect_literal_text(&expr, "1");
        assert_eq!(ty.atomic.local, "integer");
        assert_eq!(ty.atomic.prefix.as_deref(), Some("xs"));
        assert_eq!(ty.optional, true);
    } else {
        panic!("Expected CastAs expression");
    }
}

#[rstest]
fn test_instance_of_item_plus_occurrence() {
    let ast = parse_ast("$v instance of item()+");
    if let ast::Expr::InstanceOf { expr: _, ty } = ast {
        match ty {
            ast::SequenceType::Typed { item, occ } => {
                assert!(matches!(item, ast::ItemType::Item));
                assert!(matches!(occ, ast::Occurrence::OneOrMore));
            }
            _ => panic!("Expected Typed sequence type"),
        }
    } else {
        panic!("Expected InstanceOf expression");
    }
}

#[rstest]
fn test_castable_as_single_type() {
    let ast = parse_ast("'1' castable as xs:int");
    if let ast::Expr::CastableAs { expr, ty } = ast {
        expect_literal_text(&expr, "1");
        assert_eq!(ty.atomic.local, "int");
        assert_eq!(ty.atomic.prefix.as_deref(), Some("xs"));
        assert_eq!(ty.optional, false);
    } else {
        panic!("Expected CastableAs expression");
    }
}
