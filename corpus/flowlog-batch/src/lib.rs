#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::stable_sort_primitive,
    clippy::too_many_lines,
    clippy::unused_unit,
    non_snake_case
)]

//! Executable counterparts of `FlowLog`'s batch fixture corpus.

use std::error::Error;
use std::path::Path;

use proc_macro2::TokenStream;

macro_rules! fixture_program {
    ($($program:tt)*) => {
        miniflow::miniflow! {
            #![flowlog_batch]
            $($program)*
        }

        pub fn canonical_tokens() -> ::proc_macro2::TokenStream {
            ::quote::quote! {
                #![flowlog_batch]
                $($program)*
            }
        }
    };
}

pub(crate) use fixture_program;

macro_rules! fixture_io {
    (
        $name:ident;
        inputs { $($input:ident => $input_file:literal),* $(,)? }
        outputs { $($output:ident => $output_file:literal),+ $(,)? }
    ) => {
        pub fn run(
            fixture_dir: &::std::path::Path,
            output_dir: &::std::path::Path,
        ) -> ::std::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {
            let _ = fixture_dir;
            let mut program = $name {
                $($input: crate::common::read(fixture_dir, $input_file)?,)*
                ..$name::default()
            };
            program.run();
            $(
                crate::common::write(output_dir, $output_file, program.$output)?;
            )+
            Ok(())
        }
    };
}

pub(crate) use fixture_io;

macro_rules! binary_i32_filter_fixture {
    ($name:ident, $value:ident, $condition:expr) => {
        crate::fixture_program! {
            pub struct $name;
            relation data(i32, i32);
            relation out(i32, i32);

            out(id, $value) <-- data(id, $value), if $condition;
        }

        pub fn run(
            fixture_dir: &::std::path::Path,
            output_dir: &::std::path::Path,
        ) -> ::std::result::Result<(), ::std::boxed::Box<dyn ::std::error::Error>> {
            let mut program = $name {
                data: crate::common::read_i32_2(fixture_dir, "Data.csv")?,
                ..$name::default()
            };
            program.run();
            crate::common::write_i32_2(output_dir, "Out.csv", program.out)
        }
    };
}

pub(crate) use binary_i32_filter_fixture;

mod agg_avg;
mod agg_chained;
mod agg_count;
mod agg_count_string;
mod agg_max;
mod agg_min;
mod agg_negative;
mod agg_over_expr;
mod agg_sum;
mod arith_cat;
mod arith_chained;
mod arith_const_head;
mod arith_divide;
mod arith_float;
mod arith_minus;
mod arith_modulo;
mod arith_plus;
mod arith_times;
mod builtins;
mod cat_paren_intern;
mod common;
mod comp_basic;
mod comp_inheritance;
mod comp_multi_head_body;
mod comp_nested_typeparam;
mod comp_override;
mod comp_parametric;
mod compare_eq;
mod compare_expr;
mod compare_gt;
mod compare_gte;
mod compare_lt;
mod compare_lte;
mod compare_neq;
mod delimiter_tab;
mod equijoin_tuple;
mod join_const_body;
mod join_self;
mod join_three_way;
mod join_two_way;
mod join_wide;
mod keyword_relation_names;
mod match_builtin;
mod neg_antijoin;
mod neg_constant;
mod neg_filter;
mod neg_multiple;
mod neg_nullary;
mod neg_over_idb;
mod neg_stratified;
mod neg_wildcard;
mod output_all_types;
mod output_all_types_intern;
mod output_limit;
mod output_multi_worker;
mod output_order_by;
mod probe_edb_idb_inline;
mod probe_edb_idb_recursive_only;
mod probe_edb_idb_recursive_with_nonrec;
mod recursive_intermediate;
mod recursive_max;
mod recursive_min;
mod recursive_tc;
mod rule_empty_result;
mod rule_multi_head_multi_body;
mod rule_nullary;
mod rule_projection;
mod rule_union;
mod syntax_fact_only;
mod syntax_hybrid;
mod syntax_include;
mod syntax_inline_fact;
mod tuple_context_comp;
mod tuple_eq_filter;
mod tuple_hetero;
mod tuple_nested;
mod tuple_order_by;
mod tuple_pair;
mod tuple_placeholder;
mod tuple_str_intern;
mod tuple_wide;
mod type_bool;
mod type_float;
mod type_float_crossing;
mod type_int;
mod type_int_crossing;
mod type_string;
mod type_uint;
mod type_uint_crossing;
mod type_wide_crossing;
mod udf_arithmetic;
mod udf_comparison;
mod udf_head;
mod udf_in_aggregation;
mod udf_nested;
mod udf_predicate;
mod udf_string_intern;
mod udf_types;

/// Return the single-source program tokens for a strict fixture.
#[must_use]
pub fn canonical_tokens(fixture: &str) -> Option<TokenStream> {
    match fixture {
        "agg_avg" => Some(agg_avg::canonical_tokens()),
        "agg_chained" => Some(agg_chained::canonical_tokens()),
        "agg_count" => Some(agg_count::canonical_tokens()),
        "agg_count_string" => Some(agg_count_string::canonical_tokens()),
        "agg_max" => Some(agg_max::canonical_tokens()),
        "agg_min" => Some(agg_min::canonical_tokens()),
        "agg_negative" => Some(agg_negative::canonical_tokens()),
        "agg_over_expr" => Some(agg_over_expr::canonical_tokens()),
        "agg_sum" => Some(agg_sum::canonical_tokens()),
        "arith_cat" => Some(arith_cat::canonical_tokens()),
        "arith_chained" => Some(arith_chained::canonical_tokens()),
        "arith_const_head" => Some(arith_const_head::canonical_tokens()),
        "arith_divide" => Some(arith_divide::canonical_tokens()),
        "arith_float" => Some(arith_float::canonical_tokens()),
        "arith_minus" => Some(arith_minus::canonical_tokens()),
        "arith_modulo" => Some(arith_modulo::canonical_tokens()),
        "arith_plus" => Some(arith_plus::canonical_tokens()),
        "arith_times" => Some(arith_times::canonical_tokens()),
        "builtins" => Some(builtins::canonical_tokens()),
        "cat_paren_intern" => Some(cat_paren_intern::canonical_tokens()),
        "comp_basic" => Some(comp_basic::canonical_tokens()),
        "comp_inheritance" => Some(comp_inheritance::canonical_tokens()),
        "comp_multi_head_body" => Some(comp_multi_head_body::canonical_tokens()),
        "comp_nested_typeparam" => Some(comp_nested_typeparam::canonical_tokens()),
        "comp_override" => Some(comp_override::canonical_tokens()),
        "comp_parametric" => Some(comp_parametric::canonical_tokens()),
        "compare_eq" => Some(compare_eq::canonical_tokens()),
        "compare_expr" => Some(compare_expr::canonical_tokens()),
        "compare_gt" => Some(compare_gt::canonical_tokens()),
        "compare_gte" => Some(compare_gte::canonical_tokens()),
        "compare_lt" => Some(compare_lt::canonical_tokens()),
        "compare_lte" => Some(compare_lte::canonical_tokens()),
        "compare_neq" => Some(compare_neq::canonical_tokens()),
        "delimiter_tab" => Some(delimiter_tab::canonical_tokens()),
        "equijoin_tuple" => Some(equijoin_tuple::canonical_tokens()),
        "join_const_body" => Some(join_const_body::canonical_tokens()),
        "join_self" => Some(join_self::canonical_tokens()),
        "join_three_way" => Some(join_three_way::canonical_tokens()),
        "join_two_way" => Some(join_two_way::canonical_tokens()),
        "join_wide" => Some(join_wide::canonical_tokens()),
        "keyword_relation_names" => Some(keyword_relation_names::canonical_tokens()),
        "match_builtin" => Some(match_builtin::canonical_tokens()),
        "neg_antijoin" => Some(neg_antijoin::canonical_tokens()),
        "neg_constant" => Some(neg_constant::canonical_tokens()),
        "neg_filter" => Some(neg_filter::canonical_tokens()),
        "neg_multiple" => Some(neg_multiple::canonical_tokens()),
        "neg_nullary" => Some(neg_nullary::canonical_tokens()),
        "neg_over_idb" => Some(neg_over_idb::canonical_tokens()),
        "neg_stratified" => Some(neg_stratified::canonical_tokens()),
        "neg_wildcard" => Some(neg_wildcard::canonical_tokens()),
        "output_all_types" => Some(output_all_types::canonical_tokens()),
        "output_all_types_intern" => Some(output_all_types_intern::canonical_tokens()),
        "output_limit" => Some(output_limit::canonical_tokens()),
        "output_multi_worker" => Some(output_multi_worker::canonical_tokens()),
        "output_order_by" => Some(output_order_by::canonical_tokens()),
        "probe_edb_idb_inline" => Some(probe_edb_idb_inline::canonical_tokens()),
        "probe_edb_idb_recursive_only" => Some(probe_edb_idb_recursive_only::canonical_tokens()),
        "probe_edb_idb_recursive_with_nonrec" => {
            Some(probe_edb_idb_recursive_with_nonrec::canonical_tokens())
        }
        "recursive_intermediate" => Some(recursive_intermediate::canonical_tokens()),
        "recursive_max" => Some(recursive_max::canonical_tokens()),
        "recursive_min" => Some(recursive_min::canonical_tokens()),
        "recursive_tc" => Some(recursive_tc::canonical_tokens()),
        "rule_empty_result" => Some(rule_empty_result::canonical_tokens()),
        "rule_multi_head_multi_body" => Some(rule_multi_head_multi_body::canonical_tokens()),
        "rule_nullary" => Some(rule_nullary::canonical_tokens()),
        "rule_projection" => Some(rule_projection::canonical_tokens()),
        "rule_union" => Some(rule_union::canonical_tokens()),
        "syntax_fact_only" => Some(syntax_fact_only::canonical_tokens()),
        "syntax_hybrid" => Some(syntax_hybrid::canonical_tokens()),
        "syntax_include" => Some(syntax_include::canonical_tokens()),
        "syntax_inline_fact" => Some(syntax_inline_fact::canonical_tokens()),
        "type_bool" => Some(type_bool::canonical_tokens()),
        "type_float" => Some(type_float::canonical_tokens()),
        "type_float_crossing" => Some(type_float_crossing::canonical_tokens()),
        "type_int" => Some(type_int::canonical_tokens()),
        "type_int_crossing" => Some(type_int_crossing::canonical_tokens()),
        "type_string" => Some(type_string::canonical_tokens()),
        "type_uint" => Some(type_uint::canonical_tokens()),
        "type_uint_crossing" => Some(type_uint_crossing::canonical_tokens()),
        "type_wide_crossing" => Some(type_wide_crossing::canonical_tokens()),
        "tuple_context_comp" => Some(tuple_context_comp::canonical_tokens()),
        "tuple_eq_filter" => Some(tuple_eq_filter::canonical_tokens()),
        "tuple_hetero" => Some(tuple_hetero::canonical_tokens()),
        "tuple_nested" => Some(tuple_nested::canonical_tokens()),
        "tuple_order_by" => Some(tuple_order_by::canonical_tokens()),
        "tuple_pair" => Some(tuple_pair::canonical_tokens()),
        "tuple_placeholder" => Some(tuple_placeholder::canonical_tokens()),
        "tuple_str_intern" => Some(tuple_str_intern::canonical_tokens()),
        "tuple_wide" => Some(tuple_wide::canonical_tokens()),
        "udf_arithmetic" => Some(udf_arithmetic::canonical_tokens()),
        "udf_comparison" => Some(udf_comparison::canonical_tokens()),
        "udf_head" => Some(udf_head::canonical_tokens()),
        "udf_in_aggregation" => Some(udf_in_aggregation::canonical_tokens()),
        "udf_nested" => Some(udf_nested::canonical_tokens()),
        "udf_predicate" => Some(udf_predicate::canonical_tokens()),
        "udf_string_intern" => Some(udf_string_intern::canonical_tokens()),
        "udf_types" => Some(udf_types::canonical_tokens()),
        _ => None,
    }
}

/// Execute one translated fixture and write FlowLog-shaped output files.
///
/// # Errors
///
/// Returns an error for unknown fixtures or output I/O failures.
pub fn run(fixture: &str, fixture_dir: &Path, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    match fixture {
        "agg_avg" => agg_avg::run(fixture_dir, output_dir),
        "agg_chained" => agg_chained::run(fixture_dir, output_dir),
        "agg_count" => agg_count::run(fixture_dir, output_dir),
        "agg_count_string" => agg_count_string::run(fixture_dir, output_dir),
        "agg_max" => agg_max::run(fixture_dir, output_dir),
        "agg_min" => agg_min::run(fixture_dir, output_dir),
        "agg_negative" => agg_negative::run(fixture_dir, output_dir),
        "agg_over_expr" => agg_over_expr::run(fixture_dir, output_dir),
        "agg_sum" => agg_sum::run(fixture_dir, output_dir),
        "arith_cat" => arith_cat::run(fixture_dir, output_dir),
        "arith_chained" => arith_chained::run(fixture_dir, output_dir),
        "arith_const_head" => arith_const_head::run(fixture_dir, output_dir),
        "arith_divide" => arith_divide::run(fixture_dir, output_dir),
        "arith_float" => arith_float::run(fixture_dir, output_dir),
        "arith_minus" => arith_minus::run(fixture_dir, output_dir),
        "arith_modulo" => arith_modulo::run(fixture_dir, output_dir),
        "arith_plus" => arith_plus::run(fixture_dir, output_dir),
        "arith_times" => arith_times::run(fixture_dir, output_dir),
        "builtins" => builtins::run(fixture_dir, output_dir),
        "cat_paren_intern" => cat_paren_intern::run(fixture_dir, output_dir),
        "comp_basic" => comp_basic::run(fixture_dir, output_dir),
        "comp_inheritance" => comp_inheritance::run(fixture_dir, output_dir),
        "comp_multi_head_body" => comp_multi_head_body::run(fixture_dir, output_dir),
        "comp_nested_typeparam" => comp_nested_typeparam::run(fixture_dir, output_dir),
        "comp_override" => comp_override::run(fixture_dir, output_dir),
        "comp_parametric" => comp_parametric::run(fixture_dir, output_dir),
        "compare_eq" => compare_eq::run(fixture_dir, output_dir),
        "compare_expr" => compare_expr::run(fixture_dir, output_dir),
        "compare_gt" => compare_gt::run(fixture_dir, output_dir),
        "compare_gte" => compare_gte::run(fixture_dir, output_dir),
        "compare_lt" => compare_lt::run(fixture_dir, output_dir),
        "compare_lte" => compare_lte::run(fixture_dir, output_dir),
        "compare_neq" => compare_neq::run(fixture_dir, output_dir),
        "delimiter_tab" => delimiter_tab::run(fixture_dir, output_dir),
        "equijoin_tuple" => equijoin_tuple::run(fixture_dir, output_dir),
        "join_const_body" => join_const_body::run(fixture_dir, output_dir),
        "join_self" => join_self::run(fixture_dir, output_dir),
        "join_three_way" => join_three_way::run(fixture_dir, output_dir),
        "join_two_way" => join_two_way::run(fixture_dir, output_dir),
        "join_wide" => join_wide::run(fixture_dir, output_dir),
        "keyword_relation_names" => keyword_relation_names::run(fixture_dir, output_dir),
        "match_builtin" => match_builtin::run(fixture_dir, output_dir),
        "neg_antijoin" => neg_antijoin::run(fixture_dir, output_dir),
        "neg_constant" => neg_constant::run(fixture_dir, output_dir),
        "neg_filter" => neg_filter::run(fixture_dir, output_dir),
        "neg_multiple" => neg_multiple::run(fixture_dir, output_dir),
        "neg_nullary" => neg_nullary::run(fixture_dir, output_dir),
        "neg_over_idb" => neg_over_idb::run(fixture_dir, output_dir),
        "neg_stratified" => neg_stratified::run(fixture_dir, output_dir),
        "neg_wildcard" => neg_wildcard::run(fixture_dir, output_dir),
        "output_all_types" => output_all_types::run(fixture_dir, output_dir),
        "output_all_types_intern" => output_all_types_intern::run(fixture_dir, output_dir),
        "output_limit" => output_limit::run(fixture_dir, output_dir),
        "output_multi_worker" => output_multi_worker::run(fixture_dir, output_dir),
        "output_order_by" => output_order_by::run(fixture_dir, output_dir),
        "probe_edb_idb_inline" => probe_edb_idb_inline::run(fixture_dir, output_dir),
        "probe_edb_idb_recursive_only" => {
            probe_edb_idb_recursive_only::run(fixture_dir, output_dir)
        }
        "probe_edb_idb_recursive_with_nonrec" => {
            probe_edb_idb_recursive_with_nonrec::run(fixture_dir, output_dir)
        }
        "recursive_intermediate" => recursive_intermediate::run(fixture_dir, output_dir),
        "recursive_max" => recursive_max::run(fixture_dir, output_dir),
        "recursive_min" => recursive_min::run(fixture_dir, output_dir),
        "recursive_tc" => recursive_tc::run(fixture_dir, output_dir),
        "rule_empty_result" => rule_empty_result::run(fixture_dir, output_dir),
        "rule_multi_head_multi_body" => rule_multi_head_multi_body::run(fixture_dir, output_dir),
        "rule_nullary" => rule_nullary::run(fixture_dir, output_dir),
        "rule_projection" => rule_projection::run(fixture_dir, output_dir),
        "rule_union" => rule_union::run(fixture_dir, output_dir),
        "syntax_fact_only" => syntax_fact_only::run(fixture_dir, output_dir),
        "syntax_hybrid" => syntax_hybrid::run(fixture_dir, output_dir),
        "syntax_include" => syntax_include::run(fixture_dir, output_dir),
        "syntax_inline_fact" => syntax_inline_fact::run(fixture_dir, output_dir),
        "type_bool" => type_bool::run(fixture_dir, output_dir),
        "type_float" => type_float::run(fixture_dir, output_dir),
        "type_float_crossing" => type_float_crossing::run(fixture_dir, output_dir),
        "type_int" => type_int::run(fixture_dir, output_dir),
        "type_int_crossing" => type_int_crossing::run(fixture_dir, output_dir),
        "type_string" => type_string::run(fixture_dir, output_dir),
        "type_uint" => type_uint::run(fixture_dir, output_dir),
        "type_uint_crossing" => type_uint_crossing::run(fixture_dir, output_dir),
        "type_wide_crossing" => type_wide_crossing::run(fixture_dir, output_dir),
        "tuple_context_comp" => tuple_context_comp::run(fixture_dir, output_dir),
        "tuple_eq_filter" => tuple_eq_filter::run(fixture_dir, output_dir),
        "tuple_hetero" => tuple_hetero::run(fixture_dir, output_dir),
        "tuple_nested" => tuple_nested::run(fixture_dir, output_dir),
        "tuple_order_by" => tuple_order_by::run(fixture_dir, output_dir),
        "tuple_pair" => tuple_pair::run(fixture_dir, output_dir),
        "tuple_placeholder" => tuple_placeholder::run(fixture_dir, output_dir),
        "tuple_str_intern" => tuple_str_intern::run(fixture_dir, output_dir),
        "tuple_wide" => tuple_wide::run(fixture_dir, output_dir),
        "udf_arithmetic" => udf_arithmetic::run(fixture_dir, output_dir),
        "udf_comparison" => udf_comparison::run(fixture_dir, output_dir),
        "udf_head" => udf_head::run(fixture_dir, output_dir),
        "udf_in_aggregation" => udf_in_aggregation::run(fixture_dir, output_dir),
        "udf_nested" => udf_nested::run(fixture_dir, output_dir),
        "udf_predicate" => udf_predicate::run(fixture_dir, output_dir),
        "udf_string_intern" => udf_string_intern::run(fixture_dir, output_dir),
        "udf_types" => udf_types::run(fixture_dir, output_dir),
        _ => Err(format!("no MiniFlow batch counterpart for `{fixture}`").into()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn completed_fixture_results_match() {
        super::syntax_fact_only::check();
    }
}
