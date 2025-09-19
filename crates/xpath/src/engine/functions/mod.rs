use crate::engine::runtime::{
    FunctionImplementations, FunctionSignatures, Occurrence, ParamTypeSpec,
};
use crate::xdm::ExpandedName;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub mod boolean;
pub mod collations;
mod common;
pub mod constructors;
pub mod datetime;
pub mod diagnostics;
pub mod durations;
pub mod environment;
pub mod ids;
pub mod numeric;
pub mod qnames;
pub mod regex;
pub mod sequences;
pub mod strings;

pub use common::deep_equal_with_collation;
pub(crate) use common::{
    parse_day_time_duration_secs, parse_duration_lexical, parse_qname_lexical,
    parse_year_month_duration_months,
};

fn register_default_functions<N: 'static + Send + Sync + crate::model::XdmNode + Clone>(
    reg: Option<&mut FunctionImplementations<N>>,
    sigs: Option<&mut FunctionSignatures>,
) {
    let mut reg = reg;
    let mut sigs = sigs;
    macro_rules! reg_ns {
        ($ns:expr, $local:expr, $arity:expr, $func:expr $(,)?) => {{
            if let Some(s) = sigs.as_mut() {
                s.register_ns($ns, $local, $arity, Some($arity));
            }
            if let Some(r) = reg.as_mut() {
                r.register_ns($ns, $local, $arity, $func);
            }
        }};
        ($ns:expr, $local:expr, $arity:expr, $func:expr, $param_specs:expr $(,)?) => {{
            if let Some(s) = sigs.as_mut() {
                s.register_ns($ns, $local, $arity, Some($arity));
                let name = ExpandedName {
                    ns_uri: Some($ns.to_string()),
                    local: $local.to_string(),
                };
                s.set_param_types(name, $arity, $param_specs);
            }
            if let Some(r) = reg.as_mut() {
                r.register_ns($ns, $local, $arity, $func);
            }
        }};
    }
    macro_rules! reg_ns_range {
        ($ns:expr, $local:expr, $min:expr, $max:expr, $func:expr $(,)?) => {{
            if let Some(s) = sigs.as_mut() {
                s.register_ns($ns, $local, $min, $max);
            }
            if let Some(r) = reg.as_mut() {
                r.register_ns_range($ns, $local, $min, $max, $func);
            }
        }};
        ($ns:expr, $local:expr, $min:expr, $max:expr, $func:expr, { $($arity:expr => $param_specs:expr),+ $(,)? }) => {{
            if let Some(s) = sigs.as_mut() {
                s.register_ns($ns, $local, $min, $max);
                let name = ExpandedName {
                    ns_uri: Some($ns.to_string()),
                    local: $local.to_string(),
                };
                $(
                    s.set_param_types(name.clone(), $arity, $param_specs);
                )+
            }
            if let Some(r) = reg.as_mut() {
                r.register_ns_range($ns, $local, $min, $max, $func);
            }
        }};
    }
    macro_rules! reg_ns_variadic {
        ($ns:expr, $local:expr, $min:expr, $func:expr $(,)?) => {{
            if let Some(s) = sigs.as_mut() {
                s.register_ns($ns, $local, $min, None);
            }
            if let Some(r) = reg.as_mut() {
                r.register_ns_variadic($ns, $local, $min, $func);
            }
        }};
        ($ns:expr, $local:expr, $min:expr, $func:expr, { $($arity:expr => $param_specs:expr),+ $(,)? }) => {{
            if let Some(s) = sigs.as_mut() {
                s.register_ns($ns, $local, $min, None);
                let name = ExpandedName {
                    ns_uri: Some($ns.to_string()),
                    local: $local.to_string(),
                };
                $(
                    s.set_param_types(name.clone(), $arity, $param_specs);
                )+
            }
            if let Some(r) = reg.as_mut() {
                r.register_ns_variadic($ns, $local, $min, $func);
            }
        }};
    }

    // ===== Core booleans =====
    reg_ns!(crate::consts::FNS, "true", 0, boolean::fn_true::<N>);
    reg_ns!(crate::consts::FNS, "false", 0, boolean::fn_false::<N>);
    reg_ns_range!(
        crate::consts::FNS,
        "data",
        0,
        Some(1),
        boolean::data_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "not",
        1,
        boolean::fn_not::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns!(
        crate::consts::FNS,
        "boolean",
        1,
        boolean::fn_boolean::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );

    // ===== Numeric core =====
    reg_ns_range!(
        crate::consts::FNS,
        "number",
        0,
        Some(1),
        numeric::number_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrOne)]
        }
    );

    // ===== String family =====
    reg_ns_range!(
        crate::consts::FNS,
        "string",
        0,
        Some(1),
        strings::string_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrOne)]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "string-length",
        0,
        Some(1),
        strings::string_length_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "untypedAtomic",
        1,
        strings::untyped_atomic_fn::<N>
    );
    reg_ns_variadic!(crate::consts::FNS, "concat", 2, strings::concat_fn::<N>);
    reg_ns!(
        crate::consts::FNS,
        "string-to-codepoints",
        1,
        strings::string_to_codepoints_fn::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "codepoints-to-string",
        1,
        strings::codepoints_to_string_fn::<N>
    );
    reg_ns_range!(
        crate::consts::FNS,
        "contains",
        2,
        Some(3),
        strings::contains_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ],
            3 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "starts-with",
        2,
        Some(3),
        strings::starts_with_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ],
            3 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "ends-with",
        2,
        Some(3),
        strings::ends_with_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ],
            3 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "substring",
        2,
        Some(3),
        strings::substring_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::double(Occurrence::ExactlyOne),
            ],
            3 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::double(Occurrence::ExactlyOne),
                ParamTypeSpec::double(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "substring-before",
        2,
        strings::substring_before_fn::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns!(
        crate::consts::FNS,
        "substring-after",
        2,
        strings::substring_after_fn::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "normalize-space",
        0,
        Some(1),
        strings::normalize_space_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "translate",
        3,
        strings::translate_fn::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
        ]
    );
    reg_ns!(
        crate::consts::FNS,
        "lower-case",
        1,
        strings::lower_case_fn::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "upper-case",
        1,
        strings::upper_case_fn::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "string-join",
        2,
        strings::string_join_fn::<N>,
        vec![
            ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
        ]
    );

    // ===== Node name functions =====
    reg_ns!(
        crate::consts::FNS,
        "node-name",
        1,
        qnames::node_name_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "name",
        0,
        Some(1),
        qnames::name_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "local-name",
        0,
        Some(1),
        qnames::local_name_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
        }
    );

    // ===== QName / Namespace functions =====
    reg_ns!(
        crate::consts::FNS,
        "QName",
        2,
        qnames::qname_fn::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
        ]
    );
    reg_ns!(
        crate::consts::FNS,
        "resolve-QName",
        2,
        qnames::resolve_qname_fn::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::any_item(Occurrence::ExactlyOne),
        ]
    );
    reg_ns!(
        crate::consts::FNS,
        "namespace-uri-from-QName",
        1,
        qnames::namespace_uri_from_qname_fn::<N>,
        vec![ParamTypeSpec::qname(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "local-name-from-QName",
        1,
        qnames::local_name_from_qname_fn::<N>,
        vec![ParamTypeSpec::qname(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "prefix-from-QName",
        1,
        qnames::prefix_from_qname_fn::<N>,
        vec![ParamTypeSpec::qname(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "namespace-uri-for-prefix",
        2,
        qnames::namespace_uri_for_prefix_fn::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::any_item(Occurrence::ExactlyOne),
        ]
    );
    reg_ns!(
        crate::consts::FNS,
        "in-scope-prefixes",
        1,
        qnames::in_scope_prefixes_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ExactlyOne)]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "namespace-uri",
        0,
        Some(1),
        qnames::namespace_uri_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
        }
    );

    // ===== Numeric family =====
    reg_ns!(
        crate::consts::FNS,
        "abs",
        1,
        numeric::abs_fn::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "floor",
        1,
        numeric::floor_fn::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "ceiling",
        1,
        numeric::ceiling_fn::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "round",
        1,
        Some(2),
        numeric::round_fn::<N>,
        {
            1 => vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)],
            2 => vec![
                ParamTypeSpec::numeric(Occurrence::ZeroOrOne),
                ParamTypeSpec::numeric(Occurrence::ExactlyOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "round-half-to-even",
        1,
        Some(2),
        numeric::round_half_to_even_fn::<N>,
        {
            1 => vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)],
            2 => vec![
                ParamTypeSpec::numeric(Occurrence::ZeroOrOne),
                ParamTypeSpec::numeric(Occurrence::ExactlyOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "sum",
        1,
        Some(2),
        numeric::sum_fn::<N>,
        {
            1 => vec![ParamTypeSpec::numeric(Occurrence::ZeroOrMore)],
            2 => vec![
                ParamTypeSpec::numeric(Occurrence::ZeroOrMore),
                ParamTypeSpec::numeric(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "avg",
        1,
        numeric::avg_fn::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrMore)]
    );

    // ===== Sequence family =====
    reg_ns!(
        crate::consts::FNS,
        "empty",
        1,
        sequences::empty_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns!(
        crate::consts::FNS,
        "exists",
        1,
        sequences::exists_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns!(
        crate::consts::FNS,
        "count",
        1,
        sequences::count_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns!(
        crate::consts::FNS,
        "exactly-one",
        1,
        sequences::exactly_one_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns!(
        crate::consts::FNS,
        "one-or-more",
        1,
        sequences::one_or_more_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns!(
        crate::consts::FNS,
        "zero-or-one",
        1,
        sequences::zero_or_one_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns!(
        crate::consts::FNS,
        "reverse",
        1,
        sequences::reverse_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "subsequence",
        2,
        Some(3),
        sequences::subsequence_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
                ParamTypeSpec::double(Occurrence::ExactlyOne),
            ],
            3 => vec![
                ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
                ParamTypeSpec::double(Occurrence::ExactlyOne),
                ParamTypeSpec::double(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "distinct-values",
        1,
        Some(2),
        sequences::distinct_values_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore)],
            2 => vec![
                ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "index-of",
        2,
        Some(3),
        sequences::index_of_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_atomic(Occurrence::ExactlyOne),
            ],
            3 => vec![
                ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_atomic(Occurrence::ExactlyOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "insert-before",
        3,
        sequences::insert_before_fn::<N>,
        vec![
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
            ParamTypeSpec::double(Occurrence::ExactlyOne),
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
        ]
    );
    reg_ns!(
        crate::consts::FNS,
        "remove",
        2,
        sequences::remove_fn::<N>,
        vec![
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
            ParamTypeSpec::double(Occurrence::ExactlyOne),
        ]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "min",
        1,
        Some(2),
        numeric::min_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore)],
            2 => vec![
                ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_atomic(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "max",
        1,
        Some(2),
        numeric::max_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore)],
            2 => vec![
                ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_atomic(Occurrence::ZeroOrOne),
            ]
        }
    );

    // ===== Collation-related functions =====
    reg_ns_range!(
        crate::consts::FNS,
        "compare",
        2,
        Some(3),
        collations::compare_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ],
            3 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "codepoint-equal",
        2,
        collations::codepoint_equal_fn::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "deep-equal",
        2,
        Some(3),
        collations::deep_equal_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
            ],
            3 => vec![
                ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );

    // ===== Regex family =====
    reg_ns_range!(
        crate::consts::FNS,
        "matches",
        2,
        Some(3),
        regex::matches_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ExactlyOne),
            ],
            3 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ExactlyOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "replace",
        3,
        Some(4),
        regex::replace_fn::<N>,
        {
            3 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ExactlyOne),
                ParamTypeSpec::string(Occurrence::ExactlyOne),
            ],
            4 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ExactlyOne),
                ParamTypeSpec::string(Occurrence::ExactlyOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "tokenize",
        2,
        Some(3),
        regex::tokenize_fn::<N>,
        {
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ExactlyOne),
            ],
            3 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ExactlyOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );

    // ===== Diagnostics =====
    reg_ns_range!(
        crate::consts::FNS,
        "error",
        0,
        Some(3),
        diagnostics::error_fn::<N>
    );
    reg_ns!(crate::consts::FNS, "trace", 2, diagnostics::trace_fn::<N>);

    // ===== Environment / Document / URI helpers =====
    reg_ns!(
        crate::consts::FNS,
        "default-collation",
        0,
        environment::default_collation_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "static-base-uri",
        0,
        environment::static_base_uri_fn::<N>
    );
    reg_ns_range!(
        crate::consts::FNS,
        "root",
        0,
        Some(1),
        environment::root_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "base-uri",
        0,
        Some(1),
        environment::base_uri_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "document-uri",
        0,
        Some(1),
        environment::document_uri_fn::<N>,
        {
            1 => vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "lang",
        1,
        Some(2),
        environment::lang_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)],
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::any_item(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "encode-for-uri",
        1,
        environment::encode_for_uri_fn::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "nilled",
        1,
        environment::nilled_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "iri-to-uri",
        1,
        environment::iri_to_uri_fn::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "escape-html-uri",
        1,
        environment::escape_html_uri_fn::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "resolve-uri",
        1,
        Some(2),
        environment::resolve_uri_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)],
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "normalize-unicode",
        1,
        Some(2),
        environment::normalize_unicode_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)],
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
                ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns!(
        crate::consts::FNS,
        "doc-available",
        1,
        environment::doc_available_fn::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "doc",
        1,
        environment::doc_fn::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_range!(
        crate::consts::FNS,
        "collection",
        0,
        Some(1),
        environment::collection_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
        }
    );

    // ===== ID / IDREF helpers =====
    reg_ns_range!(
        crate::consts::FNS,
        "id",
        1,
        Some(2),
        ids::id_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrMore)],
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_item(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "element-with-id",
        1,
        Some(2),
        ids::element_with_id_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrMore)],
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_item(Occurrence::ZeroOrOne),
            ]
        }
    );
    reg_ns_range!(
        crate::consts::FNS,
        "idref",
        1,
        Some(2),
        ids::idref_fn::<N>,
        {
            1 => vec![ParamTypeSpec::string(Occurrence::ZeroOrMore)],
            2 => vec![
                ParamTypeSpec::string(Occurrence::ZeroOrMore),
                ParamTypeSpec::any_item(Occurrence::ZeroOrOne),
            ]
        }
    );

    // ===== Regex replacements already handled =====
    reg_ns!(
        crate::consts::FNS,
        "unordered",
        1,
        sequences::unordered_fn::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );

    // ===== Misc constructors =====
    reg_ns!(
        crate::consts::FNS,
        "integer",
        1,
        constructors::integer_fn::<N>
    );

    // ===== Date/Time family =====
    reg_ns!(
        crate::consts::FNS,
        "dateTime",
        2,
        datetime::date_time_fn::<N>
    );
    reg_ns_range!(
        crate::consts::FNS,
        "adjust-date-to-timezone",
        1,
        Some(2),
        datetime::adjust_date_to_timezone_fn::<N>
    );
    reg_ns_range!(
        crate::consts::FNS,
        "adjust-time-to-timezone",
        1,
        Some(2),
        datetime::adjust_time_to_timezone_fn::<N>,
    );
    reg_ns_range!(
        crate::consts::FNS,
        "adjust-dateTime-to-timezone",
        1,
        Some(2),
        datetime::adjust_datetime_to_timezone_fn::<N>,
    );
    reg_ns!(
        crate::consts::FNS,
        "current-dateTime",
        0,
        datetime::current_datetime_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "current-date",
        0,
        datetime::current_date_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "current-time",
        0,
        datetime::current_time_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "implicit-timezone",
        0,
        datetime::implicit_timezone_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "year-from-dateTime",
        1,
        datetime::year_from_datetime_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "hours-from-dateTime",
        1,
        datetime::hours_from_datetime_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "minutes-from-dateTime",
        1,
        datetime::minutes_from_datetime_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "seconds-from-dateTime",
        1,
        datetime::seconds_from_datetime_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "month-from-dateTime",
        1,
        datetime::month_from_datetime_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "day-from-dateTime",
        1,
        datetime::day_from_datetime_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "hours-from-time",
        1,
        datetime::hours_from_time_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "minutes-from-time",
        1,
        datetime::minutes_from_time_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "seconds-from-time",
        1,
        datetime::seconds_from_time_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "timezone-from-dateTime",
        1,
        datetime::timezone_from_datetime_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "timezone-from-date",
        1,
        datetime::timezone_from_date_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "timezone-from-time",
        1,
        datetime::timezone_from_time_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "year-from-date",
        1,
        datetime::year_from_date_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "month-from-date",
        1,
        datetime::month_from_date_fn::<N>
    );
    reg_ns!(
        crate::consts::FNS,
        "day-from-date",
        1,
        datetime::day_from_date_fn::<N>
    );

    // ===== Duration component accessors =====
    reg_ns!(
        crate::consts::FNS,
        "years-from-duration",
        1,
        durations::years_from_duration_fn::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "months-from-duration",
        1,
        durations::months_from_duration_fn::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "days-from-duration",
        1,
        durations::days_from_duration_fn::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "hours-from-duration",
        1,
        durations::hours_from_duration_fn::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "minutes-from-duration",
        1,
        durations::minutes_from_duration_fn::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns!(
        crate::consts::FNS,
        "seconds-from-duration",
        1,
        durations::seconds_from_duration_fn::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );

    // ===== XML Schema constructors =====
    reg_ns!(
        crate::consts::XS,
        "string",
        1,
        constructors::xs_string_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "untypedAtomic",
        1,
        constructors::xs_untyped_atomic_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "boolean",
        1,
        constructors::xs_boolean_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "integer",
        1,
        constructors::xs_integer_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "decimal",
        1,
        constructors::xs_decimal_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "double",
        1,
        constructors::xs_double_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "float",
        1,
        constructors::xs_float_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "anyURI",
        1,
        constructors::xs_any_uri_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "QName",
        1,
        constructors::xs_qname_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "base64Binary",
        1,
        constructors::xs_base64_binary_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "hexBinary",
        1,
        constructors::xs_hex_binary_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "dateTime",
        1,
        constructors::xs_datetime_fn::<N>
    );
    reg_ns!(crate::consts::XS, "date", 1, constructors::xs_date_fn::<N>);
    reg_ns!(crate::consts::XS, "time", 1, constructors::xs_time_fn::<N>);
    reg_ns!(
        crate::consts::XS,
        "duration",
        1,
        constructors::xs_duration_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "dayTimeDuration",
        1,
        constructors::xs_day_time_duration_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "yearMonthDuration",
        1,
        constructors::xs_year_month_duration_fn::<N>,
    );
    reg_ns!(
        crate::consts::XS,
        "gYear",
        1,
        constructors::xs_g_year_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "gYearMonth",
        1,
        constructors::xs_g_year_month_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "gMonth",
        1,
        constructors::xs_g_month_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "gMonthDay",
        1,
        constructors::xs_g_month_day_fn::<N>
    );
    reg_ns!(crate::consts::XS, "gDay", 1, constructors::xs_g_day_fn::<N>);
    reg_ns!(crate::consts::XS, "long", 1, constructors::xs_long_fn::<N>);
    reg_ns!(crate::consts::XS, "int", 1, constructors::xs_int_fn::<N>);
    reg_ns!(
        crate::consts::XS,
        "short",
        1,
        constructors::xs_short_fn::<N>
    );
    reg_ns!(crate::consts::XS, "byte", 1, constructors::xs_byte_fn::<N>);
    reg_ns!(
        crate::consts::XS,
        "unsignedLong",
        1,
        constructors::xs_unsigned_long_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "unsignedInt",
        1,
        constructors::xs_unsigned_int_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "unsignedShort",
        1,
        constructors::xs_unsigned_short_fn::<N>,
    );
    reg_ns!(
        crate::consts::XS,
        "unsignedByte",
        1,
        constructors::xs_unsigned_byte_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "nonPositiveInteger",
        1,
        constructors::xs_non_positive_integer_fn::<N>,
    );
    reg_ns!(
        crate::consts::XS,
        "negativeInteger",
        1,
        constructors::xs_negative_integer_fn::<N>,
    );
    reg_ns!(
        crate::consts::XS,
        "nonNegativeInteger",
        1,
        constructors::xs_non_negative_integer_fn::<N>,
    );
    reg_ns!(
        crate::consts::XS,
        "positiveInteger",
        1,
        constructors::xs_positive_integer_fn::<N>,
    );
    reg_ns!(
        crate::consts::XS,
        "normalizedString",
        1,
        constructors::xs_normalized_string_fn::<N>,
    );
    reg_ns!(
        crate::consts::XS,
        "token",
        1,
        constructors::xs_token_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "language",
        1,
        constructors::xs_language_fn::<N>
    );
    reg_ns!(crate::consts::XS, "Name", 1, constructors::xs_name_fn::<N>);
    reg_ns!(
        crate::consts::XS,
        "NCName",
        1,
        constructors::xs_ncname_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "NMTOKEN",
        1,
        constructors::xs_nmtoken_fn::<N>
    );
    reg_ns!(crate::consts::XS, "ID", 1, constructors::xs_id_fn::<N>);
    reg_ns!(
        crate::consts::XS,
        "IDREF",
        1,
        constructors::xs_idref_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "ENTITY",
        1,
        constructors::xs_entity_fn::<N>
    );
    reg_ns!(
        crate::consts::XS,
        "NOTATION",
        1,
        constructors::xs_notation_fn::<N>
    );
}

pub fn default_function_registry<N: 'static + Send + Sync + crate::model::XdmNode + Clone>()
-> Arc<FunctionImplementations<N>> {
    static CACHE: OnceLock<Mutex<HashMap<TypeId, Box<dyn Any + Send + Sync>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map
        .lock()
        .expect("default function registry cache poisoned");
    let type_id = TypeId::of::<N>();
    if let Some(existing) = guard.get(&type_id) {
        let arc = existing
            .downcast_ref::<Arc<FunctionImplementations<N>>>()
            .expect("cached registry type mismatch");
        return arc.clone();
    }

    let mut reg: FunctionImplementations<N> = FunctionImplementations::new();
    ensure_default_signatures();
    register_default_functions(Some(&mut reg), None);
    let arc = Arc::new(reg);
    guard.insert(type_id, Box::new(arc.clone()));
    arc
}

pub fn default_function_signatures() -> FunctionSignatures {
    ensure_default_signatures().clone()
}

fn ensure_default_signatures() -> &'static FunctionSignatures {
    static SIGS: OnceLock<FunctionSignatures> = OnceLock::new();
    SIGS.get_or_init(|| {
        let mut sigs = FunctionSignatures::default();
        register_default_functions::<crate::model::simple::SimpleNode>(None, Some(&mut sigs));
        sigs
    })
}
