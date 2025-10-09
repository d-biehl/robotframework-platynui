use crate::engine::runtime::{FunctionImplementations, FunctionSignatures, Occurrence, ParamTypeSpec};
use crate::xdm::ExpandedName;
use std::rc::Rc;
use std::sync::OnceLock;

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
    parse_day_time_duration_secs, parse_duration_lexical, parse_qname_lexical, parse_year_month_duration_months,
};

fn register_default_functions<N: 'static + crate::model::XdmNode + Clone>(
    reg: Option<&mut FunctionImplementations<N>>,
    sigs: Option<&mut FunctionSignatures>,
) {
    let mut reg = reg;
    let mut sigs = sigs;
    // Stream-based function registration (preferred for new implementations)
    macro_rules! reg_ns_stream {
        ($ns:expr, $local:expr, $arity:expr, $func:expr $(,)?) => {{
            if let Some(s) = sigs.as_mut() {
                s.register_ns($ns, $local, $arity, Some($arity));
            }
            if let Some(r) = reg.as_mut() {
                r.register_stream_ns($ns, $local, $arity, $func);
            }
        }};
        ($ns:expr, $local:expr, $arity:expr, $func:expr, $param_specs:expr $(,)?) => {{
            if let Some(s) = sigs.as_mut() {
                s.register_ns($ns, $local, $arity, Some($arity));
                let name = ExpandedName { ns_uri: Some($ns.to_string()), local: $local.to_string() };
                s.set_param_types(name, $arity, $param_specs);
            }
            if let Some(r) = reg.as_mut() {
                r.register_stream_ns($ns, $local, $arity, $func);
            }
        }};
    }

    // ===== Core booleans =====
    reg_ns_stream!(crate::consts::FNS, "true", 0, boolean::fn_true_stream::<N>, vec![]);
    reg_ns_stream!(crate::consts::FNS, "false", 0, boolean::fn_false_stream::<N>, vec![]);
    reg_ns_stream!(crate::consts::FNS, "data", 0, boolean::data_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "data",
        1,
        boolean::data_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "not",
        1,
        boolean::fn_not_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "boolean",
        1,
        boolean::fn_boolean_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );

    // ===== Numeric core =====
    reg_ns_stream!(crate::consts::FNS, "number", 0, numeric::number_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "number",
        1,
        numeric::number_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrOne)]
    );

    // ===== String family =====
    reg_ns_stream!(crate::consts::FNS, "string", 0, strings::string_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "string",
        1,
        strings::string_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(crate::consts::FNS, "string-length", 0, strings::string_length_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "string-length",
        1,
        strings::string_length_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "untypedAtomic",
        1,
        strings::untyped_atomic_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ExactlyOne)]
    );
    // concat() is variadic (2+ args)
    if let Some(s) = sigs.as_mut() {
        s.register_ns(crate::consts::FNS, "concat", 2, None);
    }
    if let Some(r) = reg.as_mut() {
        r.register_stream_ns_variadic(crate::consts::FNS, "concat", 2, strings::concat_stream::<N>);
    }
    reg_ns_stream!(
        crate::consts::FNS,
        "string-to-codepoints",
        1,
        strings::string_to_codepoints_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "codepoints-to-string",
        1,
        strings::codepoints_to_string_stream::<N>,
        vec![ParamTypeSpec::integer(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "contains",
        2,
        strings::contains_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "contains",
        3,
        strings::contains_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "starts-with",
        2,
        strings::starts_with_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "starts-with",
        3,
        strings::starts_with_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "ends-with",
        2,
        strings::ends_with_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "ends-with",
        3,
        strings::ends_with_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "substring",
        2,
        strings::substring_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::double(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "substring",
        3,
        strings::substring_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::double(Occurrence::ExactlyOne),
            ParamTypeSpec::double(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "substring-before",
        2,
        strings::substring_before_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "substring-after",
        2,
        strings::substring_after_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(crate::consts::FNS, "normalize-space", 0, strings::normalize_space_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "normalize-space",
        1,
        strings::normalize_space_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "translate",
        3,
        strings::translate_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "lower-case",
        1,
        strings::lower_case_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "upper-case",
        1,
        strings::upper_case_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    // Stream-based string-join() with separator
    reg_ns_stream!(
        crate::consts::FNS,
        "string-join",
        2,
        strings::string_join_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore), ParamTypeSpec::string(Occurrence::ExactlyOne),]
    );

    // ===== Node name functions =====
    reg_ns_stream!(
        crate::consts::FNS,
        "node-name",
        1,
        qnames::node_name_stream::<N>,
        vec![ParamTypeSpec::node(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(crate::consts::FNS, "name", 0, qnames::name_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "name",
        1,
        qnames::name_stream::<N>,
        vec![ParamTypeSpec::node(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(crate::consts::FNS, "local-name", 0, qnames::local_name_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "local-name",
        1,
        qnames::local_name_stream::<N>,
        vec![ParamTypeSpec::node(Occurrence::ZeroOrOne)]
    );

    // ===== QName / Namespace functions =====
    reg_ns_stream!(
        crate::consts::FNS,
        "QName",
        2,
        qnames::qname_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "resolve-QName",
        2,
        qnames::resolve_qname_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::element(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "namespace-uri-from-QName",
        1,
        qnames::namespace_uri_from_qname_stream::<N>,
        vec![ParamTypeSpec::qname(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "local-name-from-QName",
        1,
        qnames::local_name_from_qname_stream::<N>,
        vec![ParamTypeSpec::qname(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "prefix-from-QName",
        1,
        qnames::prefix_from_qname_stream::<N>,
        vec![ParamTypeSpec::qname(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "namespace-uri-for-prefix",
        2,
        qnames::namespace_uri_for_prefix_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::element(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "in-scope-prefixes",
        1,
        qnames::in_scope_prefixes_stream::<N>,
        vec![ParamTypeSpec::element(Occurrence::ExactlyOne)]
    );
    reg_ns_stream!(crate::consts::FNS, "namespace-uri", 0, qnames::namespace_uri_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "namespace-uri",
        1,
        qnames::namespace_uri_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );

    // ===== Numeric family =====
    reg_ns_stream!(
        crate::consts::FNS,
        "abs",
        1,
        numeric::abs_stream::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "floor",
        1,
        numeric::floor_stream::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "ceiling",
        1,
        numeric::ceiling_stream::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "round",
        1,
        numeric::round_stream::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "round",
        2,
        numeric::round_stream::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne), ParamTypeSpec::integer(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "round-half-to-even",
        1,
        numeric::round_half_to_even_stream::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "round-half-to-even",
        2,
        numeric::round_half_to_even_stream::<N>,
        vec![ParamTypeSpec::numeric(Occurrence::ZeroOrOne), ParamTypeSpec::integer(Occurrence::ZeroOrOne),]
    );
    // Stream-based sum() with optional zero value
    reg_ns_stream!(
        crate::consts::FNS,
        "sum",
        1,
        numeric::sum_stream::<N>,
        vec![ParamTypeSpec::numeric_or_duration(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "sum",
        2,
        numeric::sum_stream::<N>,
        vec![
            ParamTypeSpec::numeric_or_duration(Occurrence::ZeroOrMore),
            ParamTypeSpec::numeric_or_duration(Occurrence::ZeroOrOne),
        ]
    );
    // Stream-based avg()
    reg_ns_stream!(
        crate::consts::FNS,
        "avg",
        1,
        numeric::avg_stream::<N>,
        vec![ParamTypeSpec::numeric_or_duration(Occurrence::ZeroOrMore)]
    );

    // ===== Sequence family (using stream-based implementations) =====
    reg_ns_stream!(
        crate::consts::FNS,
        "empty",
        1,
        sequences::empty_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "exists",
        1,
        sequences::exists_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    // count() uses stream-based implementation for zero-copy performance
    reg_ns_stream!(
        crate::consts::FNS,
        "count",
        1,
        sequences::count_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "exactly-one",
        1,
        sequences::exactly_one_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "one-or-more",
        1,
        sequences::one_or_more_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "zero-or-one",
        1,
        sequences::zero_or_one_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "reverse",
        1,
        sequences::reverse_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );
    // Stream version for arity 2 and 3
    reg_ns_stream!(
        crate::consts::FNS,
        "subsequence",
        2,
        sequences::subsequence_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore), ParamTypeSpec::double(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "subsequence",
        3,
        sequences::subsequence_stream::<N>,
        vec![
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
            ParamTypeSpec::double(Occurrence::ExactlyOne),
            ParamTypeSpec::double(Occurrence::ZeroOrOne),
        ]
    );
    // Stream version of distinct-values
    reg_ns_stream!(
        crate::consts::FNS,
        "distinct-values",
        1,
        sequences::distinct_values_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "distinct-values",
        2,
        sequences::distinct_values_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    // Stream version of index-of
    reg_ns_stream!(
        crate::consts::FNS,
        "index-of",
        2,
        sequences::index_of_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore), ParamTypeSpec::any_atomic(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "index-of",
        3,
        sequences::index_of_stream::<N>,
        vec![
            ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore),
            ParamTypeSpec::any_atomic(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "insert-before",
        3,
        sequences::insert_before_stream::<N>,
        vec![
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
            ParamTypeSpec::integer(Occurrence::ExactlyOne),
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "remove",
        2,
        sequences::remove_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore), ParamTypeSpec::integer(Occurrence::ExactlyOne),]
    );
    // Stream-based min() with optional collation
    reg_ns_stream!(
        crate::consts::FNS,
        "min",
        1,
        numeric::min_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "min",
        2,
        numeric::min_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    // Stream-based max() with optional collation
    reg_ns_stream!(
        crate::consts::FNS,
        "max",
        1,
        numeric::max_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "max",
        2,
        numeric::max_stream::<N>,
        vec![ParamTypeSpec::any_atomic(Occurrence::ZeroOrMore), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );

    // ===== Collation-related functions =====
    reg_ns_stream!(
        crate::consts::FNS,
        "compare",
        2,
        collations::compare_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "compare",
        3,
        collations::compare_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "codepoint-equal",
        2,
        collations::codepoint_equal_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "deep-equal",
        2,
        collations::deep_equal_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore), ParamTypeSpec::any_item(Occurrence::ZeroOrMore),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "deep-equal",
        3,
        collations::deep_equal_stream::<N>,
        vec![
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );

    // ===== Regex family =====
    reg_ns_stream!(
        crate::consts::FNS,
        "matches",
        2,
        regex::matches_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "matches",
        3,
        regex::matches_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "replace",
        3,
        regex::replace_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "replace",
        4,
        regex::replace_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "tokenize",
        2,
        regex::tokenize_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "tokenize",
        3,
        regex::tokenize_stream::<N>,
        vec![
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ZeroOrOne),
        ]
    );

    // ===== Diagnostics =====
    reg_ns_stream!(crate::consts::FNS, "error", 0, diagnostics::error_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "error",
        1,
        diagnostics::error_stream::<N>,
        vec![ParamTypeSpec::qname(Occurrence::ExactlyOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "error",
        2,
        diagnostics::error_stream::<N>,
        vec![ParamTypeSpec::qname(Occurrence::ExactlyOne), ParamTypeSpec::string(Occurrence::ExactlyOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "error",
        3,
        diagnostics::error_stream::<N>,
        vec![
            ParamTypeSpec::qname(Occurrence::ExactlyOne),
            ParamTypeSpec::string(Occurrence::ExactlyOne),
            ParamTypeSpec::any_item(Occurrence::ZeroOrMore),
        ]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "trace",
        2,
        diagnostics::trace_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore), ParamTypeSpec::string(Occurrence::ExactlyOne),]
    );

    // ===== Environment / Document / URI helpers =====
    reg_ns_stream!(crate::consts::FNS, "default-collation", 0, environment::default_collation_stream::<N>, vec![]);
    reg_ns_stream!(crate::consts::FNS, "static-base-uri", 0, environment::static_base_uri_stream::<N>, vec![]);
    reg_ns_stream!(crate::consts::FNS, "root", 0, environment::root_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "root",
        1,
        environment::root_stream::<N>,
        vec![ParamTypeSpec::node(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(crate::consts::FNS, "base-uri", 0, environment::base_uri_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "base-uri",
        1,
        environment::base_uri_stream::<N>,
        vec![ParamTypeSpec::node(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(crate::consts::FNS, "document-uri", 0, environment::document_uri_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "document-uri",
        1,
        environment::document_uri_stream::<N>,
        vec![ParamTypeSpec::node(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "lang",
        1,
        environment::lang_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "lang",
        2,
        environment::lang_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::node(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "encode-for-uri",
        1,
        environment::encode_for_uri_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "nilled",
        1,
        environment::nilled_stream::<N>,
        vec![ParamTypeSpec::node(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "iri-to-uri",
        1,
        environment::iri_to_uri_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "escape-html-uri",
        1,
        environment::escape_html_uri_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "resolve-uri",
        1,
        environment::resolve_uri_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "resolve-uri",
        2,
        environment::resolve_uri_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "normalize-unicode",
        1,
        environment::normalize_unicode_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "normalize-unicode",
        2,
        environment::normalize_unicode_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne), ParamTypeSpec::string(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "doc-available",
        1,
        environment::doc_available_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "doc",
        1,
        environment::doc_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(crate::consts::FNS, "collection", 0, environment::collection_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "collection",
        1,
        environment::collection_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );

    // ===== ID / IDREF helpers =====
    reg_ns_stream!(
        crate::consts::FNS,
        "id",
        1,
        ids::id_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "id",
        2,
        ids::id_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrMore), ParamTypeSpec::node(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "element-with-id",
        1,
        ids::element_with_id_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "element-with-id",
        2,
        ids::element_with_id_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrMore), ParamTypeSpec::node(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "idref",
        1,
        ids::idref_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrMore)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "idref",
        2,
        ids::idref_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrMore), ParamTypeSpec::node(Occurrence::ZeroOrOne),]
    );

    // ===== Regex replacements already handled =====
    reg_ns_stream!(
        crate::consts::FNS,
        "unordered",
        1,
        sequences::unordered_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrMore)]
    );

    // ===== Misc constructors =====
    reg_ns_stream!(
        crate::consts::FNS,
        "integer",
        1,
        constructors::integer_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );

    // ===== Date/Time family =====
    reg_ns_stream!(
        crate::consts::FNS,
        "dateTime",
        2,
        datetime::date_time_stream::<N>,
        vec![ParamTypeSpec::date(Occurrence::ZeroOrOne), ParamTypeSpec::time(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "adjust-date-to-timezone",
        1,
        datetime::adjust_date_to_timezone_stream::<N>,
        vec![ParamTypeSpec::date(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "adjust-date-to-timezone",
        2,
        datetime::adjust_date_to_timezone_stream::<N>,
        vec![ParamTypeSpec::date(Occurrence::ZeroOrOne), ParamTypeSpec::day_time_duration(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "adjust-time-to-timezone",
        1,
        datetime::adjust_time_to_timezone_stream::<N>,
        vec![ParamTypeSpec::time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "adjust-time-to-timezone",
        2,
        datetime::adjust_time_to_timezone_stream::<N>,
        vec![ParamTypeSpec::time(Occurrence::ZeroOrOne), ParamTypeSpec::day_time_duration(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "adjust-dateTime-to-timezone",
        1,
        datetime::adjust_datetime_to_timezone_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "adjust-dateTime-to-timezone",
        2,
        datetime::adjust_datetime_to_timezone_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne), ParamTypeSpec::day_time_duration(Occurrence::ZeroOrOne),]
    );
    reg_ns_stream!(crate::consts::FNS, "current-dateTime", 0, datetime::current_datetime_stream::<N>, vec![]);
    reg_ns_stream!(crate::consts::FNS, "current-date", 0, datetime::current_date_stream::<N>, vec![]);
    reg_ns_stream!(crate::consts::FNS, "current-time", 0, datetime::current_time_stream::<N>, vec![]);
    reg_ns_stream!(crate::consts::FNS, "implicit-timezone", 0, datetime::implicit_timezone_stream::<N>, vec![]);
    reg_ns_stream!(
        crate::consts::FNS,
        "year-from-dateTime",
        1,
        datetime::year_from_datetime_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "hours-from-dateTime",
        1,
        datetime::hours_from_datetime_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "minutes-from-dateTime",
        1,
        datetime::minutes_from_datetime_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "seconds-from-dateTime",
        1,
        datetime::seconds_from_datetime_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "month-from-dateTime",
        1,
        datetime::month_from_datetime_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "day-from-dateTime",
        1,
        datetime::day_from_datetime_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "hours-from-time",
        1,
        datetime::hours_from_time_stream::<N>,
        vec![ParamTypeSpec::time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "minutes-from-time",
        1,
        datetime::minutes_from_time_stream::<N>,
        vec![ParamTypeSpec::time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "seconds-from-time",
        1,
        datetime::seconds_from_time_stream::<N>,
        vec![ParamTypeSpec::time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "timezone-from-dateTime",
        1,
        datetime::timezone_from_datetime_stream::<N>,
        vec![ParamTypeSpec::date_time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "timezone-from-date",
        1,
        datetime::timezone_from_date_stream::<N>,
        vec![ParamTypeSpec::date(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "timezone-from-time",
        1,
        datetime::timezone_from_time_stream::<N>,
        vec![ParamTypeSpec::time(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "year-from-date",
        1,
        datetime::year_from_date_stream::<N>,
        vec![ParamTypeSpec::date(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "month-from-date",
        1,
        datetime::month_from_date_stream::<N>,
        vec![ParamTypeSpec::date(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "day-from-date",
        1,
        datetime::day_from_date_stream::<N>,
        vec![ParamTypeSpec::date(Occurrence::ZeroOrOne)]
    );

    // ===== Duration component accessors =====
    reg_ns_stream!(
        crate::consts::FNS,
        "years-from-duration",
        1,
        durations::years_from_duration_stream::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "months-from-duration",
        1,
        durations::months_from_duration_stream::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "days-from-duration",
        1,
        durations::days_from_duration_stream::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "hours-from-duration",
        1,
        durations::hours_from_duration_stream::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "minutes-from-duration",
        1,
        durations::minutes_from_duration_stream::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::FNS,
        "seconds-from-duration",
        1,
        durations::seconds_from_duration_stream::<N>,
        vec![ParamTypeSpec::duration(Occurrence::ZeroOrOne)]
    );

    // ===== XML Schema constructors =====
    reg_ns_stream!(
        crate::consts::XS,
        "string",
        1,
        constructors::xs_string_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "untypedAtomic",
        1,
        constructors::xs_untyped_atomic_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "boolean",
        1,
        constructors::xs_boolean_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "integer",
        1,
        constructors::xs_integer_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "decimal",
        1,
        constructors::xs_decimal_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "double",
        1,
        constructors::xs_double_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "float",
        1,
        constructors::xs_float_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "anyURI",
        1,
        constructors::xs_any_uri_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "QName",
        1,
        constructors::xs_qname_stream::<N>,
        vec![ParamTypeSpec::string(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "base64Binary",
        1,
        constructors::xs_base64_binary_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "hexBinary",
        1,
        constructors::xs_hex_binary_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "dateTime",
        1,
        constructors::xs_datetime_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "date",
        1,
        constructors::xs_date_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "time",
        1,
        constructors::xs_time_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "duration",
        1,
        constructors::xs_duration_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "dayTimeDuration",
        1,
        constructors::xs_day_time_duration_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "yearMonthDuration",
        1,
        constructors::xs_year_month_duration_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "gYear",
        1,
        constructors::xs_g_year_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "gYearMonth",
        1,
        constructors::xs_g_year_month_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "gMonth",
        1,
        constructors::xs_g_month_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "gMonthDay",
        1,
        constructors::xs_g_month_day_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "gDay",
        1,
        constructors::xs_g_day_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "long",
        1,
        constructors::xs_long_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "int",
        1,
        constructors::xs_int_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "short",
        1,
        constructors::xs_short_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "byte",
        1,
        constructors::xs_byte_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "unsignedLong",
        1,
        constructors::xs_unsigned_long_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "unsignedInt",
        1,
        constructors::xs_unsigned_int_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "unsignedShort",
        1,
        constructors::xs_unsigned_short_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "unsignedByte",
        1,
        constructors::xs_unsigned_byte_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "nonPositiveInteger",
        1,
        constructors::xs_non_positive_integer_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "negativeInteger",
        1,
        constructors::xs_negative_integer_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "nonNegativeInteger",
        1,
        constructors::xs_non_negative_integer_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "positiveInteger",
        1,
        constructors::xs_positive_integer_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "normalizedString",
        1,
        constructors::xs_normalized_string_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "token",
        1,
        constructors::xs_token_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "language",
        1,
        constructors::xs_language_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "Name",
        1,
        constructors::xs_name_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "NCName",
        1,
        constructors::xs_ncname_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "NMTOKEN",
        1,
        constructors::xs_nmtoken_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "ID",
        1,
        constructors::xs_id_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "IDREF",
        1,
        constructors::xs_idref_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "ENTITY",
        1,
        constructors::xs_entity_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
    reg_ns_stream!(
        crate::consts::XS,
        "NOTATION",
        1,
        constructors::xs_notation_stream::<N>,
        vec![ParamTypeSpec::any_item(Occurrence::ZeroOrOne)]
    );
}

pub fn default_function_registry<N: 'static + crate::model::XdmNode + Clone>() -> Rc<FunctionImplementations<N>> {
    let mut reg: FunctionImplementations<N> = FunctionImplementations::new();
    ensure_default_signatures();
    register_default_functions(Some(&mut reg), None);
    Rc::new(reg)
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
