use platynui_xpath::xdm::{XdmAtomicValue as A, XdmItem as I};
use platynui_xpath::{evaluator::evaluate_expr, runtime::DynamicContextBuilder};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

fn eval_double(expr: &str) -> Option<f64> {
    let c = ctx();
    let seq = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &c).ok()?;
    if seq.is_empty() {
        return None;
    }
    match &seq[0] {
        I::Atomic(A::Double(d)) => Some(*d),
        I::Atomic(A::Decimal(d)) => Some(*d),
        other => {
            panic!("Expected Double/Decimal, got {:?}", other);
        }
    }
}

fn banker_scaled(x: f64, p: i32) -> f64 {
    if x.is_nan() || x.is_infinite() {
        return x;
    }
    if p == 0 {
        return half_even(x);
    }
    if p > 0 {
        let factor = 10f64.powi(p);
        return half_even(x * factor) / factor;
    }
    let factor = 10f64.powi(-p);
    half_even(x / factor) * factor
}

fn half_even(v: f64) -> f64 {
    // emulate IEEE round-half-to-even for ties exactly at .5 in magnitude
    let t = v.trunc();
    let frac = v - t;
    if (frac.abs() - 0.5).abs() > 1e-12 {
        // not a tie (epsilon guard)
        return v.round();
    }
    let ti = t as i128; // larger range
    if ti % 2 == 0 { t } else { t + v.signum() }
}

#[rstest]
fn round_half_to_even_bankers_matrix(
    #[values(-100000.0, -2.5, -2.0, -1.5, -1.0, -0.5, -0.0, 0.0, 0.5, 1.5, 2.0, 2.5, 100000.0)] x: f64,
    #[values(-4, -1, 0, 1, 4, 7)] prec: i32,
) {
    let expr = format!("round-half-to-even({}, {})", x, prec);
    if let Some(r) = eval_double(&expr) {
        let expected = banker_scaled(x, prec);
        assert!((r - expected).abs() <= 1e-9, "x={}, prec={}, r={}, expected={}", x, prec, r, expected);
    } else {
        panic!("evaluation failed for expression: {}", expr);
    }
}
