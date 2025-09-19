use platynui_xpath::compiler::{compile, ir::*};
use platynui_xpath::xdm::ExpandedName;
use rstest::rstest;

fn ir(src: &str) -> InstrSeq {
    compile(src).expect("compile ok").instrs
}

#[rstest]
#[case("some $x in (1,2) satisfies $x = 2")]
#[case("every $x in (1,2) satisfies $x ge 1")]
fn quantifiers(#[case] src: &str) {
    let is = ir(src);
    assert!(is.0.iter().any(|op| matches!(op, OpCode::QuantLoop { .. })));
}

#[rstest]
fn for_expr() {
    let is = ir("for $x in (1,2) return $x + 1");
    assert!(is.0.iter().any(|op| matches!(op, OpCode::BeginScope(1))));
    assert!(is.0.iter().any(|op| {
        matches!(
            op,
            OpCode::ForLoop {
                var: ExpandedName { ns_uri: None, local },
                ..
            } if local == "x"
        )
    }));
    assert!(is.0.iter().any(|op| matches!(op, OpCode::EndScope)));
}
