use std::cell::RefCell;
use std::cmp::Ordering;
use std::path::PathBuf;
use std::rc::Rc;

use ncl_syntax::{ReadError, ReadErrorKind, Reader, Span};

use crate::environment::normalize_name;
use crate::evaluator::quoted_form_value;
use crate::package::{self, COMMON_LISP_PACKAGE, KEYWORD_PACKAGE};
use crate::value::ArrayElementType;
use crate::{Environment, Function, Rational, RuntimeError, Stream, Value};

mod arrays;
mod format;
mod hash_tables;
mod numeric;
pub(crate) mod random;
mod sequences;
mod streams;
mod types;

use numeric::*;
use random::*;
use sequences::*;
use streams::*;
use types::{
    ecase_error, etypecase_error, is_simple_vector_value, type_designator_name, vector_elements,
};

pub(crate) use types::{
    builtin_type_specializer_score, known_type_name, subtypep_value, the_check, typep_value,
};

use arrays::{
    adjustable_array_p, adjust_array, aref, array_dimension, array_dimensions,
    array_element_type, array_has_fill_pointer_p, array_in_bounds_p, array_rank,
    array_row_major_index, array_total_size, arrayp, bit, bit_not, bit_vector_p, fill_pointer,
    make_array, row_major_aref, sbit, simple_array_p, svref, vector, vector_pop, vector_push,
    vector_push_extend,
};

pub(crate) use format::format_control;
use format::format_value;

pub(crate) use hash_tables::hash_table_key_equal;
use hash_tables::{
    clrhash, gethash, hash_table_count, hash_table_iterator_next, hash_table_p,
    hash_table_rehash_size, hash_table_rehash_threshold, hash_table_size,
    hash_table_synchronized_p, hash_table_test_value, make_hash_table,
    make_hash_table_iterator, remhash,
};

pub fn install(environment: &Environment) {
    for (name, function) in [
        ("+", add as _),
        ("-", subtract as _),
        ("*", multiply as _),
        ("/", divide as _),
        ("exp", exponential as _),
        ("log", logarithm as _),
        ("sin", sine as _),
        ("cos", cosine as _),
        ("tan", tangent as _),
        ("atan", arctangent as _),
        ("asin", arcsine as _),
        ("acos", arccosine as _),
        ("sinh", hyperbolic_sine as _),
        ("cosh", hyperbolic_cosine as _),
        ("tanh", hyperbolic_tangent as _),
        ("asinh", hyperbolic_arcsine as _),
        ("acosh", hyperbolic_arccosine as _),
        ("atanh", hyperbolic_arctangent as _),
        ("expt", exponentiate as _),
        ("sqrt", square_root as _),
        ("isqrt", integer_square_root_value as _),
        ("signum", signum as _),
        ("phase", phase as _),
        ("cis", cis as _),
        ("float-sign", float_sign as _),
        ("float-radix", float_radix as _),
        ("float-digits", float_digits as _),
        ("float-precision", float_precision as _),
        ("decode-float", decode_float as _),
        ("scale-float", scale_float as _),
        ("integer-decode-float", integer_decode_float as _),
        ("complex", complex as _),
        ("realpart", realpart as _),
        ("imagpart", imagpart as _),
        ("conjugate", conjugate as _),
        ("float", float_value as _),
        ("rational", rational as _),
        ("rationalize", rationalize as _),
        ("random", random as _),
        ("make-random-state", make_random_state as _),
        ("random-state-p", random_state_p as _),
        ("=", numeric_equal as _),
        ("/=", numeric_not_equal as _),
        ("<", less_than as _),
        (">", greater_than as _),
        ("<=", less_equal as _),
        (">=", greater_equal as _),
        ("zerop", zerop as _),
        ("plusp", plusp as _),
        ("minusp", minusp as _),
        ("evenp", evenp as _),
        ("oddp", oddp as _),
        ("min", minimum as _),
        ("max", maximum as _),
        ("abs", absolute as _),
        ("1+", increment as _),
        ("1-", decrement as _),
        ("floor", floor as _),
        ("ceiling", ceiling as _),
        ("truncate", truncate as _),
        ("round", round as _),
        ("gcd", greatest_common_divisor as _),
        ("lcm", least_common_multiple as _),
        ("numerator", numerator as _),
        ("denominator", denominator as _),
        ("mod", modulo as _),
        ("rem", remainder as _),
        ("ash", arithmetic_shift as _),
        ("logand", logand as _),
        ("logior", logior as _),
        ("logxor", logxor as _),
        ("logeqv", logeqv as _),
        ("lognand", lognand as _),
        ("lognor", lognor as _),
        ("logandc1", logandc1 as _),
        ("logandc2", logandc2 as _),
        ("logorc1", logorc1 as _),
        ("logorc2", logorc2 as _),
        ("boole", boole as _),
        ("lognot", lognot as _),
        ("logtest", logtest as _),
        ("logbitp", logbitp as _),
        ("logcount", logcount as _),
        ("integer-length", integer_length as _),
        ("byte", byte as _),
        ("byte-size", byte_size as _),
        ("byte-position", byte_position as _),
        ("ldb", ldb as _),
        ("ldb-test", ldb_test as _),
        ("mask-field", mask_field as _),
        ("dpb", dpb as _),
        ("deposit-field", deposit_field as _),
        ("parse-integer", parse_integer as _),
        ("list", list as _),
        ("list*", list_star as _),
        ("make-list", make_list as _),
        ("values-list", values_list as _),
        ("list-length", list_length as _),
        ("nthcdr", nthcdr as _),
        ("acons", acons as _),
        ("pairlis", pairlis as _),
        ("cons", cons as _),
        ("car", car as _),
        ("cdr", cdr as _),
        ("caar", caar as _),
        ("cadr", cadr as _),
        ("cdar", cdar as _),
        ("cddr", cddr as _),
        ("caaar", caaar as _),
        ("caadr", caadr as _),
        ("cadar", cadar as _),
        ("caddr", caddr as _),
        ("cdaar", cdaar as _),
        ("cdadr", cdadr as _),
        ("cddar", cddar as _),
        ("cdddr", cdddr as _),
        ("caaaar", caaaar as _),
        ("caaadr", caaadr as _),
        ("caadar", caadar as _),
        ("caaddr", caaddr as _),
        ("cadaar", cadaar as _),
        ("cadadr", cadadr as _),
        ("caddar", caddar as _),
        ("cadddr", cadddr as _),
        ("cdaaar", cdaaar as _),
        ("cdaadr", cdaadr as _),
        ("cdadar", cdadar as _),
        ("cdaddr", cdaddr as _),
        ("cddaar", cddaar as _),
        ("cddadr", cddadr as _),
        ("cdddar", cdddar as _),
        ("cddddr", cddddr as _),
        ("first", first as _),
        ("rest", rest as _),
        ("append", append as _),
        ("nconc", nconc as _),
        ("revappend", revappend as _),
        ("nreconc", nreconc as _),
        ("length", length as _),
        ("reverse", reverse as _),
        ("nreverse", nreverse as _),
        ("last", last as _),
        ("butlast", butlast as _),
        ("nbutlast", nbutlast as _),
        ("copy-list", copy_list as _),
        ("copy-alist", copy_alist as _),
        ("copy-tree", copy_tree as _),
        ("vector", vector as _),
        ("make-array", make_array as _),
        ("adjust-array", adjust_array as _),
        ("fill-pointer", fill_pointer as _),
        ("vector-push", vector_push as _),
        ("vector-push-extend", vector_push_extend as _),
        ("vector-pop", vector_pop as _),
        ("adjustable-array-p", adjustable_array_p as _),
        ("array-has-fill-pointer-p", array_has_fill_pointer_p as _),
        ("make-sequence", make_sequence as _),
        ("aref", aref as _),
        ("svref", svref as _),
        ("bit", bit as _),
        ("sbit", sbit as _),
        ("bit-not", bit_not as _),
        ("bit-vector-p", bit_vector_p as _),
        ("row-major-aref", row_major_aref as _),
        ("array-row-major-index", array_row_major_index as _),
        ("array-in-bounds-p", array_in_bounds_p as _),
        ("array-element-type", array_element_type as _),
        ("simple-array-p", simple_array_p as _),
        ("arrayp", arrayp as _),
        ("array-rank", array_rank as _),
        ("array-dimensions", array_dimensions as _),
        ("array-dimension", array_dimension as _),
        ("array-total-size", array_total_size as _),
        ("make-hash-table", make_hash_table as _),
        ("gethash", gethash as _),
        ("remhash", remhash as _),
        ("clrhash", clrhash as _),
        ("hash-table-p", hash_table_p as _),
        ("hash-table-count", hash_table_count as _),
        ("hash-table-test", hash_table_test_value as _),
        ("hash-table-size", hash_table_size as _),
        ("hash-table-rehash-size", hash_table_rehash_size as _),
        (
            "hash-table-rehash-threshold",
            hash_table_rehash_threshold as _,
        ),
        ("hash-table-synchronized-p", hash_table_synchronized_p as _),
        (
            "__NCL-MAKE-HASH-TABLE-ITERATOR",
            make_hash_table_iterator as _,
        ),
        (
            "__NCL-HASH-TABLE-ITERATOR-NEXT",
            hash_table_iterator_next as _,
        ),
        ("nth", nth as _),
        ("elt", elt as _),
        ("subseq", subseq as _),
        ("fill", fill as _),
        ("replace", replace as _),
        ("copy-seq", copy_seq as _),
        ("concatenate", concatenate as _),
        ("coerce", coerce as _),
        ("string", string_value as _),
        ("make-string", make_string as _),
        ("character", character_value as _),
        ("char", character as _),
        ("schar", simple_character as _),
        ("char-code", char_code as _),
        ("char-int", char_int as _),
        ("code-char", code_char as _),
        ("int-char", int_char as _),
        ("char=", character_equal as _),
        ("char/=", character_not_equal as _),
        ("char-equal", character_case_equal as _),
        ("char-not-equal", character_case_not_equal as _),
        ("char<", character_less_than as _),
        ("char>", character_greater_than as _),
        ("char<=", character_less_equal as _),
        ("char>=", character_greater_equal as _),
        ("char-lessp", character_case_less_than as _),
        ("char-greaterp", character_case_greater_than as _),
        ("char-not-lessp", character_case_greater_equal as _),
        ("char-not-greaterp", character_case_less_equal as _),
        ("char-upcase", character_upcase as _),
        ("char-downcase", character_downcase as _),
        ("alpha-char-p", alpha_character_p as _),
        ("alphanumericp", alphanumeric_p as _),
        ("digit-char", digit_character as _),
        ("digit-char-p", digit_character_p as _),
        ("graphic-char-p", graphic_character_p as _),
        ("standard-char-p", standard_character_p as _),
        ("upper-case-p", upper_case_p as _),
        ("lower-case-p", lower_case_p as _),
        ("both-case-p", both_case_p as _),
        ("char-name", character_name as _),
        ("name-char", name_character as _),
        ("string=", string_equal as _),
        ("string-equal", string_case_equal as _),
        ("string<", string_less_than as _),
        ("string>", string_greater_than as _),
        ("string<=", string_less_equal as _),
        ("string>=", string_greater_equal as _),
        ("string-upcase", string_upcase as _),
        ("string-downcase", string_downcase as _),
        ("string-capitalize", string_capitalize as _),
        ("nstring-upcase", nstring_upcase as _),
        ("nstring-downcase", nstring_downcase as _),
        ("nstring-capitalize", nstring_capitalize as _),
        ("string-trim", string_trim as _),
        ("string-left-trim", string_left_trim as _),
        ("string-right-trim", string_right_trim as _),
        ("getf", getf as _),
        ("get-properties", get_properties as _),
        ("null", null as _),
        ("not", null as _),
        ("endp", endp as _),
        ("atom", atom as _),
        ("consp", consp as _),
        ("listp", listp as _),
        ("numberp", numberp as _),
        ("complexp", complexp as _),
        ("integerp", integerp as _),
        ("floatp", floatp as _),
        ("rationalp", rationalp as _),
        ("realp", realp as _),
        ("stringp", stringp as _),
        ("simple-string-p", simple_string_p as _),
        ("characterp", characterp as _),
        ("symbolp", symbolp as _),
        ("packagep", packagep as _),
        ("keywordp", keywordp as _),
        ("symbol-name", symbol_name_value as _),
        ("symbol-package", symbol_package_value as _),
        ("vectorp", vectorp as _),
        ("simple-vector-p", simple_vector_p as _),
        ("functionp", functionp as _),
        ("eq", eq as _),
        ("eql", eql as _),
        ("equal", equal as _),
        ("equalp", equalp as _),
        ("identity", identity as _),
        ("type-of", type_of as _),
        ("typep", typep as _),
        (
            "simple-condition-format-control",
            simple_condition_format_control as _,
        ),
        (
            "simple-condition-format-arguments",
            simple_condition_format_arguments as _,
        ),
        ("type-error-datum", type_error_datum as _),
        ("type-error-expected-type", type_error_expected_type as _),
        ("__NCL_THE_CHECK", the_check as _),
        ("__NCL_ECASE_ERROR", ecase_error as _),
        ("__NCL_ETYPECASE_ERROR", etypecase_error as _),
        ("print", print_value as _),
        ("princ", princ as _),
        ("prin1", prin1 as _),
        ("write", write_value as _),
        ("format", format_value as _),
        ("write-to-string", write_to_string as _),
        ("read-from-string", read_from_string as _),
        ("make-string-input-stream", make_string_input_stream as _),
        (
            "__NCL-STRING-INPUT-STREAM-POSITION",
            string_input_stream_position as _,
        ),
        ("make-string-output-stream", make_string_output_stream as _),
        ("make-two-way-stream", make_two_way_stream as _),
        (
            "two-way-stream-input-stream",
            two_way_stream_input_stream as _,
        ),
        (
            "two-way-stream-output-stream",
            two_way_stream_output_stream as _,
        ),
        ("make-broadcast-stream", make_broadcast_stream as _),
        ("broadcast-stream-streams", broadcast_stream_streams as _),
        (
            "concatenated-stream-streams",
            concatenated_stream_streams as _,
        ),
        ("make-concatenated-stream", make_concatenated_stream as _),
        ("make-echo-stream", make_echo_stream as _),
        ("echo-stream-input-stream", echo_stream_input_stream as _),
        ("echo-stream-output-stream", echo_stream_output_stream as _),
        ("open", open_file as _),
        ("file-position", file_position as _),
        ("file-length", file_length as _),
        ("probe-file", probe_file as _),
        ("delete-file", delete_file as _),
        ("rename-file", rename_file as _),
        ("file-write-date", file_write_date as _),
        ("truename", truename as _),
        ("get-output-stream-string", get_output_stream_string as _),
        ("stream-element-type", stream_element_type as _),
        ("write-char", write_char as _),
        ("write-string", write_string as _),
        ("terpri", terpri as _),
        ("fresh-line", fresh_line as _),
        ("clear-output", clear_output as _),
        ("finish-output", finish_output as _),
        ("force-output", force_output as _),
        ("write-line", write_line as _),
        ("close", close_stream as _),
        ("open-stream-p", open_stream_p as _),
        ("streamp", streamp as _),
        ("input-stream-p", input_stream_p as _),
        ("output-stream-p", output_stream_p as _),
    ] {
        let value = Value::builtin(name, function);
        let normalized = normalize_name(name);
        environment.define_function(normalized.clone(), value.clone());
        environment.define_function(format!("{COMMON_LISP_PACKAGE}::{normalized}"), value);
    }
    for name in [
        "FUNCALL",
        "APPLY",
        "EVAL",
        "READ",
        "READ-PRESERVING-WHITESPACE",
        "READ-CHAR",
        "PEEK-CHAR",
        "UNREAD-CHAR",
        "LISTEN",
        "CLEAR-INPUT",
        "READ-LINE",
        "READ-SEQUENCE",
        "ERROR",
        "SIGNAL",
        "WARN",
        "CERROR",
        "MAKE-CONDITION",
        "COMPILE",
        "LOAD",
        "MAP",
        "REDUCE",
        "MAP-INTO",
        "MAPHASH",
        "FIND",
        "POSITION",
        "COUNT",
        "SEARCH",
        "MISMATCH",
        "SORT",
        "STABLE-SORT",
        "MERGE",
        "EVERY",
        "SOME",
        "NOTANY",
        "NOTEVERY",
        "REMOVE",
        "REMOVE-IF",
        "REMOVE-IF-NOT",
        "DELETE",
        "DELETE-IF",
        "DELETE-IF-NOT",
        "REMOVE-DUPLICATES",
        "DELETE-DUPLICATES",
        "SUBSTITUTE",
        "SUBSTITUTE-IF",
        "SUBSTITUTE-IF-NOT",
        "NSUBSTITUTE",
        "NSUBSTITUTE-IF",
        "NSUBSTITUTE-IF-NOT",
        "UNION",
        "NUNION",
        "INTERSECTION",
        "NINTERSECTION",
        "SET-DIFFERENCE",
        "NSET-DIFFERENCE",
        "SET-EXCLUSIVE-OR",
        "NSET-EXCLUSIVE-OR",
        "SUBSETP",
        "SUBTYPEP",
        "MEMBER",
        "MEMBER-IF",
        "MEMBER-IF-NOT",
        "ADJOIN",
        "ASSOC",
        "ASSOC-IF",
        "ASSOC-IF-NOT",
        "RASSOC",
        "RASSOC-IF",
        "RASSOC-IF-NOT",
        "MAPCAR",
        "MAPC",
        "MAPL",
        "MAPLIST",
        "MAPCAN",
        "MAPCON",
        "MAKE-SYMBOL",
        "GENSYM",
        "INTERN",
        "FIND-SYMBOL",
        "FIND-ALL-SYMBOLS",
        "MAKE-PACKAGE",
        "DELETE-PACKAGE",
        "RENAME-PACKAGE",
        "FIND-PACKAGE",
        "PACKAGE-NAME",
        "PACKAGE-USE-LIST",
        "PACKAGE-NICKNAMES",
        "PACKAGE-SHADOWING-SYMBOLS",
        "PACKAGE-USED-BY-LIST",
        "DOCUMENTATION",
        "LIST-ALL-PACKAGES",
        "USE-PACKAGE",
        "UNUSE-PACKAGE",
        "EXPORT",
        "UNEXPORT",
        "IMPORT",
        "UNINTERN",
        "SHADOW",
        "SHADOWING-IMPORT",
        "BOUNDP",
        "CONSTANTP",
        "FBOUNDP",
        "MACRO-FUNCTION",
        "SPECIAL-OPERATOR-P",
        "COMPILED-FUNCTION-P",
        "FDEFINITION",
        "SYMBOL-FUNCTION",
        "SYMBOL-VALUE",
        "GET",
        "PUTPROP",
        "REMPROP",
        "SYMBOL-PLIST",
        "SET",
        "MAKUNBOUND",
        "FMAKUNBOUND",
        "MAKE-INSTANCE",
        "SLOT-VALUE",
        "CLASS-OF",
        "FIND-CLASS",
        "CLASS-NAME",
        "CLASS-PRECEDENCE-LIST",
        "SLOT-EXISTS-P",
        "SLOT-BOUNDP",
        "SLOT-MAKUNBOUND",
        "CALL-NEXT-METHOD",
        "NEXT-METHOD-P",
        "COMPUTE-RESTARTS",
        "FIND-RESTART",
        "INVOKE-RESTART",
        "INVOKE-RESTART-INTERACTIVELY",
        "RESTART-FUNCTION",
        "RESTART-NAME",
    ] {
        let value = Value::primitive(name);
        environment.define_function(name, value.clone());
        environment.define_function(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
    for (name, value) in [
        ("NIL", Value::Nil),
        ("T", Value::boolean(true)),
        ("CHAR-CODE-LIMIT", Value::Integer(0x11_00_00)),
        ("MOST-POSITIVE-CHAR-CODE", Value::Integer(0x10_FF_FF)),
    ] {
        environment.define(name, value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
}

fn default_input_arguments(arguments: &[Value], stream_index: usize, stream: &Value) -> Vec<Value> {
    let mut arguments = arguments.to_vec();
    match arguments.get(stream_index) {
        None => arguments.push(stream.clone()),
        Some(Value::Nil) | Some(Value::Boolean(true)) => arguments[stream_index] = stream.clone(),
        Some(_) => {}
    }
    arguments
}

pub(crate) fn read_with_standard_input(
    arguments: &[Value],
    stream: &Value,
    features: &[String],
) -> Result<Value, RuntimeError> {
    read_stream_form(
        "read",
        &default_input_arguments(arguments, 0, stream),
        false,
        features,
    )
}

pub(crate) fn read_preserving_whitespace_with_standard_input(
    arguments: &[Value],
    stream: &Value,
    features: &[String],
) -> Result<Value, RuntimeError> {
    read_stream_form(
        "read-preserving-whitespace",
        &default_input_arguments(arguments, 0, stream),
        true,
        features,
    )
}

pub(crate) fn read_char_with_standard_input(
    arguments: &[Value],
    stream: &Value,
) -> Result<Value, RuntimeError> {
    read_char(&default_input_arguments(arguments, 0, stream))
}

pub(crate) fn peek_char_with_standard_input(
    arguments: &[Value],
    stream: &Value,
) -> Result<Value, RuntimeError> {
    let stream_index = if matches!(arguments.first(), Some(Value::Stream(_))) {
        0
    } else {
        1
    };
    peek_char(&default_input_arguments(arguments, stream_index, stream))
}

pub(crate) fn unread_char_with_standard_input(
    arguments: &[Value],
    stream: &Value,
) -> Result<Value, RuntimeError> {
    unread_char(&default_input_arguments(arguments, 1, stream))
}

pub(crate) fn listen_with_standard_input(
    arguments: &[Value],
    stream: &Value,
) -> Result<Value, RuntimeError> {
    listen(&default_input_arguments(arguments, 0, stream))
}

pub(crate) fn clear_input_with_standard_input(
    arguments: &[Value],
    stream: &Value,
) -> Result<Value, RuntimeError> {
    clear_input(&default_input_arguments(arguments, 0, stream))
}

pub(crate) fn read_line_with_standard_input(
    arguments: &[Value],
    stream: &Value,
) -> Result<Value, RuntimeError> {
    read_line(&default_input_arguments(arguments, 0, stream))
}

pub(crate) fn read_sequence_with_standard_input(
    arguments: &[Value],
    stream: &Value,
) -> Result<Value, RuntimeError> {
    let mut arguments = arguments.to_vec();
    if matches!(
        arguments.get(1),
        Some(Value::Keyword(_)) | Some(Value::KeywordExact(_))
    ) {
        arguments.insert(1, stream.clone());
    } else {
        arguments = default_input_arguments(&arguments, 1, stream);
    }
    read_sequence(&arguments)
}

fn endp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "endp", 1)?;
    match arguments[0].list_items() {
        Some(items) => Ok(Value::boolean(items.is_empty())),
        None => Err(type_error("endp", "list", &arguments[0])),
    }
}

fn characterp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "characterp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Character(_))))
}

fn keywordp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "keywordp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Keyword(_) | Value::KeywordExact(_)
    )))
}

fn symbol_name_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-name", 1)?;
    let name = match &arguments[0] {
        Value::UninternedSymbol(name) => name.to_string(),
        value => {
            let name = value
                .symbol_name()
                .ok_or_else(|| type_error("symbol-name", "a symbol", &arguments[0]))?;
            let name = match package::split_symbol(name) {
                Some((_, symbol_name, _)) => symbol_name,
                None => name,
            };
            name.to_string()
        }
    };
    Ok(Value::string(name))
}

fn symbol_package_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbol-package", 1)?;
    let package_name = match &arguments[0] {
        Value::UninternedSymbol(_) => return Ok(Value::Nil),
        Value::Keyword(_) | Value::KeywordExact(_) => KEYWORD_PACKAGE.to_string(),
        Value::Nil | Value::Boolean(_) => COMMON_LISP_PACKAGE.to_string(),
        Value::Symbol(name) | Value::SymbolExact(name) => {
            match package::split_symbol(name.as_ref()) {
                Some((package_name, _, _)) => package::normalize_package_name(package_name),
                None => package::DEFAULT_PACKAGE.to_string(),
            }
        }
        Value::QualifiedSymbolExact {
            reference,
            package_len,
        } => package::normalize_package_name(&reference[..*package_len]),
        value => return Err(type_error("symbol-package", "a symbol", value)),
    };
    Ok(Value::package(package_name))
}

fn vectorp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vectorp", 1)?;
    Ok(Value::boolean(vector_elements(&arguments[0]).is_some()))
}

fn simple_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-vector-p", 1)?;
    Ok(Value::boolean(is_simple_vector_value(&arguments[0])))
}

fn typep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "typep", 2)?;
    Ok(Value::boolean(typep_value(&arguments[0], &arguments[1])?))
}

fn simple_condition_format_control(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-condition-format-control", 1)?;
    arguments[0]
        .simple_condition_format_control()
        .map(|control| Value::string(control.to_owned()))
        .ok_or_else(|| {
            type_error(
                "simple-condition-format-control",
                "SIMPLE-CONDITION",
                &arguments[0],
            )
        })
}

fn simple_condition_format_arguments(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-condition-format-arguments", 1)?;
    arguments[0]
        .simple_condition_format_arguments()
        .map(Value::list)
        .ok_or_else(|| {
            type_error(
                "simple-condition-format-arguments",
                "SIMPLE-CONDITION",
                &arguments[0],
            )
        })
}

fn type_error_datum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-error-datum", 1)?;
    arguments[0]
        .condition_slot("TYPE-ERROR", "DATUM")
        .ok_or_else(|| type_error("type-error-datum", "TYPE-ERROR", &arguments[0]))
}

fn type_error_expected_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-error-expected-type", 1)?;
    arguments[0]
        .condition_slot("TYPE-ERROR", "EXPECTED-TYPE")
        .ok_or_else(|| type_error("type-error-expected-type", "TYPE-ERROR", &arguments[0]))
}

fn reverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "reverse", 1)?;
    reverse_sequence("reverse", &arguments[0])
}

fn nreverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nreverse", 1)?;
    reverse_sequence("nreverse", &arguments[0])
}

fn reverse_sequence(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let mut items = sequence_elements(function, value)?;
    items.reverse();
    rebuild_sequence(function, value, items)
}

fn last(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("last", "one or two", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("last", "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument("last", value))
        .transpose()?
        .unwrap_or(1);
    if count == 0 {
        return Ok(Value::Nil);
    }
    let start = items.len().saturating_sub(count);
    Ok(Value::list(items[start..].to_vec()))
}

fn butlast(arguments: &[Value]) -> Result<Value, RuntimeError> {
    butlast_like("butlast", arguments)
}

fn nbutlast(arguments: &[Value]) -> Result<Value, RuntimeError> {
    butlast_like("nbutlast", arguments)
}

fn butlast_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    let count = arguments
        .get(1)
        .map(|value| index_argument(function, value))
        .transpose()?
        .unwrap_or(1);
    let end = items.len().saturating_sub(count);
    Ok(Value::list(items[..end].to_vec()))
}

fn copy_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-list", 1)?;
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("copy-list", "list", &arguments[0]));
    };
    Ok(Value::list(items))
}

fn copy_alist(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-alist", 1)?;
    let Some(entries) = arguments[0].list_items() else {
        return Err(type_error("copy-alist", "association list", &arguments[0]));
    };
    let copied = entries
        .into_iter()
        .map(|entry| match entry {
            Value::List(items) => Ok(Value::list(items.as_ref().clone())),
            Value::DottedList { items, tail } => Ok(Value::dotted_list(
                items.as_ref().clone(),
                tail.as_ref().clone(),
            )),
            value => Err(type_error("copy-alist", "association", &value)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::list(copied))
}

fn copy_tree(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-tree", 1)?;
    Ok(copy_tree_value(&arguments[0]))
}

fn copy_tree_value(value: &Value) -> Value {
    if value.is_typed_list() {
        return Value::list(
            value
                .list_items()
                .unwrap_or_default()
                .iter()
                .map(copy_tree_value)
                .collect(),
        );
    }
    match value {
        Value::List(items) => Value::list(items.iter().map(copy_tree_value).collect()),
        Value::DottedList { items, tail } => Value::dotted_list(
            items.iter().map(copy_tree_value).collect(),
            copy_tree_value(tail),
        ),
        _ => value.clone(),
    }
}

fn parse_array_dimensions(function: &str, value: &Value) -> Result<Vec<usize>, RuntimeError> {
    match value {
        Value::Integer(_) => Ok(vec![index_argument(function, value)?]),
        Value::Nil => Ok(Vec::new()),
        Value::List(_) | Value::Vector(_) => {
            let items = sequence_items(value).expect("list or vector has sequence items");
            items
                .iter()
                .map(|item| index_argument(function, item))
                .collect()
        }
        other => Err(type_error(
            function,
            "integer or sequence of integers",
            other,
        )),
    }
}

fn array_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
        Value::QualifiedSymbolExact {
            reference,
            package_len,
        } => Ok(normalize_name(&reference[*package_len + 2..])),
        other => Err(type_error(function, "keyword", other)),
    }
}

fn flatten_array_contents(
    function: &str,
    contents: &Value,
    dimensions: &[usize],
    output: &mut Vec<Value>,
) -> Result<(), RuntimeError> {
    if dimensions.is_empty() {
        output.push(contents.clone());
        return Ok(());
    }
    let Some(items) = sequence_items(contents) else {
        return Err(type_error(
            function,
            "nested sequence for :initial-contents",
            contents,
        ));
    };
    if items.len() != dimensions[0] {
        return Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} :initial-contents expected {} elements, got {}",
                dimensions[0],
                items.len()
            ),
            span: None,
        });
    }
    if dimensions.len() == 1 {
        output.extend(items);
    } else {
        for item in items {
            flatten_array_contents(function, &item, &dimensions[1..], output)?;
        }
    }
    Ok(())
}

fn array_coordinate_index(
    function: &str,
    dimensions: &[usize],
    indices: &[Value],
) -> Result<usize, RuntimeError> {
    let mut offset: usize = 0;
    for (axis, (dimension, value)) in dimensions.iter().zip(indices).enumerate() {
        let index = index_argument(function, value)?;
        if index >= *dimension {
            return Err(out_of_bounds(function, index));
        }
        let stride = dimensions[axis + 1..]
            .iter()
            .try_fold(1_usize, |stride, dimension| stride.checked_mul(*dimension))
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
        let contribution = index
            .checked_mul(stride)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
        offset = offset
            .checked_add(contribution)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} index is too large"),
                span: None,
            })?;
    }
    Ok(offset)
}

fn array_total_size_for(function: &str, dimensions: &[usize]) -> Result<usize, RuntimeError> {
    let Some((first, rest)) = dimensions.split_first() else {
        return Ok(1);
    };
    rest.iter().try_fold(*first, |total, dimension| {
        total
            .checked_mul(*dimension)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: format!("{function} array is too large"),
                span: None,
            })
    })
}

fn dimensions_for_array(value: &Value) -> Option<Vec<usize>> {
    match value {
        Value::Vector(items) => Some(vec![items.borrow().len()]),
        Value::Array { .. } => value.array_dimensions(),
        Value::String(text) => Some(vec![text.chars().count()]),
        _ if value.is_typed_vector() => value.vector_items().map(|items| vec![items.len()]),
        _ => None,
    }
}

fn array_elements(value: &Value) -> Option<Vec<Value>> {
    match value {
        Value::String(text) => Some(text.chars().map(Value::Character).collect()),
        Value::Array { .. } => value.array_items(),
        _ => value.vector_items(),
    }
}

fn sequence_items(value: &Value) -> Option<Vec<Value>> {
    value
        .list_items()
        .or_else(|| value.vector_items())
        .or_else(|| match value {
            Value::String(text) => Some(text.chars().map(Value::Character).collect()),
            _ => None,
        })
}

fn null(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "null", 1)?;
    Ok(Value::boolean(!arguments[0].is_truthy()))
}

fn atom(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atom", 1)?;
    Ok(Value::boolean(
        !matches!(&arguments[0], Value::List(_) | Value::DottedList { .. })
            && !arguments[0].is_typed_list(),
    ))
}

fn consp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "consp", 1)?;
    Ok(Value::boolean(
        arguments[0]
            .list_items()
            .is_some_and(|items| !items.is_empty())
            || matches!(&arguments[0], Value::DottedList { items, .. } if !items.is_empty()),
    ))
}

fn listp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "listp", 1)?;
    Ok(Value::boolean(
        matches!(&arguments[0], Value::Nil | Value::List(_)) || arguments[0].is_typed_list(),
    ))
}

fn numberp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numberp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_) | Value::Float(_) | Value::Complex { .. }
    )))
}

fn complexp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "complexp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Complex { .. }
    )))
}

fn integerp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integerp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Integer(_))))
}

fn floatp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "floatp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Float(_))))
}

fn rationalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_)
    )))
}

fn realp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "realp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Integer(_) | Value::Rational(_) | Value::Float(_)
    )))
}

fn stringp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "stringp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_))))
}

fn simple_string_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-string-p", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::String(_))))
}

fn symbolp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "symbolp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Nil
            | Value::Boolean(_)
            | Value::Symbol(_)
            | Value::UninternedSymbol(_)
            | Value::Keyword(_)
            | Value::SymbolExact(_)
            | Value::KeywordExact(_)
            | Value::QualifiedSymbolExact { .. }
    )))
}

fn packagep(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "packagep", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Package(_))))
}

fn functionp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "functionp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Function(_))))
}

fn eq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eq", 2)?;
    Ok(Value::boolean(arguments[0].eq_value(&arguments[1])))
}

fn eql(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "eql", 2)?;
    Ok(Value::boolean(eql_value(&arguments[0], &arguments[1])))
}

pub(crate) fn eql_value(left: &Value, right: &Value) -> bool {
    let numeric_equal = match (left, right) {
        (Value::Integer(left), Value::Integer(right)) => left == right,
        (Value::Rational(left), Value::Rational(right)) => left == right,
        (Value::Float(left), Value::Float(right)) => left == right,
        (
            Value::Complex {
                real: left_real,
                imaginary: left_imaginary,
            },
            Value::Complex {
                real: right_real,
                imaginary: right_imaginary,
            },
        ) => eql_value(left_real, right_real) && eql_value(left_imaginary, right_imaginary),
        _ => false,
    };
    left.eq_value(right) || numeric_equal
}

fn equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equal", 2)?;
    Ok(Value::boolean(arguments[0].equal_value(&arguments[1])))
}

fn equalp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "equalp", 2)?;
    Ok(Value::boolean(equalp_value(&arguments[0], &arguments[1])))
}

fn equalp_value(left: &Value, right: &Value) -> bool {
    if let (Ok(left), Ok(right)) = (number(left), number(right)) {
        return numeric_equalp(left, right);
    }
    match (left, right) {
        (Value::String(left), Value::String(right)) => left.eq_ignore_ascii_case(right),
        (Value::Character(left), Value::Character(right)) => {
            left.to_ascii_uppercase() == right.to_ascii_uppercase()
        }
        (Value::List(left), Value::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (Value::Vector(left), Value::Vector(right)) => {
            let left = left.borrow();
            let right = right.borrow();
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::Array {
                dimensions: left_dimensions,
                elements: left_elements,
                ..
            },
            Value::Array {
                dimensions: right_dimensions,
                elements: right_elements,
                ..
            },
        ) => {
            let left_elements = left_elements.borrow();
            let right_elements = right_elements.borrow();
            left_dimensions == right_dimensions
                && left_elements.len() == right_elements.len()
                && left_elements
                    .iter()
                    .zip(right_elements.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::DottedList {
                items: left,
                tail: left_tail,
            },
            Value::DottedList {
                items: right,
                tail: right_tail,
            },
        ) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
                && equalp_value(left_tail, right_tail)
        }
        _ => eql_value(left, right),
    }
}

fn identity(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "identity", 1)?;
    Ok(arguments[0].clone())
}

fn type_of(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "type-of", 1)?;
    Ok(Value::symbol(arguments[0].type_of_name()))
}

fn print_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("print", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], true);
    write_destination("print", arguments.get(1), "\n")?;
    write_destination("print", arguments.get(1), &text)?;
    write_destination("print", arguments.get(1), "\n")?;
    Ok(arguments[0].clone())
}

fn princ(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("princ", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], false);
    write_destination("princ", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

fn prin1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("prin1", "1 to 2", arguments.len()));
    }
    let text = printed_value(&arguments[0], true);
    write_destination("prin1", arguments.get(1), &text)?;
    Ok(arguments[0].clone())
}

fn write_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write", "at least 1", arguments.len()));
    }
    let (escape, stream) = parse_print_options("write", &arguments[1..], true)?;
    let text = printed_value(&arguments[0], escape);
    write_destination("write", stream.as_ref(), &text)?;
    Ok(arguments[0].clone())
}

fn write_to_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("write-to-string", "at least 1", arguments.len()));
    }
    let (escape, _) = parse_print_options("write-to-string", &arguments[1..], false)?;
    Ok(Value::string(printed_value(&arguments[0], escape)))
}

fn parse_print_options(
    function: &str,
    options: &[Value],
    allow_stream: bool,
) -> Result<(bool, Option<Value>), RuntimeError> {
    if options.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} requires keyword/value pairs"),
            span: None,
        });
    }
    let mut escape = true;
    let mut stream = None;
    for pair in options.chunks_exact(2) {
        let name = array_option_name(function, &pair[0])?;
        match name.as_str() {
            "ESCAPE" => escape = pair[1].is_truthy(),
            "STREAM" if allow_stream => stream = Some(pair[1].clone()),
            "STREAM" => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not support keyword :stream"),
                    span: None,
                });
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    Ok((escape, stream))
}

fn printed_value(value: &Value, escape: bool) -> String {
    match value {
        Value::String(value) if !escape => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::List(values) => {
            let contents = values
                .iter()
                .map(|value| printed_value(value, escape))
                .collect::<Vec<_>>()
                .join(" ");
            format!("({contents})")
        }
        Value::DottedList { items, tail } => {
            let mut text = String::from("(");
            if !items.is_empty() {
                text.push_str(
                    &items
                        .iter()
                        .map(|value| printed_value(value, escape))
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                text.push(' ');
            }
            text.push_str(". ");
            text.push_str(&printed_value(tail, escape));
            text.push(')');
            text
        }
        Value::Vector(values) => {
            let values = values.borrow();
            let contents = values
                .iter()
                .map(|value| printed_value(value, escape))
                .collect::<Vec<_>>()
                .join(" ");
            format!("#({contents})")
        }
        _ => value.to_string(),
    }
}

fn read_from_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    read_from_string_with_features(arguments, &[])
}

pub(crate) fn read_from_string_with_features(
    arguments: &[Value],
    features: &[String],
) -> Result<Value, RuntimeError> {
    if arguments.len() < 1 {
        return Err(arity("read-from-string", "at least 1", arguments.len()));
    }
    let source = match &arguments[0] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("read-from-string", "a string", value)),
    };
    let eof_error_p = arguments.get(1).map_or(true, Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let source_length = source.chars().count();
    let mut start = 0;
    let mut end = source_length;
    let mut preserving_whitespace = false;
    let keyword_arguments = arguments.get(3..).unwrap_or_default();
    if keyword_arguments.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "read-from-string keyword arguments must be name/value pairs".to_string(),
            span: None,
        });
    }
    for pair in keyword_arguments.chunks_exact(2) {
        let name = match &pair[0] {
            Value::Keyword(name) | Value::KeywordExact(name) => name.as_ref(),
            value => return Err(type_error("read-from-string", "a keyword", value)),
        };
        if name.eq_ignore_ascii_case("START") {
            start = stream_bound("read-from-string", &pair[1], source_length)?;
        } else if name.eq_ignore_ascii_case("END") {
            end = stream_bound("read-from-string", &pair[1], source_length)?;
        } else if name.eq_ignore_ascii_case("PRESERVE-WHITESPACE") {
            preserving_whitespace = pair[1].is_truthy();
        } else {
            return Err(RuntimeError::InvalidForm {
                message: format!("read-from-string does not support keyword :{name}"),
                span: None,
            });
        }
    }
    if start > end {
        return Err(RuntimeError::InvalidForm {
            message: "read-from-string start must not exceed end".to_string(),
            span: None,
        });
    }
    let window = source
        .chars()
        .skip(start)
        .take(end - start)
        .collect::<String>();
    let mut reader = Reader::with_features(&window, features.iter());
    let (value, byte_position) = match reader.read_form()? {
        Some(form) => {
            let value = quoted_form_value(&form)?;
            let byte_position = if preserving_whitespace {
                form.span.end
            } else {
                reader.consume_one_whitespace_after_form();
                reader.position()
            };
            (value, byte_position)
        }
        None => {
            let position = reader.position();
            if eof_error_p {
                return Err(RuntimeError::Read(ReadError::new(
                    ReadErrorKind::UnexpectedEnd { context: "a form" },
                    Span::new(position, position),
                )));
            }
            (eof_value, position)
        }
    };
    let local_position = window[..byte_position].chars().count();
    let position = start
        .checked_add(local_position)
        .ok_or(RuntimeError::NumericOverflow)?;
    let position = i64::try_from(position).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![value, Value::Integer(position)]))
}

fn read(arguments: &[Value]) -> Result<Value, RuntimeError> {
    read_stream_form("read", arguments, false, &[])
}

fn read_preserving_whitespace(arguments: &[Value]) -> Result<Value, RuntimeError> {
    read_stream_form("read-preserving-whitespace", arguments, true, &[])
}

fn read_stream_form(
    function: &str,
    arguments: &[Value],
    preserving_whitespace: bool,
    features: &[String],
) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity(function, "0 to 4", arguments.len()));
    }
    let stream = match arguments.first() {
        Some(Value::Stream(stream)) => stream,
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
            return Err(RuntimeError::InvalidForm {
                message: format!(
                    "{function} requires an explicit input stream; standard input is unavailable"
                ),
                span: None,
            });
        }
        Some(value) => return Err(type_error(function, "an input stream", value)),
    };
    let eof_error_p = arguments.get(1).map_or(true, Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let source = {
        let stream = stream.borrow();
        if !stream.is_input() {
            return Err(stream_state_error(function, "an input stream"));
        }
        stream
            .remaining_input()
            .ok_or_else(|| stream_state_error(function, "an open input stream"))?
    };
    let mut reader = Reader::with_features(&source, features.iter());
    let (value, byte_position) = match reader.read_form()? {
        Some(form) => {
            let value = quoted_form_value(&form)?;
            let byte_position = if preserving_whitespace {
                form.span.end
            } else {
                reader.consume_one_whitespace_after_form();
                reader.position()
            };
            (value, byte_position)
        }
        None => {
            let position = reader.position();
            let consumed = source[..position].chars().count();
            if !stream.borrow_mut().consume_input(consumed) {
                return Err(stream_state_error(function, "an open input stream"));
            }
            if eof_error_p {
                return Err(RuntimeError::Read(ReadError::new(
                    ReadErrorKind::UnexpectedEnd { context: "a form" },
                    Span::new(position, position),
                )));
            }
            return Ok(eof_value);
        }
    };
    let consumed = source[..byte_position].chars().count();
    if !stream.borrow_mut().consume_input(consumed) {
        return Err(stream_state_error(function, "an open input stream"));
    }
    Ok(value)
}

fn exact(arguments: &[Value], function: &str, expected: usize) -> Result<(), RuntimeError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(arity(function, expected.to_string(), arguments.len()))
    }
}

fn arity(function: &str, expected: impl Into<String>, actual: usize) -> RuntimeError {
    RuntimeError::Arity {
        function: function.to_string(),
        expected: expected.into(),
        actual,
    }
}

fn type_error(function: &str, expected: &str, value: &Value) -> RuntimeError {
    RuntimeError::Type {
        expected: format!("{function} requires {expected}"),
        actual: value.type_name().to_string(),
        span: None,
    }
}
