use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as V, xdm::XdmItem as I};
use rstest::rstest;

fn eval(expr: &str) -> Vec<I<platynui_xpath::model::simple::SimpleNode>> {
    let ctx = DynamicContextBuilder::default().build();
    evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx).unwrap()
}

#[rstest]
fn datetime_eq_across_timezones() {
    let out = eval("xs:dateTime('2020-01-01T10:00:00+01:00') eq xs:dateTime('2020-01-01T09:00:00Z')");
    assert!(matches!(&out[0], I::Atomic(V::Boolean(true))));
}

#[rstest]
fn date_relational_with_timezones() {
    // 2020-06-01+02:00 -> 2020-05-31T22:00:00Z > 2020-05-31Z midnight
    let out = eval("xs:date('2020-06-01+02:00') gt xs:date('2020-05-31Z')");
    assert!(matches!(&out[0], I::Atomic(V::Boolean(true))));
}

#[rstest]
fn date_eq_unspecified_uses_implicit_utc() {
    let out = eval("xs:date('2020-01-01Z') eq xs:date('2020-01-01')");
    assert!(matches!(&out[0], I::Atomic(V::Boolean(true))));
}

#[rstest]
fn time_relational_and_eq() {
    let out1 = eval("xs:time('10:00:00+01:00') gt xs:time('08:59:59Z')");
    assert!(matches!(&out1[0], I::Atomic(V::Boolean(true))));
    let out2 = eval("xs:time('10:00:00') eq xs:time('10:00:00Z')");
    assert!(matches!(&out2[0], I::Atomic(V::Boolean(true))));
}

#[rstest]
fn duration_comparisons() {
    let out1 = eval("xs:yearMonthDuration('P1Y3M') gt xs:yearMonthDuration('P14M')");
    assert!(matches!(&out1[0], I::Atomic(V::Boolean(true))));
    let out2 = eval("xs:dayTimeDuration('PT60S') gt xs:dayTimeDuration('PT59S')");
    assert!(matches!(&out2[0], I::Atomic(V::Boolean(true))));
}

#[rstest]
fn temporal_arithmetic_basic() {
    let out1 = eval(
        "xs:dateTime('2020-01-01T00:00:00Z') + xs:dayTimeDuration('PT3600S') eq xs:dateTime('2020-01-01T01:00:00Z')",
    );
    assert!(matches!(&out1[0], I::Atomic(V::Boolean(true))));
    let out2 = eval("xs:date('2020-01-31') + xs:yearMonthDuration('P1M') eq xs:date('2020-02-29')");
    assert!(matches!(&out2[0], I::Atomic(V::Boolean(true))));
    let out3 = eval(
        "xs:dateTime('2020-01-02T00:00:00Z') - xs:dateTime('2020-01-01T23:00:00Z') eq xs:dayTimeDuration('PT3600S')",
    );
    assert!(matches!(&out3[0], I::Atomic(V::Boolean(true))));
}

#[rstest]
fn duration_mul_div() {
    let out1 = eval("xs:dayTimeDuration('PT2S') * 2 eq xs:dayTimeDuration('PT4S')");
    assert!(matches!(&out1[0], I::Atomic(V::Boolean(true))));
    let out2 = eval("xs:yearMonthDuration('P12M') div 2 eq xs:yearMonthDuration('P6M')");
    assert!(matches!(&out2[0], I::Atomic(V::Boolean(true))));
}
