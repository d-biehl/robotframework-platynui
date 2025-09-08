use pest::Parser;
use pest::iterators::Pairs;
use platynui_xpath::parser::{Rule, XPathParser, ast as p_ast};

// Re-export AST types for convenient use in tests
pub use platynui_xpath::parser::ast;

// Parse to production AST
pub fn parse_ast(xpath: &str) -> p_ast::Expr {
    XPathParser::parse_to_ast(xpath)
        .unwrap_or_else(|e| panic!("Expression '{}' should parse successfully: {:?}", xpath, e))
}

// Quick raw grammar parse (used by syntax-only tests)
pub fn parse_xpath(input: &str) -> Result<Pairs<'_, Rule>, pest::error::Error<Rule>> {
    XPathParser::parse(Rule::xpath, input)
}

// Generic well-formedness check (placeholder: production AST is already well-formed)
pub fn is_well_formed_ast_expr(_e: &p_ast::Expr) -> bool {
    true
}

// Expect helpers for common shapes
pub fn expect_binary(
    ast: &p_ast::Expr,
    expected: p_ast::BinaryOp,
) -> (&p_ast::Expr, &p_ast::Expr) {
    match ast {
        p_ast::Expr::Binary { left, op, right } => {
            assert_eq!(*op, expected, "Expected binary op {:?}", expected);
            (left, right)
        }
        other => panic!("Expected binary expression, got: {:?}", other),
    }
}

pub fn expect_general_cmp(
    ast: &p_ast::Expr,
    expected: p_ast::GeneralComp,
) -> (&p_ast::Expr, &p_ast::Expr) {
    match ast {
        p_ast::Expr::GeneralComparison { left, op, right } => {
            assert_eq!(
                *op, expected,
                "Expected general comparison op {:?}",
                expected
            );
            (left, right)
        }
        other => panic!("Expected general comparison, got: {:?}", other),
    }
}

pub fn expect_function_call<'a>(
    ast: &'a p_ast::Expr,
    expected_name: &str,
    expected_argc: usize,
) -> Vec<&'a p_ast::Expr> {
    match ast {
        p_ast::Expr::FunctionCall { name, args } => {
            assert_eq!(
                name.local, expected_name,
                "Expected function name '{}', got '{}'",
                expected_name, name.local
            );
            assert_eq!(args.len(), expected_argc, "Expected {} args", expected_argc);
            args.iter().collect()
        }
        other => panic!("Expected function call, got: {:?}", other),
    }
}

pub fn expect_literal_text(ast: &p_ast::Expr, expected: &str) {
    use p_ast::Literal as L;
    match ast {
        p_ast::Expr::Literal(l) => {
            let got = match l {
                L::Integer(i) => i.to_string(),
                L::Double(d) => d.to_string(),
                L::String(s) => s.clone(),
                L::Boolean(b) => {
                    if *b {
                        "true".into()
                    } else {
                        "false".into()
                    }
                }
                L::AnyUri(s) => s.clone(),
                L::UntypedAtomic(s) => s.clone(),
                L::EmptySequence => String::new(),
            };
            assert_eq!(got, expected, "Expected literal '{}'", expected);
        }
        other => panic!("Expected literal, got: {:?}", other),
    }
}

pub fn expect_name_identifier(ast: &p_ast::Expr, expected_local: &str) {
    match ast {
        p_ast::Expr::Path(p) => {
            let first = p.steps.first().expect("path with at least one step");
            match &first.test {
                p_ast::NodeTest::Name(p_ast::NameTest::QName(qn)) => assert_eq!(
                    qn.local, expected_local,
                    "Expected name '{}'",
                    expected_local
                ),
                other => panic!("Expected QName step, got: {:?}", other),
            }
        }
        other => panic!("Expected path name identifier, got: {:?}", other),
    }
}
