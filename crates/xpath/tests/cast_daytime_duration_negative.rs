//! Targeted negative test: casting bare "PT" to xs:dayTimeDuration must fail.
//!
//! XPath / XML Schema lexical rules require at least one component after the designator:
//!   dayTimeDuration ::= 'P' ( <days>'D' )? ( 'T' ( <hours>'H'? <minutes>'M'? <seconds>'S'? ) )?
//! A bare "PT" (no H/M/S) is therefore invalid and must raise FORG0001.
//!
//! We already cover this in the casting matrix, but this focused test guards against
//! accidental future regression if the matrix is refactored.

use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

#[rstest]
fn cast_day_time_duration_bare_pt_invalid() {
    let ctx = DynamicContextBuilder::default().build();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("'PT' cast as xs:dayTimeDuration", &ctx)
        .expect_err("expected lexical error for bare PT");
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}
