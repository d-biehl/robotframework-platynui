use platynui_xpath::{
    evaluator::evaluate_expr, runtime::DynamicContextBuilder,
};
use proptest::prelude::*;
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
    let s = seq[0].to_string();
    // Accept Debug variants Double(…) or Decimal(…)
    for prefix in ["Double(", "Decimal("] {
        if let Some(rest) = s.strip_prefix(prefix).and_then(|t| t.strip_suffix(")")) {
            return rest.parse().ok();
        }
    }
    None
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
fn proptest_round_half_to_even_bankers() {
    proptest!(ProptestConfig::with_cases(256), |(x in -100000f64..100000f64, prec in -4i32..8i32)| {
        let expr = format!("round-half-to-even({}, {})", x, prec);
        if let Some(r) = eval_double(&expr) {
            let expected = banker_scaled(x, prec);
            prop_assert!((r - expected).abs() <= 1e-9, "x={}, prec={}, r={}, expected={}", x, prec, r, expected);
        } else {
            prop_assert!(false, "evaluation failed for expression: {}", expr);
        }
    });
}
