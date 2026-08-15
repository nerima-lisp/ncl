use std::cell::RefCell;
use std::cmp::Ordering;
use std::f64::consts::PI;
use std::path::PathBuf;
use std::rc::Rc;

use ncl_syntax::{ReadError, ReadErrorKind, Reader, Span};

use crate::environment::normalize_name;
use crate::evaluator::quoted_form_value;
use crate::package::{self, COMMON_LISP_PACKAGE, KEYWORD_PACKAGE};
use crate::{Environment, Function, Rational, RuntimeError, Stream, Value};

pub fn install(environment: &Environment) {
    for (name, function) in [
        ("+", add as _),
        ("-", subtract as _),
        ("*", multiply as _),
        ("/", divide as _),
        ("sin", sine as _),
        ("cos", cosine as _),
        ("tan", tangent as _),
        ("asin", arc_sine as _),
        ("acos", arc_cosine as _),
        ("atan", arc_tangent as _),
        ("sinh", hyperbolic_sine as _),
        ("cosh", hyperbolic_cosine as _),
        ("tanh", hyperbolic_tangent as _),
        ("asinh", arc_hyperbolic_sine as _),
        ("acosh", arc_hyperbolic_cosine as _),
        ("atanh", arc_hyperbolic_tangent as _),
        ("exp", exponential as _),
        ("log", logarithm as _),
        ("cis", cis as _),
        ("expt", exponentiate as _),
        ("sqrt", square_root as _),
        ("signum", signum as _),
        ("float", float_value as _),
        ("complex", complex as _),
        ("conjugate", conjugate as _),
        ("phase", phase as _),
        ("realpart", realpart as _),
        ("imagpart", imagpart as _),
        ("rational", rational as _),
        ("rationalize", rationalize as _),
        ("=", numeric_equal as _),
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
        ("byte", byte as _),
        ("byte-size", byte_size as _),
        ("byte-position", byte_position as _),
        ("ldb", ldb as _),
        ("ldb-test", ldb_test as _),
        ("dpb", dpb as _),
        ("mask-field", mask_field as _),
        ("deposit-field", deposit_field as _),
        ("logbitp", logbitp as _),
        ("logand", logand as _),
        ("logior", logior as _),
        ("logxor", logxor as _),
        ("lognand", lognand as _),
        ("lognor", lognor as _),
        ("logandc1", logandc1 as _),
        ("logandc2", logandc2 as _),
        ("logorc1", logorc1 as _),
        ("logorc2", logorc2 as _),
        ("logeqv", logeqv as _),
        ("boole", boole as _),
        ("lognot", lognot as _),
        ("logtest", logtest as _),
        ("logcount", logcount as _),
        ("integer-length", integer_length as _),
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
        ("first", first as _),
        ("second", second as _),
        ("third", third as _),
        ("fourth", fourth as _),
        ("fifth", fifth as _),
        ("sixth", sixth as _),
        ("seventh", seventh as _),
        ("eighth", eighth as _),
        ("ninth", ninth as _),
        ("tenth", tenth as _),
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
        ("make-sequence", make_sequence as _),
        ("aref", aref as _),
        ("svref", svref as _),
        ("bit", bit as _),
        ("sbit", sbit as _),
        ("row-major-aref", row_major_aref as _),
        ("array-row-major-index", array_row_major_index as _),
        ("array-in-bounds-p", array_in_bounds_p as _),
        ("array-element-type", array_element_type as _),
        ("array-has-fill-pointer-p", array_has_fill_pointer_p as _),
        ("adjustable-array-p", adjustable_array_p as _),
        ("array-displacement", array_displacement as _),
        ("simple-array-p", simple_array_p as _),
        ("arrayp", arrayp as _),
        ("array-rank", array_rank as _),
        ("array-dimensions", array_dimensions as _),
        ("array-dimension", array_dimension as _),
        ("array-total-size", array_total_size as _),
        ("adjust-array", adjust_array as _),
        ("make-hash-table", make_hash_table as _),
        ("gethash", gethash as _),
        ("remhash", remhash as _),
        ("clrhash", clrhash as _),
        ("hash-table-p", hash_table_p as _),
        ("hash-table-count", hash_table_count as _),
        ("hash-table-test", hash_table_test_value as _),
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
        ("stringp", stringp as _),
        ("simple-string-p", simple_string_p as _),
        ("characterp", characterp as _),
        ("symbolp", symbolp as _),
        ("packagep", packagep as _),
        ("keywordp", keywordp as _),
        ("symbol-name", symbol_name_value as _),
        ("symbol-package", symbol_package_value as _),
        ("vectorp", vectorp as _),
        ("fill-pointer", fill_pointer as _),
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
        ("__NCL_THE_CHECK", the_check as _),
        ("__NCL_REQUIRE_INTEGER", require_integer as _),
        ("__NCL_REQUIRE_LIST", require_list as _),
        (
            "__NCL_APPEND_OUTPUT_TO_STRING",
            append_output_to_string as _,
        ),
        ("__NCL_ECASE_ERROR", ecase_error as _),
        ("__NCL_ETYPECASE_ERROR", etypecase_error as _),
        ("print", print_value as _),
        ("princ", princ as _),
        ("prin1", prin1 as _),
        ("write", write_value as _),
        ("format", format_value as _),
        ("write-to-string", write_to_string as _),
        ("read-from-string", read_from_string as _),
        ("read", read as _),
        (
            "read-preserving-whitespace",
            read_preserving_whitespace as _,
        ),
        ("make-string-input-stream", make_string_input_stream as _),
        ("%stream-input-position", stream_input_position as _),
        ("make-string-output-stream", make_string_output_stream as _),
        ("open", open_file as _),
        ("probe-file", probe_file as _),
        ("delete-file", delete_file as _),
        ("rename-file", rename_file as _),
        ("file-write-date", file_write_date as _),
        ("truename", truename as _),
        ("get-output-stream-string", get_output_stream_string as _),
        ("read-char", read_char as _),
        ("peek-char", peek_char as _),
        ("unread-char", unread_char as _),
        ("read-line", read_line as _),
        ("write-char", write_char as _),
        ("write-string", write_string as _),
        ("terpri", terpri as _),
        ("fresh-line", fresh_line as _),
        ("write-line", write_line as _),
        ("close", close_stream as _),
        ("streamp", streamp as _),
        ("input-stream-p", input_stream_p as _),
        ("output-stream-p", output_stream_p as _),
    ] {
        let value = Value::builtin(name, function);
        let normalized = normalize_name(name);
        environment.define(normalized.clone(), value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{normalized}"), value);
    }
    for name in [
        "EVAL",
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
        "UPGRADED-ARRAY-ELEMENT-TYPE",
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
        "FIND-PACKAGE",
        "PACKAGE-NAME",
        "PACKAGE-USE-LIST",
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
        "COMPILER-MACRO-FUNCTION",
        "SPECIAL-OPERATOR-P",
        "COMPILED-FUNCTION-P",
        "FUNCTION-LAMBDA-EXPRESSION",
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
        "ALLOCATE-INSTANCE",
        "CHANGE-CLASS",
        "REINITIALIZE-INSTANCE",
        "SHARED-INITIALIZE",
        "ENSURE-GENERIC-FUNCTION",
        "FIND-METHOD",
        "COMPUTE-APPLICABLE-METHODS",
        "GENERIC-FUNCTION-CLASS",
        "GENERIC-FUNCTION-METHODS",
        "GENERIC-FUNCTION-NAME",
        "METHOD-CLASS",
        "METHOD-COMBINATION",
        "METHOD-FUNCTION",
        "METHOD-GENERIC-FUNCTION",
        "METHOD-LAMBDA-LIST",
        "METHOD-QUALIFIERS",
        "METHOD-SPECIALIZERS",
        "SLOT-VALUE",
        "CLASS-OF",
        "FIND-CLASS",
        "CLASS-NAME",
        "SLOT-EXISTS-P",
        "SLOT-BOUNDP",
        "SLOT-MAKUNBOUND",
        "CALL-NEXT-METHOD",
        "NEXT-METHOD-P",
        "COMPUTE-RESTARTS",
        "FIND-RESTART",
        "INVOKE-RESTART",
        "RESTART-NAME",
    ] {
        let value = Value::primitive(name);
        environment.define(name, value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
    for (name, value) in [
        ("NIL", Value::Nil),
        ("T", Value::boolean(true)),
        ("ARRAY-RANK-LIMIT", Value::Integer(i64::MAX)),
        ("ARRAY-DIMENSION-LIMIT", Value::Integer(i64::MAX)),
        ("ARRAY-TOTAL-SIZE-LIMIT", Value::Integer(i64::MAX)),
        ("CHAR-CODE-LIMIT", Value::Integer(0x11_00_00)),
        ("MOST-POSITIVE-CHAR-CODE", Value::Integer(0x10_FF_FF)),
        ("BOOLE-CLR", Value::Integer(0)),
        ("BOOLE-SET", Value::Integer(1)),
        ("BOOLE-1", Value::Integer(2)),
        ("BOOLE-2", Value::Integer(3)),
        ("BOOLE-C1", Value::Integer(4)),
        ("BOOLE-C2", Value::Integer(5)),
        ("BOOLE-AND", Value::Integer(6)),
        ("BOOLE-IOR", Value::Integer(7)),
        ("BOOLE-XOR", Value::Integer(8)),
        ("BOOLE-EQV", Value::Integer(9)),
        ("BOOLE-NAND", Value::Integer(10)),
        ("BOOLE-NOR", Value::Integer(11)),
        ("BOOLE-ANDC1", Value::Integer(12)),
        ("BOOLE-ANDC2", Value::Integer(13)),
        ("BOOLE-ORC1", Value::Integer(14)),
        ("BOOLE-ORC2", Value::Integer(15)),
    ] {
        environment.define(name, value.clone());
        environment.define(format!("{COMMON_LISP_PACKAGE}::{name}"), value);
    }
}

fn add(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Numeric::Real(Number::Integer(0));
    for argument in arguments {
        result = add_numeric(result, numeric_argument("+", argument)?)?;
    }
    numeric_to_value(result)
}

fn subtract(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("-", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| numeric_argument("-", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    if values.len() == 1 {
        result = negate_numeric(result)?;
    } else {
        for value in &values[1..] {
            result = subtract_numeric(result, *value)?;
        }
    }
    numeric_to_value(result)
}

fn multiply(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = Numeric::Real(Number::Integer(1));
    for argument in arguments {
        result = multiply_numeric(result, numeric_argument("*", argument)?)?;
    }
    numeric_to_value(result)
}

fn divide(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("/", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| numeric_argument("/", value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result;
    if values.len() == 1 {
        result = divide_numeric(Numeric::Real(Number::Integer(1)), values[0])?;
    } else {
        result = values[0];
        for value in &values[1..] {
            result = divide_numeric(result, *value)?;
        }
    }
    numeric_to_value(result)
}

fn exponentiate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "expt", 2)?;
    let base = numeric_argument("expt", &arguments[0])?;
    let exponent = numeric_argument("expt", &arguments[1])?;

    if let (Numeric::Real(base), Numeric::Real(exponent)) = (base, exponent) {
        if !base.is_float() {
            if let Some((exponent_numerator, exponent_denominator)) = exponent.exact_parts() {
                if exponent_denominator == 1 {
                    return number_to_value(exact_power(base, exponent_numerator)?);
                }
            }
        }

        if base.as_float() >= 0.0 || float_is_integer(exponent.as_float()) {
            return Ok(Value::Float(base.as_float().powf(exponent.as_float())));
        }

        return exponentiate_complex(Numeric::Real(base), Numeric::Real(exponent));
    }

    exponentiate_complex(base, exponent)
}

fn exponentiate_complex(base: Numeric, exponent: Numeric) -> Result<Value, RuntimeError> {
    let (base_real, base_imag) = base.into_complex();
    let (exponent_real, exponent_imag) = exponent.into_complex();

    if base_real.as_float() == 0.0 && base_imag.as_float() == 0.0 {
        return zero_power(exponent_real, exponent_imag);
    }

    let base_real = base_real.as_float();
    let base_imag = base_imag.as_float();
    let exponent_real = exponent_real.as_float();
    let exponent_imag = exponent_imag.as_float();

    let magnitude = base_real.hypot(base_imag);
    let angle = base_imag.atan2(base_real);
    let log_real = magnitude.ln();
    let log_imag = angle;

    let power_real = exponent_real * log_real - exponent_imag * log_imag;
    let power_imag = exponent_real * log_imag + exponent_imag * log_real;
    let scale = power_real.exp();
    let real_part = canonicalize_float(scale * power_imag.cos());
    let imag_part = canonicalize_float(scale * power_imag.sin());

    Ok(Value::complex(
        number_to_value(real_part)?,
        number_to_value(imag_part)?,
    ))
}

fn sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sin", 1)?;
    numeric_to_value(sine_numeric(numeric_argument("sin", &arguments[0])?))
}

fn sine_numeric(value: Numeric) -> Numeric {
    match value {
        Numeric::Real(value) => Numeric::Real(Number::Float(value.as_float().sin())),
        Numeric::Complex { real, imag } => {
            let real_part = canonicalize_float(real.as_float().sin() * imag.as_float().cosh());
            let imag_part = canonicalize_float(real.as_float().cos() * imag.as_float().sinh());
            if imag_part.as_float() == 0.0 {
                Numeric::Real(real_part)
            } else {
                Numeric::Complex {
                    real: real_part,
                    imag: imag_part,
                }
            }
        }
    }
}

fn cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cos", 1)?;
    numeric_to_value(cosine_numeric(numeric_argument("cos", &arguments[0])?))
}

fn cosine_numeric(value: Numeric) -> Numeric {
    match value {
        Numeric::Real(value) => Numeric::Real(Number::Float(value.as_float().cos())),
        Numeric::Complex { real, imag } => {
            let real_part = canonicalize_float(real.as_float().cos() * imag.as_float().cosh());
            let imag_part = canonicalize_float(-(real.as_float().sin() * imag.as_float().sinh()));
            if imag_part.as_float() == 0.0 {
                Numeric::Real(real_part)
            } else {
                Numeric::Complex {
                    real: real_part,
                    imag: imag_part,
                }
            }
        }
    }
}

fn tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tan", 1)?;
    let value = numeric_argument("tan", &arguments[0])?;
    numeric_to_value(divide_numeric(sine_numeric(value), cosine_numeric(value))?)
}

fn arc_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "asin", 1)?;
    numeric_to_value(arc_sine_numeric(numeric_argument("asin", &arguments[0])?)?)
}

fn arc_sine_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let imaginary_unit = Numeric::Complex {
        real: Number::Integer(0),
        imag: Number::Integer(1),
    };
    let one = Numeric::Real(Number::Integer(1));
    let negative_imaginary_unit = Numeric::Complex {
        real: Number::Integer(0),
        imag: Number::Integer(-1),
    };
    let value_squared = multiply_numeric(value, value)?;
    let radicand = subtract_numeric(one, value_squared)?;
    let root = square_root_numeric(radicand)?;
    let sum = add_numeric(multiply_numeric(imaginary_unit, value)?, root)?;

    Ok(canonicalize_numeric(multiply_numeric(
        negative_imaginary_unit,
        logarithm_numeric(sum),
    )?))
}

fn arc_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acos", 1)?;
    numeric_to_value(arc_cosine_numeric(numeric_argument(
        "acos",
        &arguments[0],
    )?)?)
}

fn arc_cosine_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    Ok(canonicalize_numeric(subtract_numeric(
        Numeric::Real(Number::Float(PI / 2.0)),
        arc_sine_numeric(value)?,
    )?))
}

fn arc_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    match arguments.len() {
        1 => numeric_to_value(arc_tangent_numeric(numeric_argument(
            "atan",
            &arguments[0],
        )?)?),
        2 => {
            let y = number_argument("atan", &real_number_argument("atan", &arguments[0])?)?;
            let x = number_argument("atan", &real_number_argument("atan", &arguments[1])?)?;
            number_to_value(arc_tangent_real(y, x))
        }
        _ => Err(arity("atan", "1 or 2", arguments.len())),
    }
}

fn arc_tangent_real(y: Number, x: Number) -> Number {
    canonicalize_number(Number::Float(y.as_float().atan2(x.as_float())))
}

fn arc_tangent_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let imaginary_unit = Numeric::Complex {
        real: Number::Integer(0),
        imag: Number::Integer(1),
    };
    let one = Numeric::Real(Number::Integer(1));
    let difference = subtract_numeric(
        logarithm_numeric(add_numeric(one, multiply_numeric(imaginary_unit, value)?)?),
        logarithm_numeric(subtract_numeric(
            one,
            multiply_numeric(imaginary_unit, value)?,
        )?),
    )?;

    Ok(canonicalize_numeric(multiply_numeric(
        Numeric::Complex {
            real: Number::Integer(0),
            imag: Number::Float(-0.5),
        },
        difference,
    )?))
}

fn hyperbolic_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sinh", 1)?;
    numeric_to_value(hyperbolic_sine_numeric(numeric_argument(
        "sinh",
        &arguments[0],
    )?))
}

fn hyperbolic_sine_numeric(value: Numeric) -> Numeric {
    match value {
        Numeric::Real(value) => Numeric::Real(Number::Float(value.as_float().sinh())),
        Numeric::Complex { real, imag } => {
            let real_part = canonicalize_float(real.as_float().sinh() * imag.as_float().cos());
            let imag_part = canonicalize_float(real.as_float().cosh() * imag.as_float().sin());
            if imag_part.as_float() == 0.0 {
                Numeric::Real(real_part)
            } else {
                Numeric::Complex {
                    real: real_part,
                    imag: imag_part,
                }
            }
        }
    }
}

fn hyperbolic_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cosh", 1)?;
    numeric_to_value(hyperbolic_cosine_numeric(numeric_argument(
        "cosh",
        &arguments[0],
    )?))
}

fn hyperbolic_cosine_numeric(value: Numeric) -> Numeric {
    match value {
        Numeric::Real(value) => Numeric::Real(Number::Float(value.as_float().cosh())),
        Numeric::Complex { real, imag } => {
            let real_part = canonicalize_float(real.as_float().cosh() * imag.as_float().cos());
            let imag_part = canonicalize_float(real.as_float().sinh() * imag.as_float().sin());
            if imag_part.as_float() == 0.0 {
                Numeric::Real(real_part)
            } else {
                Numeric::Complex {
                    real: real_part,
                    imag: imag_part,
                }
            }
        }
    }
}

fn hyperbolic_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "tanh", 1)?;
    let value = numeric_argument("tanh", &arguments[0])?;
    numeric_to_value(divide_numeric(
        hyperbolic_sine_numeric(value),
        hyperbolic_cosine_numeric(value),
    )?)
}

fn arc_hyperbolic_sine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "asinh", 1)?;
    numeric_to_value(arc_hyperbolic_sine_numeric(numeric_argument(
        "asinh",
        &arguments[0],
    )?)?)
}

fn arc_hyperbolic_sine_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let one = Numeric::Real(Number::Integer(1));
    let value_squared = multiply_numeric(value, value)?;
    let radicand = add_numeric(one, value_squared)?;
    let root = square_root_numeric(radicand)?;
    let sum = add_numeric(value, root)?;
    Ok(canonicalize_numeric(logarithm_numeric(sum)))
}

fn arc_hyperbolic_cosine(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acosh", 1)?;
    numeric_to_value(arc_hyperbolic_cosine_numeric(numeric_argument(
        "acosh",
        &arguments[0],
    )?)?)
}

fn arc_hyperbolic_cosine_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let one = Numeric::Real(Number::Integer(1));
    let lower = square_root_numeric(subtract_numeric(value, one)?)?;
    let upper = square_root_numeric(add_numeric(value, one)?)?;
    let sum = add_numeric(value, multiply_numeric(lower, upper)?)?;
    Ok(canonicalize_numeric(logarithm_numeric(sum)))
}

fn arc_hyperbolic_tangent(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atanh", 1)?;
    numeric_to_value(arc_hyperbolic_tangent_numeric(numeric_argument(
        "atanh",
        &arguments[0],
    )?)?)
}

fn arc_hyperbolic_tangent_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    let one = Numeric::Real(Number::Integer(1));
    let numerator = logarithm_numeric(add_numeric(one, value)?);
    let denominator = logarithm_numeric(subtract_numeric(one, value)?);
    Ok(canonicalize_numeric(multiply_numeric(
        Numeric::Real(Number::Float(0.5)),
        subtract_numeric(numerator, denominator)?,
    )?))
}

fn exponential(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "exp", 1)?;
    match numeric_argument("exp", &arguments[0])? {
        Numeric::Real(value) => Ok(Value::Float(value.as_float().exp())),
        Numeric::Complex { real, imag } => {
            let scale = real.as_float().exp();
            let angle = imag.as_float();
            Ok(Value::complex(
                number_to_value(canonicalize_float(scale * angle.cos()))?,
                number_to_value(canonicalize_float(scale * angle.sin()))?,
            ))
        }
    }
}

fn logarithm(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("log", "1 or 2", arguments.len()));
    }

    let value = logarithm_numeric(numeric_argument("log", &arguments[0])?);
    if arguments.len() == 1 {
        return numeric_to_value(value);
    }

    let base = logarithm_numeric(numeric_argument("log", &arguments[1])?);
    numeric_to_value(divide_numeric(value, base)?)
}

fn logarithm_numeric(value: Numeric) -> Numeric {
    let (real, imag) = value.into_complex();
    let magnitude = real.as_float().hypot(imag.as_float());
    let angle = imag.as_float().atan2(real.as_float());
    let real_part = canonicalize_float(magnitude.ln());
    let imag_part = canonicalize_float(angle);

    if imag_part.as_float() == 0.0 {
        Numeric::Real(real_part)
    } else {
        Numeric::Complex {
            real: real_part,
            imag: imag_part,
        }
    }
}

fn cis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cis", 1)?;
    let angle = number_argument("cis", &arguments[0])?.as_float();
    Ok(Value::complex(
        number_to_value(canonicalize_float(angle.cos()))?,
        number_to_value(canonicalize_float(angle.sin()))?,
    ))
}

fn zero_power(exponent_real: Number, exponent_imag: Number) -> Result<Value, RuntimeError> {
    if exponent_imag.as_float() != 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }

    if exponent_real.as_float() == 0.0 {
        return Ok(Value::Integer(1));
    }

    if exponent_real.as_float() < 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }

    if let Some((exponent_numerator, exponent_denominator)) = exponent_real.exact_parts() {
        if exponent_numerator > 0 && exponent_denominator == 1 {
            return Ok(Value::Integer(0));
        }
    }

    Ok(Value::Float(0.0))
}

fn canonicalize_float(value: f64) -> Number {
    if value.abs() < 1e-12 {
        Number::Integer(0)
    } else {
        Number::Float(value)
    }
}

fn float_is_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0
}

fn square_root(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "sqrt", 1)?;
    match numeric_argument("sqrt", &arguments[0])? {
        Numeric::Real(number) => square_root_real(number),
        Numeric::Complex { real, imag } => square_root_complex(real, imag),
    }
}

fn square_root_real(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) if value >= 0 => {
            let root = integer_square_root(value as u128);
            if root * root == value as u128 {
                Ok(Value::Integer(root as i64))
            } else {
                Ok(Value::Float((value as f64).sqrt()))
            }
        }
        Number::Integer(value) => Ok(Value::complex(
            Value::Integer(0),
            square_root_real(Number::Integer(
                value.checked_neg().ok_or(RuntimeError::NumericOverflow)?,
            ))?,
        )),
        Number::Rational(value) if value.numerator() >= 0 => {
            let numerator = value.numerator() as u128;
            let denominator = value.denominator() as u128;
            let numerator_root = integer_square_root(numerator);
            let denominator_root = integer_square_root(denominator);
            if numerator_root * numerator_root == numerator
                && denominator_root * denominator_root == denominator
            {
                rational_number(numerator_root as i128, denominator_root as i128)
                    .and_then(number_to_value)
            } else {
                Ok(Value::Float(
                    (value.numerator() as f64 / value.denominator() as f64).sqrt(),
                ))
            }
        }
        Number::Rational(value) => Ok(Value::complex(
            Value::Integer(0),
            square_root_real(Number::Rational(Rational::new(
                -i128::from(value.numerator()),
                i128::from(value.denominator()),
            )?))?,
        )),
        Number::Float(value) if value >= 0.0 => Ok(Value::Float(value.sqrt())),
        Number::Float(value) => Ok(Value::complex(
            Value::Integer(0),
            Value::Float((-value).sqrt()),
        )),
    }
}

fn integer_square_root(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let bits = 128 - value.leading_zeros();
    let mut root = 1u128 << (bits / 2 + 1);
    loop {
        let next = (root + value / root) / 2;
        if next >= root {
            return root;
        }
        root = next;
    }
}

fn square_root_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    let real = real.as_float();
    let imag = imag.as_float();
    let magnitude = real.hypot(imag);
    let real_part = ((magnitude + real) / 2.0).sqrt();
    let imag_magnitude = ((magnitude - real) / 2.0).sqrt();
    let imag_part = if imag < 0.0 {
        -imag_magnitude
    } else {
        imag_magnitude
    };

    Ok(Value::complex(
        Value::Float(real_part),
        Value::Float(imag_part),
    ))
}

fn signum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "signum", 1)?;
    match numeric_argument("signum", &arguments[0])? {
        Numeric::Real(number) => signum_real(number),
        Numeric::Complex { real, imag } => signum_complex(real, imag),
    }
}

fn signum_real(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value.signum())),
        Number::Rational(value) => Ok(Value::Integer(value.numerator().signum())),
        Number::Float(value) if value.is_nan() => Err(RuntimeError::InvalidForm {
            message: "signum of NaN is undefined".to_owned(),
            span: None,
        }),
        Number::Float(value) if value == 0.0 => Ok(Value::Float(value)),
        Number::Float(value) => Ok(Value::Float(if value.is_sign_negative() {
            -1.0
        } else {
            1.0
        })),
    }
}

fn signum_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    if real.as_float() == 0.0 && imag.as_float() == 0.0 {
        return numeric_to_value(Numeric::Complex { real, imag });
    }

    let magnitude = absolute_complex(real, imag)?;
    let magnitude = numeric_argument("signum", &magnitude)?;
    let value = Numeric::Complex { real, imag };
    numeric_to_value(divide_numeric(value, magnitude)?)
}

fn float_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || arguments.len() > 2 {
        return Err(arity("float", "1 to 2", arguments.len()));
    }
    let number = number_argument("float", &arguments[0])?;
    if let Some(prototype) = arguments.get(1) {
        if !matches!(prototype, Value::Float(_)) {
            return Err(type_error("float", "a float prototype", prototype));
        }
    }
    Ok(Value::Float(number.as_float()))
}

fn rational(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rational", 1)?;
    match number_argument("rational", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rational_from_float(value),
    }
}

fn rational_from_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rational requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    const FRACTION_MASK: u64 = (1 << 52) - 1;
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let mut significand = bits & FRACTION_MASK;
    let mut exponent = if exponent_bits == 0 {
        -1074
    } else {
        significand |= 1 << 52;
        exponent_bits - 1023 - 52
    };

    if exponent < 0 {
        let canceled = significand.trailing_zeros().min((-exponent) as u32);
        significand >>= canceled;
        exponent += canceled as i32;
    }

    let mut numerator = i128::from(significand);
    if negative {
        numerator = -numerator;
    }
    let denominator = if exponent >= 0 {
        numerator = numerator
            .checked_shl(exponent as u32)
            .ok_or(RuntimeError::NumericOverflow)?;
        1
    } else {
        1i128
            .checked_shl((-exponent) as u32)
            .ok_or(RuntimeError::NumericOverflow)?
    };
    Value::rational(numerator, denominator)
}

fn rationalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rationalize", 1)?;
    match number_argument("rationalize", &arguments[0])? {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => rationalize_float(value),
    }
}

fn rationalize_float(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::InvalidForm {
            message: "rationalize requires a finite real".to_owned(),
            span: None,
        });
    }
    if value == 0.0 {
        return Ok(Value::Integer(0));
    }

    let tolerance = (value.abs() * f64::EPSILON / 2.0).max(f64::MIN_POSITIVE);
    let (numerator, denominator) = simplest_rational(value - tolerance, value + tolerance)?;
    number_to_value(rational_number(numerator, denominator)?)
}

fn simplest_rational(lower: f64, upper: f64) -> Result<(i128, i128), RuntimeError> {
    if !lower.is_finite() || !upper.is_finite() || lower > upper {
        return Err(RuntimeError::NumericOverflow);
    }
    if lower <= 0.0 && upper >= 0.0 {
        return Ok((0, 1));
    }
    if upper < 0.0 {
        let (numerator, denominator) = simplest_positive_rational(-upper, -lower, 0)?;
        return Ok((-numerator, denominator));
    }
    simplest_positive_rational(lower, upper, 0)
}

fn simplest_positive_rational(
    lower: f64,
    upper: f64,
    depth: u32,
) -> Result<(i128, i128), RuntimeError> {
    if depth > 128 || !lower.is_finite() || !upper.is_finite() || lower <= 0.0 || lower > upper {
        return Err(RuntimeError::NumericOverflow);
    }

    let lower_floor = lower.floor();
    let upper_floor = upper.floor();
    if lower == lower_floor {
        return Ok((lower_floor as i128, 1));
    }
    if lower_floor < upper_floor {
        return Ok(((lower_floor as i128) + 1, 1));
    }

    let lower_fraction = lower - lower_floor;
    let upper_fraction = upper - lower_floor;
    let (reciprocal_numerator, reciprocal_denominator) =
        simplest_positive_rational(1.0 / upper_fraction, 1.0 / lower_fraction, depth + 1)?;
    let numerator = (lower_floor as i128)
        .checked_mul(reciprocal_numerator)
        .and_then(|value| value.checked_add(reciprocal_denominator))
        .ok_or(RuntimeError::NumericOverflow)?;
    Ok((numerator, reciprocal_numerator))
}

fn exact_power(base: Number, exponent: i64) -> Result<Number, RuntimeError> {
    let (mut numerator, mut denominator) =
        base.exact_parts().expect("exact power received a float");
    let negative_exponent = exponent < 0;
    if negative_exponent && numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if negative_exponent {
        std::mem::swap(&mut numerator, &mut denominator);
    }

    let magnitude = exponent.unsigned_abs();
    rational_number(
        checked_power(i128::from(numerator), magnitude)?,
        checked_power(i128::from(denominator), magnitude)?,
    )
}

fn checked_power(base: i128, mut exponent: u64) -> Result<i128, RuntimeError> {
    let mut result = 1i128;
    let mut factor = base;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = result
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = factor
                .checked_mul(factor)
                .ok_or(RuntimeError::NumericOverflow)?;
        }
    }
    Ok(result)
}

fn numeric_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("=", "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| numeric_argument("=", value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(
        values
            .windows(2)
            .all(|window| numeric_equal_values(window[0], window[1])),
    ))
}

fn less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<", arguments, |ordering| ordering == Ordering::Less)
}

fn greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">", arguments, |ordering| ordering == Ordering::Greater)
}

fn less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers("<=", arguments, |ordering| ordering != Ordering::Greater)
}

fn greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_numbers(">=", arguments, |ordering| ordering != Ordering::Less)
}

fn zerop(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "zerop", 1)?;
    Ok(Value::boolean(
        match numeric_argument("zerop", &arguments[0])? {
            Numeric::Real(number) => number.as_float() == 0.0,
            Numeric::Complex { real, imag } => real.as_float() == 0.0 && imag.as_float() == 0.0,
        },
    ))
}

fn plusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "plusp", 1)?;
    Ok(Value::boolean(
        number_argument("plusp", &arguments[0])?.as_float() > 0.0,
    ))
}

fn minusp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "minusp", 1)?;
    Ok(Value::boolean(
        number_argument("minusp", &arguments[0])?.as_float() < 0.0,
    ))
}

fn evenp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "evenp", 1)?;
    Ok(Value::boolean(
        integer_argument("evenp", &arguments[0])? % 2 == 0,
    ))
}

fn oddp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "oddp", 1)?;
    Ok(Value::boolean(
        integer_argument("oddp", &arguments[0])? % 2 != 0,
    ))
}

fn minimum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "min", true)
}

fn maximum(arguments: &[Value]) -> Result<Value, RuntimeError> {
    extreme(arguments, "max", false)
}

fn extreme(
    arguments: &[Value],
    function: &str,
    choose_minimum: bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = values[0];
    for value in &values[1..] {
        let ordering = compare_number_values(*value, result);
        if (choose_minimum && ordering == Ordering::Less)
            || (!choose_minimum && ordering == Ordering::Greater)
        {
            result = *value;
        }
    }
    number_to_value(result)
}

fn absolute(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "abs", 1)?;
    match numeric_argument("abs", &arguments[0])? {
        Numeric::Real(number) => absolute_real(number),
        Numeric::Complex { real, imag } => absolute_complex(real, imag),
    }
}

fn absolute_real(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => value
            .checked_abs()
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => number_to_value(rational_number(
            i128::from(value.numerator()).abs(),
            i128::from(value.denominator()),
        )?),
        Number::Float(value) => Ok(Value::Float(value.abs())),
    }
}

fn absolute_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    let magnitude_squared =
        add_numbers(multiply_numbers(real, real)?, multiply_numbers(imag, imag)?)?;
    square_root_real(magnitude_squared)
}

fn compare_numbers(
    function: &str,
    arguments: &[Value],
    comparison: fn(Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity(function, "at least one", 0));
    }
    let values = arguments
        .iter()
        .map(|value| number_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(values.windows(2).all(|window| {
        comparison(compare_number_values(window[0], window[1]))
    })))
}

fn increment(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1+", 1)?;
    add(&[arguments[0].clone(), Value::Integer(1)])
}

fn decrement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "1-", 1)?;
    subtract(&[arguments[0].clone(), Value::Integer(1)])
}

#[derive(Clone, Copy)]
enum RoundingMode {
    Floor,
    Ceiling,
    Truncate,
    Round,
}

fn floor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "floor", RoundingMode::Floor)
}

fn ceiling(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "ceiling", RoundingMode::Ceiling)
}

fn truncate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "truncate", RoundingMode::Truncate)
}

fn round(arguments: &[Value]) -> Result<Value, RuntimeError> {
    quotient_and_remainder(arguments, "round", RoundingMode::Round)
}

fn quotient_and_remainder(
    arguments: &[Value],
    function: &str,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 2 {
        return Err(arity(function, "one or two", arguments.len()));
    }
    let dividend = number_argument(function, &arguments[0])?;
    let divisor = if arguments.len() == 2 {
        number_argument(function, &arguments[1])?
    } else {
        Number::Integer(1)
    };
    if dividend.is_float() || divisor.is_float() {
        float_quotient_and_remainder(dividend, divisor, mode)
    } else {
        exact_quotient_and_remainder(dividend, divisor, mode)
    }
}

fn exact_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let (dividend_numerator, dividend_denominator) = dividend
        .exact_parts()
        .expect("exact quotient received a float");
    let (divisor_numerator, divisor_denominator) = divisor
        .exact_parts()
        .expect("exact quotient received a float");
    if divisor_numerator == 0 {
        return Err(RuntimeError::DivisionByZero);
    }

    let dividend_numerator = i128::from(dividend_numerator);
    let dividend_denominator = i128::from(dividend_denominator);
    let divisor_numerator = i128::from(divisor_numerator);
    let divisor_denominator = i128::from(divisor_denominator);
    let mut quotient_numerator = dividend_numerator * divisor_denominator;
    let mut quotient_denominator = dividend_denominator * divisor_numerator;
    if quotient_denominator < 0 {
        quotient_numerator = -quotient_numerator;
        quotient_denominator = -quotient_denominator;
    }
    let truncated = quotient_numerator / quotient_denominator;
    let quotient =
        adjust_exact_quotient(truncated, quotient_numerator, quotient_denominator, mode)?;
    let quotient = i64::try_from(quotient).map_err(|_| RuntimeError::NumericOverflow)?;
    let remainder = rational_number(
        dividend_numerator * divisor_denominator
            - i128::from(quotient) * divisor_numerator * dividend_denominator,
        dividend_denominator * divisor_denominator,
    )?;
    Ok(Value::values(vec![
        Value::Integer(quotient),
        number_to_value(remainder)?,
    ]))
}

fn adjust_exact_quotient(
    truncated: i128,
    numerator: i128,
    denominator: i128,
    mode: RoundingMode,
) -> Result<i128, RuntimeError> {
    let remainder = numerator % denominator;
    if remainder == 0 {
        return Ok(truncated);
    }
    let direction = if numerator < 0 { -1 } else { 1 };
    match mode {
        RoundingMode::Truncate => Ok(truncated),
        RoundingMode::Floor if direction < 0 => truncated
            .checked_sub(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Ceiling if direction > 0 => truncated
            .checked_add(1)
            .ok_or(RuntimeError::NumericOverflow),
        RoundingMode::Round => {
            let distance = remainder.abs() * 2;
            if distance > denominator || (distance == denominator && truncated % 2 != 0) {
                truncated
                    .checked_add(direction)
                    .ok_or(RuntimeError::NumericOverflow)
            } else {
                Ok(truncated)
            }
        }
        _ => Ok(truncated),
    }
}

fn float_quotient_and_remainder(
    dividend: Number,
    divisor: Number,
    mode: RoundingMode,
) -> Result<Value, RuntimeError> {
    let dividend = dividend.as_float();
    let divisor = divisor.as_float();
    if divisor == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    let ratio = dividend / divisor;
    let rounded = match mode {
        RoundingMode::Floor => ratio.floor(),
        RoundingMode::Ceiling => ratio.ceil(),
        RoundingMode::Truncate => ratio.trunc(),
        RoundingMode::Round => round_float(ratio),
    };
    let quotient = float_integer(rounded)?;
    let remainder = Value::Float(dividend - quotient as f64 * divisor);
    Ok(Value::values(vec![Value::Integer(quotient), remainder]))
}

fn round_float(value: f64) -> f64 {
    let truncated = value.trunc();
    let fraction = (value - truncated).abs();
    if fraction > 0.5 || (fraction == 0.5 && truncated % 2.0 != 0.0) {
        truncated + value.signum()
    } else {
        truncated
    }
}

fn float_integer(value: f64) -> Result<i64, RuntimeError> {
    if !value.is_finite() || value < i64::MIN as f64 || value >= 9_223_372_036_854_775_808.0 {
        return Err(RuntimeError::NumericOverflow);
    }
    Ok(value as i64)
}

fn greatest_common_divisor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 0i128;
    for argument in arguments {
        result = integer_gcd(result, i128::from(integer_argument("gcd", argument)?));
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

fn least_common_multiple(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let mut result = 1i128;
    for argument in arguments {
        let value = i128::from(integer_argument("lcm", argument)?);
        if result == 0 || value == 0 {
            result = 0;
            continue;
        }
        let divisor = integer_gcd(result, value);
        result = (result / divisor)
            .checked_mul(value.abs())
            .ok_or(RuntimeError::NumericOverflow)?;
    }
    i64::try_from(result)
        .map(Value::Integer)
        .map_err(|_| RuntimeError::NumericOverflow)
}

fn integer_gcd(mut left: i128, mut right: i128) -> i128 {
    left = left.abs();
    right = right.abs();
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn numerator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "numerator", 1)?;
    match arguments[0] {
        Value::Integer(value) => Ok(Value::Integer(value)),
        Value::Rational(value) => Ok(Value::Integer(value.numerator())),
        ref value => Err(type_error("numerator", "rational", value)),
    }
}

fn denominator(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "denominator", 1)?;
    match arguments[0] {
        Value::Integer(_) => Ok(Value::Integer(1)),
        Value::Rational(value) => Ok(Value::Integer(value.denominator())),
        ref value => Err(type_error("denominator", "rational", value)),
    }
}

fn modulo(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mod", 2)?;
    let left = integer_argument("mod", &arguments[0])?;
    let right = integer_argument("mod", &arguments[1])?;
    let remainder = integer_remainder(left, right)?;
    if remainder != 0 && (left < 0) != (right < 0) {
        remainder
            .checked_add(right)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow)
    } else {
        Ok(Value::Integer(remainder))
    }
}

fn remainder(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rem", 2)?;
    let left = integer_argument("rem", &arguments[0])?;
    let right = integer_argument("rem", &arguments[1])?;
    integer_remainder(left, right).map(Value::Integer)
}

fn integer_remainder(left: i64, right: i64) -> Result<i64, RuntimeError> {
    if right == 0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left == i64::MIN && right == -1 {
        return Ok(0);
    }
    left.checked_rem(right).ok_or(RuntimeError::NumericOverflow)
}

fn arithmetic_shift(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ash", 2)?;
    let value = integer_argument("ash", &arguments[0])?;
    let count = integer_argument("ash", &arguments[1])?;
    if count >= 0 {
        if count >= 64 {
            return if value == 0 {
                Ok(Value::Integer(0))
            } else {
                Err(RuntimeError::NumericOverflow)
            };
        }
        return value
            .checked_shl(count as u32)
            .map(Value::Integer)
            .ok_or(RuntimeError::NumericOverflow);
    }

    let shift = if count == i64::MIN {
        u64::MAX
    } else {
        (-count) as u64
    };
    Ok(Value::Integer(if shift >= 64 {
        if value < 0 {
            -1
        } else {
            0
        }
    } else {
        value >> shift as u32
    }))
}

fn byte(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte", 2)?;
    let size = integer_argument("byte", &arguments[0])?;
    let position = integer_argument("byte", &arguments[1])?;
    validate_byte_bounds("byte", size, position)?;
    Ok(byte_spec_value(size, position))
}

fn byte_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte-size", 1)?;
    let (size, _) = parse_byte_spec("byte-size", &arguments[0])?;
    Ok(Value::Integer(i64::from(size)))
}

fn byte_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "byte-position", 1)?;
    let (_, position) = parse_byte_spec("byte-position", &arguments[0])?;
    Ok(Value::Integer(i64::from(position)))
}

fn ldb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb", 2)?;
    ldb_value("ldb", &arguments[0], &arguments[1])
}

fn ldb_test(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "ldb-test", 2)?;
    let (size, position) = parse_byte_spec("ldb-test", &arguments[0])?;
    let integer = integer_argument("ldb-test", &arguments[1])? as u64;
    Ok(Value::boolean(
        extract_byte_field(integer, size, position) != 0,
    ))
}

fn dpb(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "dpb", 3)?;
    dpb_value("dpb", &arguments[0], &arguments[1], &arguments[2])
}

fn mask_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "mask-field", 2)?;
    let (size, position) = parse_byte_spec("mask-field", &arguments[0])?;
    let integer = integer_argument("mask-field", &arguments[1])? as u64;
    Ok(Value::Integer((integer & byte_mask(size, position)) as i64))
}

fn deposit_field(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "deposit-field", 3)?;
    let (size, position) = parse_byte_spec("deposit-field", &arguments[1])?;
    let newbyte = integer_argument("deposit-field", &arguments[0])? as u64;
    let integer = integer_argument("deposit-field", &arguments[2])? as u64;
    let mask = byte_mask(size, position);
    Ok(Value::Integer(
        ((integer & !mask) | (newbyte & mask)) as i64,
    ))
}

pub(crate) fn ldb_value(
    function: &str,
    byte_spec: &Value,
    integer: &Value,
) -> Result<Value, RuntimeError> {
    let (size, position) = parse_byte_spec(function, byte_spec)?;
    let integer = integer_argument(function, integer)? as u64;
    let field = extract_byte_field(integer, size, position);
    let field = i64::try_from(field).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::Integer(field))
}

pub(crate) fn dpb_value(
    function: &str,
    newbyte: &Value,
    byte_spec: &Value,
    integer: &Value,
) -> Result<Value, RuntimeError> {
    let (size, position) = parse_byte_spec(function, byte_spec)?;
    let newbyte = integer_argument(function, newbyte)? as u64;
    let integer = integer_argument(function, integer)? as u64;
    let mask = byte_mask(size, position);
    let field = (newbyte << position) & mask;
    Ok(Value::Integer(((integer & !mask) | field) as i64))
}

fn logand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logand", -1, |left, right| left & right)
}

fn logior(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logior", 0, |left, right| left | right)
}

fn logxor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logxor", 0, |left, right| left ^ right)
}

fn lognand(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let value = arguments.iter().try_fold(-1_i64, |accumulator, argument| {
        Ok::<_, RuntimeError>(accumulator & integer_argument("lognand", argument)?)
    })?;
    Ok(Value::Integer(!value))
}

fn lognor(arguments: &[Value]) -> Result<Value, RuntimeError> {
    let value = arguments.iter().try_fold(0_i64, |accumulator, argument| {
        Ok::<_, RuntimeError>(accumulator | integer_argument("lognor", argument)?)
    })?;
    Ok(Value::Integer(!value))
}

fn logandc1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logandc1", 2)?;
    let left = integer_argument("logandc1", &arguments[0])?;
    let right = integer_argument("logandc1", &arguments[1])?;
    Ok(Value::Integer((!left) & right))
}

fn logandc2(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logandc2", 2)?;
    let left = integer_argument("logandc2", &arguments[0])?;
    let right = integer_argument("logandc2", &arguments[1])?;
    Ok(Value::Integer(left & (!right)))
}

fn logorc1(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logorc1", 2)?;
    let left = integer_argument("logorc1", &arguments[0])?;
    let right = integer_argument("logorc1", &arguments[1])?;
    Ok(Value::Integer((!left) | right))
}

fn logorc2(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logorc2", 2)?;
    let left = integer_argument("logorc2", &arguments[0])?;
    let right = integer_argument("logorc2", &arguments[1])?;
    Ok(Value::Integer(left | (!right)))
}

fn logeqv(arguments: &[Value]) -> Result<Value, RuntimeError> {
    bitwise(arguments, "logeqv", -1, |left, right| !(left ^ right))
}

fn boole(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "boole", 3)?;
    let operation = integer_argument("boole", &arguments[0])?;
    let left = integer_argument("boole", &arguments[1])?;
    let right = integer_argument("boole", &arguments[2])?;
    let value = match operation {
        0 => 0,
        1 => -1,
        2 => left,
        3 => right,
        4 => !left,
        5 => !right,
        6 => left & right,
        7 => left | right,
        8 => left ^ right,
        9 => !(left ^ right),
        10 => !(left & right),
        11 => !(left | right),
        12 => (!left) & right,
        13 => left & (!right),
        14 => (!left) | right,
        15 => left | (!right),
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: format!(
                    "boole operation must be an integer between 0 and 15, got {operation}"
                ),
                span: None,
            });
        }
    };
    Ok(Value::Integer(value))
}

fn logbitp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logbitp", 2)?;
    let index = integer_argument("logbitp", &arguments[0])?;
    validate_bit_index("logbitp", index)?;
    let integer = integer_argument("logbitp", &arguments[1])? as u64;
    Ok(Value::boolean(((integer >> index as u32) & 1) != 0))
}

fn bitwise(
    arguments: &[Value],
    function: &str,
    identity: i64,
    operation: fn(i64, i64) -> i64,
) -> Result<Value, RuntimeError> {
    let mut result = identity;
    for argument in arguments {
        result = operation(result, integer_argument(function, argument)?);
    }
    Ok(Value::Integer(result))
}

fn lognot(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "lognot", 1)?;
    Ok(Value::Integer(!integer_argument("lognot", &arguments[0])?))
}

fn logtest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logtest", 2)?;
    let left = integer_argument("logtest", &arguments[0])?;
    let right = integer_argument("logtest", &arguments[1])?;
    Ok(Value::boolean((left & right) != 0))
}

fn logcount(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "logcount", 1)?;
    let value = integer_argument("logcount", &arguments[0])?;
    let count = if value < 0 {
        (!value).count_ones()
    } else {
        value.count_ones()
    };
    Ok(Value::Integer(count as i64))
}

fn integer_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "integer-length", 1)?;
    let value = integer_argument("integer-length", &arguments[0])?;
    let magnitude = if value < 0 { !value } else { value } as u64;
    Ok(Value::Integer((64 - magnitude.leading_zeros()) as i64))
}

fn parse_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() || (arguments.len() - 1) % 2 != 0 {
        return Err(arity(
            "parse-integer",
            "a string and keyword/value pairs",
            arguments.len(),
        ));
    }
    let chars = match &arguments[0] {
        Value::String(value) => value.as_ref().chars().collect::<Vec<_>>(),
        value => return Err(type_error("parse-integer", "a string", value)),
    };
    let mut start = 0;
    let mut end = chars.len();
    let mut radix = 10_i64;
    let mut junk_allowed = false;
    for pair in arguments[1..].chunks_exact(2) {
        match array_option_name("parse-integer", &pair[0])?.as_str() {
            "START" => start = index_argument("parse-integer", &pair[1])?,
            "END" => end = index_argument("parse-integer", &pair[1])?,
            "RADIX" => radix = integer_argument("parse-integer", &pair[1])?,
            "JUNK-ALLOWED" => junk_allowed = pair[1].is_truthy(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("parse-integer does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > chars.len() {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer bounds are invalid".to_string(),
            span: None,
        });
    }
    if !(2..=36).contains(&radix) {
        return Err(RuntimeError::InvalidForm {
            message: format!("parse-integer radix must be between 2 and 36, got {radix}"),
            span: None,
        });
    }
    let radix = u32::try_from(radix).expect("parse-integer radix was checked");
    let mut cursor = start;
    while cursor < end && chars[cursor].is_whitespace() {
        cursor += 1;
    }
    let negative = match chars.get(cursor) {
        Some('+') => {
            cursor += 1;
            false
        }
        Some('-') => {
            cursor += 1;
            true
        }
        _ => false,
    };
    let digits_start = cursor;
    let mut magnitude = 0_i128;
    while cursor < end {
        let Some(digit) = parse_integer_digit(chars[cursor]) else {
            break;
        };
        if digit >= radix {
            break;
        }
        magnitude = magnitude
            .checked_mul(i128::from(radix))
            .and_then(|value| value.checked_add(i128::from(digit)))
            .ok_or(RuntimeError::NumericOverflow)?;
        cursor += 1;
    }
    if cursor == digits_start {
        if junk_allowed {
            let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
            return Ok(Value::values(vec![Value::Nil, Value::Integer(position)]));
        }
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found no integer".to_string(),
            span: None,
        });
    }
    let signed = if negative {
        magnitude
            .checked_neg()
            .ok_or(RuntimeError::NumericOverflow)?
    } else {
        magnitude
    };
    let integer = i64::try_from(signed).map_err(|_| RuntimeError::NumericOverflow)?;
    if junk_allowed {
        let position = i64::try_from(cursor).map_err(|_| RuntimeError::NumericOverflow)?;
        return Ok(Value::values(vec![
            Value::Integer(integer),
            Value::Integer(position),
        ]));
    }
    let mut trailing = cursor;
    while trailing < end && chars[trailing].is_whitespace() {
        trailing += 1;
    }
    if trailing != end {
        return Err(RuntimeError::InvalidForm {
            message: "parse-integer found junk after the integer".to_string(),
            span: None,
        });
    }
    let position = i64::try_from(end).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::values(vec![
        Value::Integer(integer),
        Value::Integer(position),
    ]))
}

fn parse_integer_digit(character: char) -> Option<u32> {
    match character {
        '0'..='9' => Some(u32::from(character as u8 - b'0')),
        'A'..='Z' => Some(u32::from(character as u8 - b'A') + 10),
        'a'..='z' => Some(u32::from(character as u8 - b'a') + 10),
        _ => None,
    }
}

fn list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::list(arguments.to_vec()))
}

fn list_star(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("list*", "at least one", 0));
    }
    if arguments.len() == 1 {
        return Ok(arguments[0].clone());
    }

    let mut values = arguments[..arguments.len() - 1].to_vec();
    match arguments.last().expect("arguments is non-empty") {
        Value::Nil | Value::List(_) => {
            let Some(items) = arguments.last().and_then(Value::list_items) else {
                unreachable!();
            };
            values.extend(items);
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        tail => Ok(Value::dotted_list(values, tail.clone())),
    }
}

fn make_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-list", "at least one", 0));
    }
    if (arguments.len() - 1) % 2 != 0 {
        return Err(arity(
            "make-list",
            "a size and keyword/value pairs",
            arguments.len(),
        ));
    }

    let size = index_argument("make-list", &arguments[0])?;
    let mut initial_element = Value::Nil;
    for pair in arguments[1..].chunks_exact(2) {
        match array_option_name("make-list", &pair[0])?.as_str() {
            "INITIAL-ELEMENT" => initial_element = pair[1].clone(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-list does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    Ok(Value::list(vec![initial_element; size]))
}

fn values_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "values-list", 1)?;
    let Some(values) = arguments[0].list_items() else {
        return Err(type_error("values-list", "list", &arguments[0]));
    };
    Ok(Value::values(values))
}

fn list_length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "list-length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) => items.len(),
        value => return Err(type_error("list-length", "proper list", value)),
    };
    Ok(Value::Integer(length as i64))
}

fn nthcdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nthcdr", 2)?;
    let index = index_argument("nthcdr", &arguments[0])?;
    match &arguments[1] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items.iter().skip(index).cloned().collect())),
        Value::DottedList { items, tail } if index < items.len() => Ok(Value::dotted_list(
            items.iter().skip(index).cloned().collect(),
            tail.as_ref().clone(),
        )),
        Value::DottedList { items, tail } if index == items.len() => Ok(tail.as_ref().clone()),
        value @ Value::DottedList { .. } => Err(type_error("nthcdr", "proper list", value)),
        value => Err(type_error("nthcdr", "list", value)),
    }
}

fn acons(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "acons", 3)?;
    let Some(alist) = arguments[2].list_items() else {
        return Err(type_error("acons", "list", &arguments[2]));
    };
    let mut result = Vec::with_capacity(alist.len() + 1);
    result.push(Value::dotted_list(
        vec![arguments[0].clone()],
        arguments[1].clone(),
    ));
    result.extend(alist);
    Ok(Value::list(result))
}

fn pairlis(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("pairlis", "2 or 3", arguments.len()));
    }
    let Some(keys) = arguments[0].list_items() else {
        return Err(type_error("pairlis", "list", &arguments[0]));
    };
    let Some(values) = arguments[1].list_items() else {
        return Err(type_error("pairlis", "list", &arguments[1]));
    };
    if keys.len() != values.len() {
        return Err(RuntimeError::InvalidForm {
            message: "pairlis requires lists of equal length".to_string(),
            span: None,
        });
    }
    let mut result = match arguments.get(2) {
        Some(alist) => alist
            .list_items()
            .ok_or_else(|| type_error("pairlis", "list", alist))?,
        None => Vec::new(),
    };
    for (key, value) in keys.into_iter().zip(values) {
        result.insert(0, Value::dotted_list(vec![key], value));
    }
    Ok(Value::list(result))
}

fn cons(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cons", 2)?;
    match &arguments[1] {
        Value::Nil => Ok(Value::list(vec![arguments[0].clone()])),
        Value::List(items) => {
            let mut values = Vec::with_capacity(items.len() + 1);
            values.push(arguments[0].clone());
            values.extend(items.iter().cloned());
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            let mut values = Vec::with_capacity(items.len() + 1);
            values.push(arguments[0].clone());
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        _ => Ok(Value::dotted_list(
            vec![arguments[0].clone()],
            arguments[1].clone(),
        )),
    }
}

fn car(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "car", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => items
            .first()
            .cloned()
            .ok_or_else(|| type_error("car", "non-empty list", &arguments[0])),
        Value::DottedList { items, .. } => items
            .first()
            .cloned()
            .ok_or_else(|| type_error("car", "non-empty list", &arguments[0])),
        value => Err(type_error("car", "list", value)),
    }
}

fn cdr(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "cdr", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items.iter().skip(1).cloned().collect())),
        Value::DottedList { items, tail } if items.len() > 1 => Ok(Value::dotted_list(
            items.iter().skip(1).cloned().collect(),
            tail.as_ref().clone(),
        )),
        Value::DottedList { tail, .. } => Ok(tail.as_ref().clone()),
        value => Err(type_error("cdr", "list", value)),
    }
}

fn first(arguments: &[Value]) -> Result<Value, RuntimeError> {
    car(arguments)
}

fn second(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("second", 1, arguments)
}

fn third(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("third", 2, arguments)
}

fn fourth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("fourth", 3, arguments)
}

fn fifth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("fifth", 4, arguments)
}

fn sixth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("sixth", 5, arguments)
}

fn seventh(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("seventh", 6, arguments)
}

fn eighth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("eighth", 7, arguments)
}

fn ninth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("ninth", 8, arguments)
}

fn tenth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    nth_accessor("tenth", 9, arguments)
}

fn rest(arguments: &[Value]) -> Result<Value, RuntimeError> {
    cdr(arguments)
}

fn nth_accessor(function: &str, index: usize, arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, function, 1)?;
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    Ok(items.get(index).cloned().unwrap_or(Value::Nil))
}

fn append(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("append", arguments)
}

fn append_lists(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Ok(Value::Nil);
    }
    let mut values = Vec::new();
    for argument in &arguments[..arguments.len() - 1] {
        let Some(items) = argument.list_items() else {
            return Err(type_error(function, "list", argument));
        };
        values.extend(items);
    }
    match arguments.last().expect("arguments is non-empty") {
        Value::Nil | Value::List(_) => {
            let Some(items) = arguments.last().and_then(Value::list_items) else {
                unreachable!();
            };
            values.extend(items);
            Ok(Value::list(values))
        }
        Value::DottedList { items, tail } => {
            if values.is_empty() && items.is_empty() {
                return Ok(arguments.last().expect("arguments is non-empty").clone());
            }
            values.extend(items.iter().cloned());
            Ok(Value::dotted_list(values, tail.as_ref().clone()))
        }
        tail if values.is_empty() => Ok(tail.clone()),
        tail => Ok(Value::dotted_list(values, tail.clone())),
    }
}

fn nconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    append_lists("nconc", arguments)
}

fn revappend(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("revappend", arguments)
}

fn nreconc(arguments: &[Value]) -> Result<Value, RuntimeError> {
    revappend_like("nreconc", arguments)
}

fn revappend_like(function: &str, arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let Some(mut items) = arguments[0].list_items() else {
        return Err(type_error(function, "list", &arguments[0]));
    };
    items.reverse();
    let append_arguments = [Value::list(items), arguments[1].clone()];
    append_lists(function, &append_arguments)
}

fn length(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "length", 1)?;
    let length = match &arguments[0] {
        Value::Nil => 0,
        Value::List(items) => items.len(),
        value if value.vector_items().is_some() => {
            value.vector_items().expect("vector has vector items").len()
        }
        Value::String(value) => value.chars().count(),
        _ => {
            return Err(type_error("length", "sequence", &arguments[0]));
        }
    };
    Ok(Value::Integer(length as i64))
}

fn nth(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nth", 2)?;
    let Some(items) = arguments[1].list_items() else {
        return Err(type_error("nth", "list", &arguments[1]));
    };
    let index = index_argument("nth", &arguments[0])?;
    Ok(items.get(index).cloned().unwrap_or(Value::Nil))
}

fn elt(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "elt", 2)?;
    let index = index_argument("elt", &arguments[1])?;
    match &arguments[0] {
        Value::Nil => Err(out_of_bounds("elt", index)),
        Value::List(items) => items
            .get(index)
            .cloned()
            .ok_or_else(|| out_of_bounds("elt", index)),
        Value::String(value) => value
            .chars()
            .nth(index)
            .map(Value::Character)
            .ok_or_else(|| out_of_bounds("elt", index)),
        value => value
            .vector_items()
            .and_then(|items| items.get(index).cloned())
            .ok_or_else(|| {
                if value.vector_items().is_some() {
                    out_of_bounds("elt", index)
                } else {
                    type_error("elt", "sequence", value)
                }
            }),
    }
}

fn string_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "string", 1)?;
    Ok(Value::string(string_designator("string", &arguments[0])?))
}

fn validate_make_string_element_type(value: &Value) -> Result<(), RuntimeError> {
    let element_type = type_designator_name("make-string", value)?;
    match element_type.as_str() {
        "CHARACTER" | "BASE-CHAR" | "STANDARD-CHAR" | "EXTENDED-CHAR" => Ok(()),
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "make-string :element-type must be a character type, got {element_type}"
            ),
            span: None,
        }),
    }
}

fn make_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-string", "at least 1", 0));
    }
    let length = index_argument("make-string", &arguments[0])?;
    let mut initial = ' ';
    match arguments.get(1) {
        None => {}
        Some(value)
            if arguments.len() == 2
                && !matches!(value, Value::Keyword(_) | Value::KeywordExact(_)) =>
        {
            initial = character_argument("make-string", value)?;
        }
        Some(_) => {
            if (arguments.len() - 1) % 2 != 0 {
                return Err(arity(
                    "make-string",
                    "a size and keyword/value pairs",
                    arguments.len(),
                ));
            }
            for pair in arguments[1..].chunks_exact(2) {
                match array_option_name("make-string", &pair[0])?.as_str() {
                    "INITIAL-ELEMENT" => {
                        initial = character_argument("make-string", &pair[1])?;
                    }
                    "ELEMENT-TYPE" => validate_make_string_element_type(&pair[1])?,
                    option => {
                        return Err(RuntimeError::InvalidForm {
                            message: format!("make-string does not accept :{option}"),
                            span: None,
                        });
                    }
                }
            }
        }
    }
    Ok(Value::string(
        std::iter::repeat(initial).take(length).collect::<String>(),
    ))
}

fn character_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "character", 1)?;
    Ok(Value::Character(character_designator(
        "character",
        &arguments[0],
    )?))
}

fn character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char", 2)?;
    let index = index_argument("char", &arguments[1])?;
    let Value::String(value) = &arguments[0] else {
        return Err(type_error("char", "string", &arguments[0]));
    };
    value
        .chars()
        .nth(index)
        .map(Value::Character)
        .ok_or_else(|| out_of_bounds("char", index))
}

fn simple_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "schar", 2)?;
    let index = index_argument("schar", &arguments[1])?;
    let Value::String(value) = &arguments[0] else {
        return Err(type_error("schar", "simple-string", &arguments[0]));
    };
    value
        .chars()
        .nth(index)
        .map(Value::Character)
        .ok_or_else(|| out_of_bounds("schar", index))
}

fn char_code(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-code", 1)?;
    Ok(Value::Integer(
        character_argument("char-code", &arguments[0])? as i64,
    ))
}

fn char_int(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-int", 1)?;
    Ok(Value::Integer(
        character_argument("char-int", &arguments[0])? as i64,
    ))
}

fn code_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "code-char", 1)?;
    code_char_value("code-char", &arguments[0])
}

fn int_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "int-char", 1)?;
    code_char_value("int-char", &arguments[0])
}

fn code_char_value(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let code = integer_argument(function, value)?;
    Ok(u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .map(Value::Character)
        .unwrap_or(Value::Nil))
}

fn character_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char=", arguments, false, |left, right| left == right)
}

fn character_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char/=", arguments, false)
}

fn character_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-equal", arguments, true, |left, right| left == right)
}

fn character_case_not_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters_distinct("char-not-equal", arguments, true)
}

fn character_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<", arguments, false, |left, right| left < right)
}

fn character_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>", arguments, false, |left, right| left > right)
}

fn character_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char<=", arguments, false, |left, right| left <= right)
}

fn character_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char>=", arguments, false, |left, right| left >= right)
}

fn character_case_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-lessp", arguments, true, |left, right| left < right)
}

fn character_case_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-greaterp", arguments, true, |left, right| left > right)
}

fn character_case_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-greaterp", arguments, true, |left, right| {
        left <= right
    })
}

fn character_case_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_characters("char-not-lessp", arguments, true, |left, right| {
        left >= right
    })
}

fn compare_characters(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
    comparison: fn(char, char) -> bool,
) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity(function, "at least 2", arguments.len()));
    }
    let characters = arguments
        .iter()
        .map(|value| character_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::boolean(characters.windows(2).all(|window| {
        let left = if ignore_case {
            window[0].to_ascii_lowercase()
        } else {
            window[0]
        };
        let right = if ignore_case {
            window[1].to_ascii_lowercase()
        } else {
            window[1]
        };
        comparison(left, right)
    })))
}

fn compare_characters_distinct(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity(function, "at least 2", arguments.len()));
    }
    let characters = arguments
        .iter()
        .map(|value| character_argument(function, value))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, left) in characters.iter().enumerate() {
        for right in characters.iter().skip(index + 1) {
            let left = if ignore_case {
                left.to_ascii_lowercase()
            } else {
                *left
            };
            let right = if ignore_case {
                right.to_ascii_lowercase()
            } else {
                *right
            };
            if left == right {
                return Ok(Value::Nil);
            }
        }
    }
    Ok(Value::boolean(true))
}

fn character_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-upcase", 1)?;
    Ok(Value::Character(
        character_argument("char-upcase", &arguments[0])?.to_ascii_uppercase(),
    ))
}

fn character_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-downcase", 1)?;
    Ok(Value::Character(
        character_argument("char-downcase", &arguments[0])?.to_ascii_lowercase(),
    ))
}

fn alpha_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alpha-char-p", arguments, |character| {
        character.is_alphabetic()
    })
}

fn alphanumeric_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("alphanumericp", arguments, |character| {
        character.is_alphanumeric()
    })
}

fn graphic_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("graphic-char-p", arguments, |character| {
        !character.is_control()
    })
}

fn standard_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("standard-char-p", arguments, |character| {
        character == '\n' || character == ' ' || character.is_ascii_graphic()
    })
}

fn upper_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("upper-case-p", arguments, char::is_uppercase)
}

fn lower_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("lower-case-p", arguments, char::is_lowercase)
}

fn both_case_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    character_predicate("both-case-p", arguments, |character| {
        character.is_uppercase() || character.is_lowercase()
    })
}

fn character_predicate(
    function: &str,
    arguments: &[Value],
    predicate: impl Fn(char) -> bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 1)?;
    Ok(Value::boolean(predicate(character_argument(
        function,
        &arguments[0],
    )?)))
}

fn digit_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("digit-char", "1 or 2", arguments.len()));
    }
    let weight = integer_argument("digit-char", &arguments[0])?;
    let radix = radix_argument("digit-char", arguments, 1)?;
    if weight < 0 || weight >= i64::from(radix) {
        return Ok(Value::Nil);
    }
    let digit = weight as u32;
    let character = if digit < 10 {
        (b'0' + digit as u8) as char
    } else {
        (b'A' + (digit - 10) as u8) as char
    };
    Ok(Value::Character(character))
}

fn digit_character_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("digit-char-p", "1 or 2", arguments.len()));
    }
    let character = character_argument("digit-char-p", &arguments[0])?;
    let radix = radix_argument("digit-char-p", arguments, 1)?;
    let digit = match character {
        '0'..='9' => Some(character as u32 - '0' as u32),
        'A'..='Z' => Some(character as u32 - 'A' as u32 + 10),
        'a'..='z' => Some(character as u32 - 'a' as u32 + 10),
        _ => None,
    };
    match digit {
        Some(digit) if digit < radix => Ok(Value::Integer(i64::from(digit))),
        _ => Ok(Value::Nil),
    }
}

fn radix_argument(function: &str, arguments: &[Value], index: usize) -> Result<u32, RuntimeError> {
    let radix = arguments
        .get(index)
        .map(|value| integer_argument(function, value))
        .transpose()?
        .unwrap_or(10);
    if !(2..=36).contains(&radix) {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} radix must be between 2 and 36"),
            span: None,
        });
    }
    Ok(radix as u32)
}

fn character_name(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "char-name", 1)?;
    Ok(
        named_character_name(character_argument("char-name", &arguments[0])?)
            .map(Value::string)
            .unwrap_or(Value::Nil),
    )
}

fn name_character(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "name-char", 1)?;
    let name = string_designator("name-char", &arguments[0])?;
    if let Some(character) = named_character_from_name(&name) {
        return Ok(Value::Character(character));
    }
    let mut characters = name.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => Ok(Value::Character(character)),
        _ => Ok(Value::Nil),
    }
}

fn named_character_name(character: char) -> Option<&'static str> {
    match character {
        '\0' => Some("Null"),
        '\x07' => Some("Bell"),
        '\x08' => Some("Backspace"),
        '\t' => Some("Tab"),
        '\n' => Some("Newline"),
        '\x0c' => Some("Page"),
        '\r' => Some("Return"),
        ' ' => Some("Space"),
        '\x7f' => Some("Rubout"),
        _ => None,
    }
}

fn named_character_from_name(name: &str) -> Option<char> {
    match name.to_ascii_uppercase().as_str() {
        "NULL" | "NUL" => Some('\0'),
        "BELL" => Some('\x07'),
        "BACKSPACE" | "BS" => Some('\x08'),
        "TAB" => Some('\t'),
        "NEWLINE" | "LINEFEED" | "LF" => Some('\n'),
        "PAGE" | "FORMFEED" | "FF" => Some('\x0c'),
        "RETURN" | "CR" => Some('\r'),
        "SPACE" => Some(' '),
        "RUBOUT" | "DELETE" | "DEL" => Some('\x7f'),
        _ => None,
    }
}

fn string_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string=", arguments, false)
}

fn string_case_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_equality("string-equal", arguments, true)
}

fn string_less_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<", arguments, false, |ordering| {
        ordering == Ordering::Less
    })
}

fn string_greater_than(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>", arguments, false, |ordering| {
        ordering == Ordering::Greater
    })
}

fn string_less_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string<=", arguments, false, |ordering| {
        ordering != Ordering::Greater
    })
}

fn string_greater_equal(arguments: &[Value]) -> Result<Value, RuntimeError> {
    compare_strings("string>=", arguments, false, |ordering| {
        ordering != Ordering::Less
    })
}

fn compare_strings(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
    comparison: fn(Ordering) -> bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = string_designator(function, &arguments[0])?;
    let right = string_designator(function, &arguments[1])?;
    let (index, ordering) = string_order(&left, &right, ignore_case);
    if comparison(ordering) {
        Ok(Value::Integer(index as i64))
    } else {
        Ok(Value::Nil)
    }
}

fn string_equality(
    function: &str,
    arguments: &[Value],
    ignore_case: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let left = string_designator(function, &arguments[0])?;
    let right = string_designator(function, &arguments[1])?;
    let (_, ordering) = string_order(&left, &right, ignore_case);
    Ok(Value::boolean(ordering == Ordering::Equal))
}

fn string_order(left: &str, right: &str, ignore_case: bool) -> (usize, Ordering) {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    for (index, (left, right)) in left.iter().zip(&right).enumerate() {
        let left = if ignore_case {
            left.to_ascii_lowercase()
        } else {
            *left
        };
        let right = if ignore_case {
            right.to_ascii_lowercase()
        } else {
            *right
        };
        if left != right {
            return (index, left.cmp(&right));
        }
    }
    (left.len().min(right.len()), left.len().cmp(&right.len()))
}

fn string_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-upcase", StringCase::Upper)
}

fn string_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-downcase", StringCase::Lower)
}

fn string_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "string-capitalize", StringCase::Capitalize)
}

fn nstring_upcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-upcase", StringCase::Upper)
}

fn nstring_downcase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-downcase", StringCase::Lower)
}

fn nstring_capitalize(arguments: &[Value]) -> Result<Value, RuntimeError> {
    string_case_transform(arguments, "nstring-capitalize", StringCase::Capitalize)
}

#[derive(Clone, Copy)]
enum StringCase {
    Upper,
    Lower,
    Capitalize,
}

fn string_case_transform(
    arguments: &[Value],
    function: &str,
    case: StringCase,
) -> Result<Value, RuntimeError> {
    if !(1..=5).contains(&arguments.len()) || (arguments.len() - 1) % 2 != 0 {
        return Err(arity(function, "1, 3, or 5", arguments.len()));
    }
    let value = string_designator(function, &arguments[0])?;
    let characters = value.chars().collect::<Vec<_>>();
    let (start, end) = sequence_bounds(function, characters.len(), &arguments[1..])?;
    let mut output = String::new();
    let mut word_start = true;
    for (index, character) in characters.into_iter().enumerate() {
        let in_range = (start..end).contains(&index);
        match case {
            StringCase::Upper if in_range => output.extend(character.to_uppercase()),
            StringCase::Lower if in_range => output.extend(character.to_lowercase()),
            StringCase::Capitalize if character.is_alphanumeric() => {
                if in_range && word_start {
                    output.extend(character.to_uppercase());
                } else if in_range {
                    output.extend(character.to_lowercase());
                } else {
                    output.push(character);
                }
                word_start = false;
            }
            StringCase::Capitalize => {
                output.push(character);
                word_start = true;
            }
            _ => output.push(character),
        }
    }
    Ok(Value::string(output))
}

fn string_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-trim", true, true)
}

fn string_left_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-left-trim", true, false)
}

fn string_right_trim(arguments: &[Value]) -> Result<Value, RuntimeError> {
    trim_string(arguments, "string-right-trim", false, true)
}

fn trim_string(
    arguments: &[Value],
    function: &str,
    trim_left: bool,
    trim_right: bool,
) -> Result<Value, RuntimeError> {
    exact(arguments, function, 2)?;
    let trim_set = sequence_elements(function, &arguments[0])?
        .into_iter()
        .map(|value| character_argument(function, &value))
        .collect::<Result<Vec<_>, _>>()?;
    let value = string_designator(function, &arguments[1])?;
    let characters = value.chars().collect::<Vec<_>>();
    let is_trimmed = |character: &char| trim_set.contains(character);
    let start = if trim_left {
        characters
            .iter()
            .position(|character| !is_trimmed(character))
    } else {
        Some(0)
    }
    .unwrap_or(characters.len());
    let end = if trim_right {
        characters
            .iter()
            .rposition(|character| !is_trimmed(character))
            .map_or(0, |index| index + 1)
    } else {
        characters.len()
    };
    Ok(Value::string(
        characters[start.min(end)..end].iter().collect::<String>(),
    ))
}

fn character_argument(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        value => Err(type_error(function, "character", value)),
    }
}

fn character_designator(function: &str, value: &Value) -> Result<char, RuntimeError> {
    match value {
        Value::Character(value) => Ok(*value),
        Value::String(_)
        | Value::Symbol(_)
        | Value::UninternedSymbol(_)
        | Value::Keyword(_)
        | Value::SymbolExact(_)
        | Value::KeywordExact(_) => {
            let string = string_designator(function, value)?;
            let mut characters = string.chars();
            match (characters.next(), characters.next()) {
                (Some(character), None) => Ok(character),
                _ => Err(type_error(function, "character designator", value)),
            }
        }
        value => Err(type_error(function, "character designator", value)),
    }
}

fn string_designator(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Nil => Ok("NIL".to_string()),
        Value::Boolean(true) => Ok("T".to_string()),
        Value::Boolean(false) => Ok("NIL".to_string()),
        Value::String(value)
        | Value::Symbol(value)
        | Value::UninternedSymbol(value)
        | Value::Keyword(value)
        | Value::SymbolExact(value)
        | Value::KeywordExact(value) => Ok(value.to_string()),
        Value::Character(value) => Ok(value.to_string()),
        value => Err(type_error(function, "string designator", value)),
    }
}

fn subseq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("subseq", "2 or 3", arguments.len()));
    }
    let length = sequence_length(&arguments[0])
        .ok_or_else(|| type_error("subseq", "sequence", &arguments[0]))?;
    let start = index_argument("subseq", &arguments[1])?;
    let end = arguments
        .get(2)
        .map(|value| index_argument("subseq", value))
        .transpose()?
        .unwrap_or(length);
    if start > end || end > length {
        return Err(RuntimeError::InvalidForm {
            message: "subseq bounds are invalid".to_string(),
            span: None,
        });
    }
    match &arguments[0] {
        Value::Nil => Ok(Value::Nil),
        Value::List(items) => Ok(Value::list(items[start..end].to_vec())),
        Value::Vector {
            fill_pointer,
            element_type,
            adjustable,
            ..
        } => {
            let items = arguments[0].vector_items().expect("vector items");
            let slice = items[start..end].to_vec();
            Ok(Value::vector_with_fill_pointer_element_type_and_adjustable(
                slice,
                *fill_pointer,
                element_type.as_ref().clone(),
                *adjustable,
            ))
        }
        Value::String(value) => {
            let result = value
                .chars()
                .skip(start)
                .take(end - start)
                .collect::<String>();
            Ok(Value::string(result))
        }
        _ => Err(type_error("subseq", "sequence", &arguments[0])),
    }
}

fn fill(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("fill", "at least two", arguments.len()));
    }
    if (arguments.len() - 2) % 2 != 0 {
        return Err(arity(
            "fill",
            "an item, a sequence, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let length = sequence_length(&arguments[1])
        .ok_or_else(|| type_error("fill", "sequence", &arguments[1]))?;
    let (start, end) = sequence_bounds("fill", length, &arguments[2..])?;
    if matches!(arguments[1], Value::String(_)) && !matches!(arguments[0], Value::Character(_)) {
        return Err(type_error(
            "fill",
            "a character for a string",
            &arguments[0],
        ));
    }
    let mut items = sequence_elements("fill", &arguments[1])?;
    for item in &mut items[start..end] {
        *item = arguments[0].clone();
    }
    rebuild_sequence("fill", &arguments[1], items)
}

fn replace(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("replace", "at least two", arguments.len()));
    }
    if (arguments.len() - 2) % 2 != 0 {
        return Err(arity(
            "replace",
            "two sequences and keyword/value pairs",
            arguments.len(),
        ));
    }
    let first_length = sequence_length(&arguments[0])
        .ok_or_else(|| type_error("replace", "sequence", &arguments[0]))?;
    let second_length = sequence_length(&arguments[1])
        .ok_or_else(|| type_error("replace", "sequence", &arguments[1]))?;
    let (start1, end1, start2, end2) =
        replace_bounds(first_length, second_length, &arguments[2..])?;
    let mut result = sequence_elements("replace", &arguments[0])?;
    let source = sequence_elements("replace", &arguments[1])?;
    let count = (end1 - start1).min(end2 - start2);
    if matches!(arguments[0], Value::String(_))
        && source[start2..start2 + count]
            .iter()
            .any(|value| !matches!(value, Value::Character(_)))
    {
        return Err(type_error(
            "replace",
            "characters in the source sequence for a string destination",
            &arguments[1],
        ));
    }
    for offset in 0..count {
        result[start1 + offset] = source[start2 + offset].clone();
    }
    rebuild_sequence("replace", &arguments[0], result)
}

fn copy_seq(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "copy-seq", 1)?;
    let items = sequence_elements("copy-seq", &arguments[0])?;
    rebuild_sequence("copy-seq", &arguments[0], items)
}

fn concatenate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("concatenate", "at least one", 0));
    }
    let result_type = type_designator_name("concatenate", &arguments[0])?;
    let mut items = Vec::new();
    for sequence in &arguments[1..] {
        items.extend(sequence_elements("concatenate", sequence)?);
    }
    match result_type.as_str() {
        "LIST" => Ok(Value::list(items)),
        "VECTOR" => Ok(Value::vector(items)),
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
            let mut result = String::new();
            for item in items {
                let Value::Character(character) = item else {
                    return Err(type_error(
                        "concatenate",
                        "characters for a string result",
                        &item,
                    ));
                };
                result.push(character);
            }
            Ok(Value::string(result))
        }
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "concatenate result type must be LIST, VECTOR, or STRING, got {result_type}"
            ),
            span: None,
        }),
    }
}

fn make_sequence(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 || (arguments.len() - 2) % 2 != 0 {
        return Err(arity(
            "make-sequence",
            "a result type, a size, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let result_type = type_designator_name("make-sequence", &arguments[0])?;
    let size = index_argument("make-sequence", &arguments[1])?;
    let mut initial_element = Value::Nil;
    for pair in arguments[2..].chunks_exact(2) {
        match array_option_name("make-sequence", &pair[0])?.as_str() {
            "INITIAL-ELEMENT" => initial_element = pair[1].clone(),
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-sequence does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    match result_type.as_str() {
        "LIST" => Ok(Value::list(vec![initial_element; size])),
        "VECTOR" | "SIMPLE-VECTOR" => Ok(Value::vector(vec![initial_element; size])),
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
            let initial = character_argument("make-sequence", &initial_element)?;
            Ok(Value::string(
                std::iter::repeat(initial).take(size).collect::<String>(),
            ))
        }
        _ => Err(RuntimeError::InvalidForm {
            message: format!(
                "make-sequence result type must be LIST, VECTOR, or STRING, got {result_type}"
            ),
            span: None,
        }),
    }
}

fn coerce(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "coerce", 2)?;
    let result_type = type_designator_name("coerce", &arguments[1])?;
    match result_type.as_str() {
        "LIST" => Ok(Value::list(sequence_elements("coerce", &arguments[0])?)),
        "VECTOR" | "SIMPLE-VECTOR" => {
            Ok(Value::vector(sequence_elements("coerce", &arguments[0])?))
        }
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
            let result = match &arguments[0] {
                Value::Nil
                | Value::Boolean(_)
                | Value::String(_)
                | Value::Symbol(_)
                | Value::UninternedSymbol(_)
                | Value::Keyword(_)
                | Value::SymbolExact(_)
                | Value::KeywordExact(_)
                | Value::Character(_) => string_designator("coerce", &arguments[0])?,
                value => sequence_elements("coerce", value)?
                    .into_iter()
                    .map(|item| character_argument("coerce", &item))
                    .collect::<Result<String, RuntimeError>>()?,
            };
            Ok(Value::string(result))
        }
        "SEQUENCE" => match &arguments[0] {
            Value::Nil | Value::List(_) | Value::Vector { .. } | Value::String(_) => {
                Ok(arguments[0].clone())
            }
            value => Err(type_error("coerce", "a sequence", value)),
        },
        "CHARACTER" => match &arguments[0] {
            Value::Character(_) => Ok(arguments[0].clone()),
            value => Err(type_error("coerce", "a character", value)),
        },
        _ => Err(RuntimeError::InvalidForm {
            message: format!("coerce does not support result type {result_type}"),
            span: None,
        }),
    }
}

fn sequence_bounds(
    function: &str,
    length: usize,
    options: &[Value],
) -> Result<(usize, usize), RuntimeError> {
    let mut start = 0;
    let mut end = length;
    for pair in options.chunks_exact(2) {
        match array_option_name(function, &pair[0])?.as_str() {
            "START" => start = index_argument(function, &pair[1])?,
            "END" => end = index_argument(function, &pair[1])?,
            option => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start > end || end > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bounds are invalid"),
            span: None,
        });
    }
    Ok((start, end))
}

fn replace_bounds(
    first_length: usize,
    second_length: usize,
    options: &[Value],
) -> Result<(usize, usize, usize, usize), RuntimeError> {
    let mut start1 = 0;
    let mut end1 = first_length;
    let mut start2 = 0;
    let mut end2 = second_length;
    for pair in options.chunks_exact(2) {
        let option = array_option_name("replace", &pair[0])?;
        match option.as_str() {
            "START1" => start1 = index_argument("replace", &pair[1])?,
            "END1" => end1 = index_argument("replace", &pair[1])?,
            "START2" => start2 = index_argument("replace", &pair[1])?,
            "END2" => end2 = index_argument("replace", &pair[1])?,
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("replace does not accept :{option}"),
                    span: None,
                });
            }
        }
    }
    if start1 > end1 || end1 > first_length || start2 > end2 || end2 > second_length {
        return Err(RuntimeError::InvalidForm {
            message: "replace bounds are invalid".to_string(),
            span: None,
        });
    }
    Ok((start1, end1, start2, end2))
}

fn sequence_elements(function: &str, value: &Value) -> Result<Vec<Value>, RuntimeError> {
    match value {
        Value::Nil => Ok(Vec::new()),
        Value::List(items) => Ok(items.as_ref().clone()),
        value if value.vector_items().is_some() => {
            Ok(value.vector_items().expect("vector has vector items"))
        }
        Value::String(value) => Ok(value.chars().map(Value::Character).collect()),
        _ => Err(type_error(function, "sequence", value)),
    }
}

fn rebuild_sequence(
    function: &str,
    template: &Value,
    items: Vec<Value>,
) -> Result<Value, RuntimeError> {
    match template {
        Value::Nil | Value::List(_) => Ok(Value::list(items)),
        Value::Vector {
            fill_pointer,
            element_type,
            adjustable,
            ..
        } => Ok(Value::vector_with_fill_pointer_element_type_and_adjustable(
            items,
            *fill_pointer,
            element_type.as_ref().clone(),
            *adjustable,
        )),
        Value::String(_) => {
            let mut result = String::new();
            for item in items {
                let Value::Character(character) = item else {
                    return Err(type_error(
                        function,
                        "characters for a string sequence",
                        &item,
                    ));
                };
                result.push(character);
            }
            Ok(Value::string(result))
        }
        value => Err(type_error(function, "sequence", value)),
    }
}

fn getf(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&arguments.len()) {
        return Err(arity("getf", "2 or 3", arguments.len()));
    }
    let Some(items) = arguments[0].list_items() else {
        return Err(type_error("getf", "property list", &arguments[0]));
    };
    if items.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "getf requires an even-length property list".to_string(),
            span: None,
        });
    }
    for pair in items.chunks_exact(2) {
        if arguments[1].eq_value(&pair[0]) {
            return Ok(pair[1].clone());
        }
    }
    Ok(arguments.get(2).cloned().unwrap_or(Value::Nil))
}

fn get_properties(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-properties", 2)?;
    let Some(plist) = arguments[0].list_items() else {
        return Err(type_error("get-properties", "property list", &arguments[0]));
    };
    let Some(indicators) = arguments[1].list_items() else {
        return Err(type_error("get-properties", "list", &arguments[1]));
    };
    if plist.len() % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "get-properties requires an even-length property list".to_string(),
            span: None,
        });
    }
    for (index, pair) in plist.chunks_exact(2).enumerate() {
        if indicators
            .iter()
            .any(|indicator| indicator.eq_value(&pair[0]))
        {
            return Ok(Value::values(vec![
                pair[0].clone(),
                pair[1].clone(),
                Value::list(plist[index * 2..].to_vec()),
            ]));
        }
    }
    Ok(Value::values(vec![Value::Nil, Value::Nil, Value::Nil]))
}

fn sequence_length(value: &Value) -> Option<usize> {
    match value {
        Value::Nil => Some(0),
        Value::List(items) => Some(items.len()),
        value if value.vector_items().is_some() => {
            Some(value.vector_items().expect("vector has vector items").len())
        }
        Value::String(value) => Some(value.chars().count()),
        _ => None,
    }
}

fn index_argument(function: &str, value: &Value) -> Result<usize, RuntimeError> {
    let index = integer_argument(function, value)?;
    usize::try_from(index).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} index must be non-negative"),
        span: None,
    })
}

fn out_of_bounds(function: &str, index: usize) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} index {index} is out of bounds"),
        span: None,
    }
}

fn endp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "endp", 1)?;
    match &arguments[0] {
        Value::Nil => Ok(Value::boolean(true)),
        Value::List(_) => Ok(Value::boolean(false)),
        value => Err(type_error("endp", "list", value)),
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
        value => return Err(type_error("symbol-package", "a symbol", value)),
    };
    Ok(Value::symbol(package_name))
}

fn vectorp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "vectorp", 1)?;
    Ok(Value::boolean(arguments[0].vector_items().is_some()))
}

fn simple_vector_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-vector-p", 1)?;
    Ok(Value::boolean(arguments[0].is_simple_vector()))
}

fn fill_pointer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "fill-pointer", 1)?;
    arguments[0]
        .vector_fill_pointer()
        .map(|fill_pointer| Value::Integer(fill_pointer as i64))
        .ok_or_else(|| type_error("fill-pointer", "vector with fill pointer", &arguments[0]))
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

pub(crate) fn typep_value(value: &Value, type_designator: &Value) -> Result<bool, RuntimeError> {
    type_matches_designator("typep", value, type_designator, None)
}

pub(crate) fn typep_value_in(
    value: &Value,
    type_designator: &Value,
    environment: &Environment,
) -> Result<bool, RuntimeError> {
    type_matches_designator("typep", value, type_designator, Some(environment))
}

pub(crate) fn subtypep_value(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    validate_subtype_designator("subtypep", subtype, environment)?;
    validate_subtype_designator("subtypep", supertype, environment)?;
    let relation = subtype_relation(subtype, supertype, environment)?;
    Ok(Value::values(vec![
        Value::boolean(relation.unwrap_or(false)),
        Value::boolean(relation.is_some()),
    ]))
}

pub(crate) fn upgraded_array_element_type_value(
    type_spec: &Value,
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    validate_element_subtype_designator("upgraded-array-element-type", type_spec, environment)?;
    match type_spec {
        Value::List(_) => Ok(type_spec.clone()),
        Value::DottedList { .. } => Err(invalid_type_spec(
            "upgraded-array-element-type",
            "type designator must be a proper list",
        )),
        _ => {
            let type_name = type_designator_name("upgraded-array-element-type", type_spec)?;
            let upgraded = match type_name.as_str() {
                "BASE-CHAR" | "STANDARD-CHAR" | "EXTENDED-CHAR" => "CHARACTER",
                _ => type_name.as_str(),
            };
            Ok(match upgraded {
                "NIL" => Value::Nil,
                name => Value::symbol(name),
            })
        }
    }
}

fn validate_subtype_designator(
    function: &str,
    designator: &Value,
    environment: &Environment,
) -> Result<(), RuntimeError> {
    match designator {
        Value::List(items) => {
            let Some(operator_value) = items.first() else {
                return Err(invalid_type_spec(
                    function,
                    "compound type designator must name an operator",
                ));
            };
            let operator = type_designator_name(function, operator_value)?;
            let arguments = &items[1..];
            match operator.as_str() {
                "OR" | "AND" => {
                    for argument in arguments {
                        validate_subtype_designator(function, argument, environment)?;
                    }
                }
                "NOT" | "EQL" => {
                    require_type_spec_arity(function, &operator, arguments, 1, 1)?;
                    if operator == "NOT" {
                        validate_subtype_designator(function, &arguments[0], environment)?;
                    }
                }
                "MEMBER" => {}
                "INTEGER" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    for bound in arguments {
                        integer_type_bound(function, bound)?;
                    }
                }
                "MOD" => {
                    require_type_spec_arity(function, &operator, arguments, 1, 1)?;
                    let Value::Integer(modulus) = arguments[0] else {
                        return Err(type_error(function, "non-negative integer", &arguments[0]));
                    };
                    if modulus < 0 {
                        return Err(invalid_type_spec(
                            function,
                            "MOD type specifier requires a non-negative modulus",
                        ));
                    }
                }
                "SIGNED-BYTE" | "UNSIGNED-BYTE" => {
                    byte_type_size(function, &operator, arguments)?;
                }
                "CONS" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    for argument in arguments {
                        validate_subtype_designator(function, argument, environment)?;
                    }
                }
                "VECTOR" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    if let Some(element_type) = arguments.first() {
                        validate_element_subtype_designator(function, element_type, environment)?;
                    }
                    if let Some(size) = arguments.get(1) {
                        type_spec_size(function, size)?;
                    }
                }
                "SIMPLE-VECTOR" | "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 1)?;
                    if let Some(size) = arguments.first() {
                        type_spec_size(function, size)?;
                    }
                }
                "ARRAY" | "SIMPLE-ARRAY" => {
                    require_type_spec_arity(function, &operator, arguments, 0, 2)?;
                    if let Some(element_type) = arguments.first() {
                        validate_element_subtype_designator(function, element_type, environment)?;
                    }
                    if let Some(dimensions) = arguments.get(1) {
                        validate_array_dimensions_spec(function, dimensions)?;
                    }
                }
                _ => {
                    return Err(invalid_type_spec(
                        function,
                        format!("unknown compound type designator {operator}"),
                    ));
                }
            }
            Ok(())
        }
        Value::DottedList { .. } => Err(invalid_type_spec(
            function,
            "type designator must be a proper list",
        )),
        _ => {
            let type_name = type_designator_name(function, designator)?;
            if known_type_name(&type_name, environment) {
                Ok(())
            } else {
                Err(invalid_type_spec(
                    function,
                    format!("unknown type designator {type_name}"),
                ))
            }
        }
    }
}

fn validate_element_subtype_designator(
    function: &str,
    designator: &Value,
    environment: &Environment,
) -> Result<(), RuntimeError> {
    if is_type_wildcard(designator) {
        Ok(())
    } else {
        validate_subtype_designator(function, designator, environment)
    }
}

fn validate_array_dimensions_spec(function: &str, dimensions: &Value) -> Result<(), RuntimeError> {
    if is_type_wildcard(dimensions) {
        return Ok(());
    }
    match dimensions {
        Value::Nil | Value::Boolean(false) => Ok(()),
        Value::Integer(rank) => usize::try_from(*rank)
            .map(|_| ())
            .map_err(|_| invalid_type_spec(function, "array rank must be non-negative")),
        Value::List(dimensions) => {
            for dimension in dimensions.iter() {
                if is_type_wildcard(dimension) {
                    continue;
                }
                let Value::Integer(dimension) = dimension else {
                    return Err(type_error(function, "array dimension or *", dimension));
                };
                if *dimension < 0 {
                    return Err(invalid_type_spec(
                        function,
                        "array dimensions must be non-negative",
                    ));
                }
            }
            Ok(())
        }
        value => Err(type_error(function, "array dimensions", value)),
    }
}

fn known_type_name(type_name: &str, environment: &Environment) -> bool {
    is_builtin_type_name(type_name)
        || environment.lookup_class(type_name).is_some()
        || environment.lookup_structure(type_name).is_some()
        || environment.lookup_condition(type_name).is_some()
}

fn is_builtin_type_name(type_name: &str) -> bool {
    matches!(
        type_name,
        "T" | "OBJECT"
            | "NIL"
            | "NULL"
            | "BOOLEAN"
            | "NUMBER"
            | "REAL"
            | "RATIONAL"
            | "RATIO"
            | "INTEGER"
            | "FIXNUM"
            | "BIGNUM"
            | "BIT"
            | "FLOAT"
            | "CHARACTER"
            | "BASE-CHAR"
            | "STANDARD-CHAR"
            | "EXTENDED-CHAR"
            | "STRING"
            | "BASE-STRING"
            | "SIMPLE-STRING"
            | "SIMPLE-BASE-STRING"
            | "STREAM"
            | "SYMBOL"
            | "PACKAGE"
            | "ENVIRONMENT"
            | "KEYWORD"
            | "CONS"
            | "LIST"
            | "ATOM"
            | "VECTOR"
            | "SIMPLE-VECTOR"
            | "BIT-VECTOR"
            | "SIMPLE-BIT-VECTOR"
            | "ARRAY"
            | "SIMPLE-ARRAY"
            | "HASH-TABLE"
            | "CONDITION"
            | "RESTART"
            | "STRUCTURE"
            | "SEQUENCE"
            | "FUNCTION"
            | "COMPILED-FUNCTION"
            | "UNBOUND"
            | "VALUES"
            | "CLASS"
            | "METHOD"
            | "STANDARD-OBJECT"
    )
}

fn subtype_relation(
    subtype: &Value,
    supertype: &Value,
    environment: &Environment,
) -> Result<Option<bool>, RuntimeError> {
    if same_type_designator(subtype, supertype) {
        return Ok(Some(true));
    }

    if let Some((operator, arguments)) = compound_type_parts(subtype) {
        match operator.as_str() {
            "OR" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(argument, supertype, environment)? {
                        Some(true) => {}
                        Some(false) => return Ok(Some(false)),
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "AND" => {
                for argument in arguments {
                    if subtype_relation(argument, supertype, environment)? == Some(true) {
                        return Ok(Some(true));
                    }
                }
                return Ok(None);
            }
            "MEMBER" | "EQL" => {
                let candidates = if operator == "MEMBER" {
                    arguments
                } else {
                    &arguments[..1]
                };
                let mut unknown = false;
                for candidate in candidates {
                    match type_matches_designator("subtypep", candidate, supertype, None) {
                        Ok(true) => {}
                        Ok(false) => return Ok(Some(false)),
                        Err(_) => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "INTEGER" => {
                if let Some((super_operator, super_arguments)) = compound_type_parts(supertype) {
                    if super_operator == "INTEGER" {
                        return Ok(Some(integer_spec_is_subtype(arguments, super_arguments)?));
                    }
                }
                if let Some(super_name) = atomic_type_name(supertype) {
                    return Ok(Some(compound_subtype_named(&operator, &super_name)));
                }
            }
            "MOD" | "SIGNED-BYTE" | "UNSIGNED-BYTE" => {
                if let Some(super_name) = atomic_type_name(supertype) {
                    return Ok(Some(compound_subtype_named(&operator, &super_name)));
                }
            }
            "CONS" | "VECTOR" | "SIMPLE-VECTOR" | "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" | "ARRAY"
            | "SIMPLE-ARRAY" => {
                if let Some(super_name) = atomic_type_name(supertype) {
                    return Ok(Some(compound_subtype_named(&operator, &super_name)));
                }
            }
            _ => {}
        }
    }

    if let Some((operator, arguments)) = compound_type_parts(supertype) {
        match operator.as_str() {
            "OR" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(subtype, argument, environment)? {
                        Some(true) => return Ok(Some(true)),
                        Some(false) => {}
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(false) });
            }
            "AND" => {
                let mut unknown = false;
                for argument in arguments {
                    match subtype_relation(subtype, argument, environment)? {
                        Some(false) => return Ok(Some(false)),
                        Some(true) => {}
                        None => unknown = true,
                    }
                }
                return Ok(if unknown { None } else { Some(true) });
            }
            "INTEGER" => {
                if let Some(subtype_name) = atomic_type_name(subtype) {
                    return Ok(Some(named_integer_is_subtype(&subtype_name, arguments)?));
                }
            }
            _ => {}
        }
    }

    let Some(subtype_name) = atomic_type_name(subtype) else {
        return Ok(None);
    };
    let Some(supertype_name) = atomic_type_name(supertype) else {
        return Ok(None);
    };
    Ok(named_subtype_relation(
        &subtype_name,
        &supertype_name,
        environment,
    ))
}

fn compound_type_parts(value: &Value) -> Option<(String, &[Value])> {
    let Value::List(items) = value else {
        return None;
    };
    let operator = type_designator_name("subtypep", items.first()?).ok()?;
    Some((operator, &items[1..]))
}

fn atomic_type_name(value: &Value) -> Option<String> {
    if matches!(value, Value::List(_) | Value::DottedList { .. }) {
        None
    } else {
        type_designator_name("subtypep", value).ok()
    }
}

fn same_type_designator(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::List(left), Value::List(right)) => {
            if left.len() != right.len() {
                return false;
            }
            let Some(left_operator) = left
                .first()
                .and_then(|value| type_designator_name("subtypep", value).ok())
            else {
                return false;
            };
            let Some(right_operator) = right
                .first()
                .and_then(|value| type_designator_name("subtypep", value).ok())
            else {
                return false;
            };
            if left_operator != right_operator {
                return false;
            }
            left.iter()
                .zip(right.iter())
                .enumerate()
                .all(|(index, (left, right))| {
                    if index == 0 {
                        true
                    } else if matches!(left_operator.as_str(), "MEMBER" | "EQL") {
                        eql_value(left, right)
                    } else {
                        same_type_designator(left, right)
                    }
                })
        }
        (Value::DottedList { .. }, Value::DottedList { .. }) => false,
        (Value::List(_), _) | (_, Value::List(_)) => false,
        (Value::DottedList { .. }, _) | (_, Value::DottedList { .. }) => false,
        _ => match (
            type_designator_name("subtypep", left).ok(),
            type_designator_name("subtypep", right).ok(),
        ) {
            (Some(left), Some(right)) => left == right,
            (None, None) => eql_value(left, right),
            _ => false,
        },
    }
}

fn integer_spec_is_subtype(
    subtype_arguments: &[Value],
    supertype_arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let subtype_lower = subtype_arguments
        .first()
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let subtype_upper = subtype_arguments
        .get(1)
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let supertype_lower = supertype_arguments
        .first()
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();
    let supertype_upper = supertype_arguments
        .get(1)
        .map(|bound| integer_type_bound("subtypep", bound))
        .transpose()?
        .flatten();

    let subtype_empty = subtype_lower
        .zip(subtype_upper)
        .is_some_and(|(lower, upper)| lower > upper);
    let supertype_empty = supertype_lower
        .zip(supertype_upper)
        .is_some_and(|(lower, upper)| lower > upper);
    if subtype_empty {
        return Ok(true);
    }
    if supertype_empty {
        return Ok(false);
    }

    let lower_ok = match (subtype_lower, supertype_lower) {
        (_, None) => true,
        (Some(subtype), Some(supertype)) => subtype >= supertype,
        (None, Some(_)) => false,
    };
    let upper_ok = match (subtype_upper, supertype_upper) {
        (_, None) => true,
        (Some(subtype), Some(supertype)) => subtype <= supertype,
        (None, Some(_)) => false,
    };
    Ok(lower_ok && upper_ok)
}

fn named_integer_is_subtype(
    subtype_name: &str,
    supertype_arguments: &[Value],
) -> Result<bool, RuntimeError> {
    if subtype_name == "BIT" {
        return integer_spec_is_subtype(
            &[Value::Integer(0), Value::Integer(1)],
            supertype_arguments,
        );
    }
    if matches!(subtype_name, "INTEGER" | "FIXNUM" | "BIGNUM") {
        let lower = supertype_arguments
            .first()
            .map(|bound| integer_type_bound("subtypep", bound))
            .transpose()?
            .flatten();
        let upper = supertype_arguments
            .get(1)
            .map(|bound| integer_type_bound("subtypep", bound))
            .transpose()?
            .flatten();
        return Ok(lower.is_none() && upper.is_none());
    }
    Ok(false)
}

fn compound_subtype_named(operator: &str, supertype_name: &str) -> bool {
    match operator {
        "INTEGER" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "MOD" | "SIGNED-BYTE" | "UNSIGNED-BYTE" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "CONS" => matches!(supertype_name, "CONS" | "LIST" | "SEQUENCE"),
        "VECTOR" | "SIMPLE-VECTOR" => matches!(
            supertype_name,
            "VECTOR" | "SIMPLE-VECTOR" | "ARRAY" | "SIMPLE-ARRAY" | "SEQUENCE" | "ATOM"
        ),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => matches!(
            supertype_name,
            "BIT-VECTOR"
                | "SIMPLE-BIT-VECTOR"
                | "VECTOR"
                | "SIMPLE-VECTOR"
                | "ARRAY"
                | "SIMPLE-ARRAY"
                | "SEQUENCE"
                | "ATOM"
        ),
        "ARRAY" | "SIMPLE-ARRAY" => {
            matches!(supertype_name, "ARRAY" | "SIMPLE-ARRAY" | "ATOM")
        }
        _ => false,
    }
}

fn named_subtype_relation(
    subtype_name: &str,
    supertype_name: &str,
    environment: &Environment,
) -> Option<bool> {
    if subtype_name == supertype_name
        || matches!(supertype_name, "T" | "OBJECT")
        || builtin_subtype(subtype_name, supertype_name)
    {
        return Some(true);
    }

    if let Some(class) = environment.lookup_class(subtype_name) {
        if class
            .precedence
            .iter()
            .any(|name| name.eq_ignore_ascii_case(supertype_name))
        {
            return Some(true);
        }
    }
    if let Some(condition) = environment.lookup_condition(subtype_name) {
        if condition
            .precedence
            .iter()
            .any(|name| name.eq_ignore_ascii_case(supertype_name))
        {
            return Some(true);
        }
    }
    if let Some(structure) = environment.lookup_structure(subtype_name) {
        if supertype_name == "STRUCTURE"
            || structure
                .type_names
                .iter()
                .any(|name| name.eq_ignore_ascii_case(supertype_name))
        {
            return Some(true);
        }
    }

    if known_type_name(subtype_name, environment) && known_type_name(supertype_name, environment) {
        Some(false)
    } else {
        None
    }
}

fn builtin_subtype(subtype_name: &str, supertype_name: &str) -> bool {
    match subtype_name {
        "NIL" | "NULL" => matches!(
            supertype_name,
            "SYMBOL" | "LIST" | "SEQUENCE" | "ATOM" | "BOOLEAN" | "NIL" | "NULL"
        ),
        "BOOLEAN" => matches!(supertype_name, "SYMBOL" | "ATOM"),
        "NUMBER" => matches!(supertype_name, "REAL" | "ATOM"),
        "REAL" => matches!(supertype_name, "NUMBER" | "ATOM"),
        "FIXNUM" | "BIGNUM" | "BIT" => matches!(
            supertype_name,
            "INTEGER" | "RATIONAL" | "NUMBER" | "REAL" | "ATOM"
        ),
        "INTEGER" => matches!(supertype_name, "RATIONAL" | "NUMBER" | "REAL" | "ATOM"),
        "RATIO" => matches!(supertype_name, "RATIONAL" | "NUMBER" | "REAL" | "ATOM"),
        "RATIONAL" => matches!(supertype_name, "NUMBER" | "REAL" | "ATOM"),
        "FLOAT" => matches!(supertype_name, "NUMBER" | "REAL" | "ATOM"),
        "BASE-CHAR" => matches!(supertype_name, "CHARACTER" | "ATOM"),
        "STANDARD-CHAR" => matches!(supertype_name, "BASE-CHAR" | "CHARACTER" | "ATOM"),
        "EXTENDED-CHAR" => matches!(supertype_name, "CHARACTER" | "ATOM"),
        "CHARACTER" => supertype_name == "ATOM",
        "STRING" | "BASE-STRING" => {
            matches!(
                supertype_name,
                "STRING" | "BASE-STRING" | "SEQUENCE" | "ATOM"
            )
        }
        "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => matches!(
            supertype_name,
            "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" | "SEQUENCE" | "ATOM"
        ),
        "SYMBOL" => supertype_name == "ATOM",
        "KEYWORD" => matches!(supertype_name, "SYMBOL" | "ATOM"),
        "CONS" => matches!(supertype_name, "LIST" | "SEQUENCE"),
        "LIST" => supertype_name == "SEQUENCE",
        "VECTOR" | "SIMPLE-VECTOR" => matches!(
            supertype_name,
            "VECTOR" | "SIMPLE-VECTOR" | "ARRAY" | "SIMPLE-ARRAY" | "SEQUENCE" | "ATOM"
        ),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => matches!(
            supertype_name,
            "BIT-VECTOR"
                | "SIMPLE-BIT-VECTOR"
                | "VECTOR"
                | "SIMPLE-VECTOR"
                | "ARRAY"
                | "SIMPLE-ARRAY"
                | "SEQUENCE"
                | "ATOM"
        ),
        "ARRAY" | "SIMPLE-ARRAY" => {
            matches!(supertype_name, "ARRAY" | "SIMPLE-ARRAY" | "ATOM")
        }
        "COMPILED-FUNCTION" => matches!(supertype_name, "FUNCTION" | "ATOM"),
        "FUNCTION" | "STREAM" | "PACKAGE" | "ENVIRONMENT" | "HASH-TABLE" | "CONDITION"
        | "RESTART" | "STRUCTURE" | "UNBOUND" | "VALUES" | "CLASS" | "STANDARD-OBJECT" => {
            supertype_name == "ATOM"
        }
        _ => false,
    }
}

pub(crate) fn the_check(arguments: &[Value]) -> Result<Value, RuntimeError> {
    the_check_with_environment(arguments, None)
}

pub(crate) fn the_check_in(
    arguments: &[Value],
    environment: &Environment,
) -> Result<Value, RuntimeError> {
    the_check_with_environment(arguments, Some(environment))
}

fn the_check_with_environment(
    arguments: &[Value],
    environment: Option<&Environment>,
) -> Result<Value, RuntimeError> {
    exact(arguments, "the", 2)?;
    let type_description = arguments[1].to_string();
    if type_matches_designator("the", &arguments[0], &arguments[1], environment)? {
        Ok(arguments[0].clone())
    } else {
        Err(RuntimeError::Type {
            expected: format!("the requires value of type {type_description}"),
            actual: arguments[0].type_name().to_string(),
            span: None,
        })
    }
}

fn require_integer(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_REQUIRE_INTEGER", 1)?;
    match &arguments[0] {
        Value::Integer(_) => Ok(arguments[0].clone()),
        value => Err(RuntimeError::Type {
            expected: "INTEGER".to_string(),
            actual: value.type_name().to_string(),
            span: None,
        }),
    }
}

fn require_list(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_REQUIRE_LIST", 1)?;
    match &arguments[0] {
        Value::Nil | Value::List(_) => Ok(arguments[0].clone()),
        value => Err(RuntimeError::Type {
            expected: "LIST".to_string(),
            actual: value.type_name().to_string(),
            span: None,
        }),
    }
}

fn ecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "ecase fell through".to_string(),
        span: None,
    })
}

fn etypecase_error(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__NCL_ETYPECASE_ERROR", 0)?;
    Err(RuntimeError::InvalidForm {
        message: "etypecase fell through".to_string(),
        span: None,
    })
}

fn type_designator_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let type_name = match value {
        Value::Nil | Value::Boolean(false) => "NIL",
        Value::Boolean(true) => "T",
        Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::Keyword(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => name.as_ref(),
        value => return Err(type_error(function, "type designator", value)),
    };
    let type_name = type_name.rsplit("::").next().unwrap_or(type_name);
    Ok(package::normalize_symbol_name(type_name))
}

fn type_matches_designator(
    function: &str,
    value: &Value,
    type_designator: &Value,
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    match type_designator {
        Value::List(items) => type_matches_compound(function, value, items.as_ref(), environment),
        Value::DottedList { .. } => Err(invalid_type_spec(
            function,
            "type designator must be a proper list",
        )),
        _ => {
            let type_name = type_designator_name(function, type_designator)?;
            type_matches(value, &type_name, environment)
        }
    }
}

fn type_matches_compound(
    function: &str,
    value: &Value,
    items: &[Value],
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    let Some(operator_value) = items.first() else {
        return Err(invalid_type_spec(
            function,
            "compound type designator must name an operator",
        ));
    };
    let operator = type_designator_name(function, operator_value)?;
    let arguments = &items[1..];
    match operator.as_str() {
        "OR" => {
            for type_designator in arguments {
                if type_matches_designator(function, value, type_designator, environment)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        "AND" => {
            for type_designator in arguments {
                if !type_matches_designator(function, value, type_designator, environment)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        "NOT" => {
            require_type_spec_arity(function, &operator, arguments, 1, 1)?;
            Ok(!type_matches_designator(
                function,
                value,
                &arguments[0],
                environment,
            )?)
        }
        "MEMBER" => Ok(arguments
            .iter()
            .any(|candidate| eql_value(value, candidate))),
        "EQL" => {
            require_type_spec_arity(function, &operator, arguments, 1, 1)?;
            Ok(eql_value(value, &arguments[0]))
        }
        "INTEGER" => integer_type_matches(function, value, arguments),
        "MOD" => mod_type_matches(function, value, arguments),
        "SIGNED-BYTE" => signed_byte_type_matches(function, value, arguments),
        "UNSIGNED-BYTE" => unsigned_byte_type_matches(function, value, arguments),
        "CONS" => cons_type_matches(function, value, arguments, environment),
        "VECTOR" => vector_type_matches(function, value, arguments, environment),
        "SIMPLE-VECTOR" => simple_vector_type_matches(function, value, arguments),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => bit_vector_type_matches(function, value, arguments),
        "ARRAY" | "SIMPLE-ARRAY" => {
            array_type_matches(function, &operator, value, arguments, environment)
        }
        _ => Err(invalid_type_spec(
            function,
            format!("unknown compound type designator {operator}"),
        )),
    }
}

fn require_type_spec_arity(
    function: &str,
    operator: &str,
    arguments: &[Value],
    minimum: usize,
    maximum: usize,
) -> Result<(), RuntimeError> {
    if (minimum..=maximum).contains(&arguments.len()) {
        Ok(())
    } else {
        Err(invalid_type_spec(
            function,
            format!("{operator} type specifier expects between {minimum} and {maximum} arguments"),
        ))
    }
}

fn invalid_type_spec(function: &str, message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function}: {}", message.into()),
        span: None,
    }
}

fn integer_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "INTEGER", arguments, 0, 2)?;
    let lower = arguments
        .first()
        .map(|bound| integer_type_bound(function, bound))
        .transpose()?
        .flatten();
    let upper = arguments
        .get(1)
        .map(|bound| integer_type_bound(function, bound))
        .transpose()?
        .flatten();
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    Ok(
        lower.map_or(true, |bound| *number >= bound)
            && upper.map_or(true, |bound| *number <= bound),
    )
}

fn integer_type_bound(function: &str, value: &Value) -> Result<Option<i64>, RuntimeError> {
    if is_type_wildcard(value) {
        return Ok(None);
    }
    match value {
        Value::Integer(bound) => Ok(Some(*bound)),
        value => Err(type_error(function, "integer or *", value)),
    }
}

fn mod_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "MOD", arguments, 1, 1)?;
    let Value::Integer(modulus) = arguments[0] else {
        return Err(type_error(function, "non-negative integer", &arguments[0]));
    };
    if modulus < 0 {
        return Err(invalid_type_spec(
            function,
            "MOD type specifier requires a non-negative modulus",
        ));
    }
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    Ok(*number >= 0 && *number < modulus)
}

fn unsigned_byte_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let size = byte_type_size(function, "UNSIGNED-BYTE", arguments)?;
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    if *number < 0 {
        return Ok(false);
    }
    let Some(size) = size else {
        return Ok(true);
    };
    if size >= 63 {
        return Ok(true);
    }
    let upper = (1_i128 << size) - 1;
    Ok((*number as i128) <= upper)
}

fn signed_byte_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    let size = byte_type_size(function, "SIGNED-BYTE", arguments)?;
    let Value::Integer(number) = value else {
        return Ok(false);
    };
    let Some(size) = size else {
        return Ok(true);
    };
    if size == 0 {
        return Ok(false);
    }
    if size >= 64 {
        return Ok(true);
    }
    let half = 1_i128 << (size - 1);
    let number = *number as i128;
    Ok(number >= -half && number < half)
}

fn byte_type_size(
    function: &str,
    operator: &str,
    arguments: &[Value],
) -> Result<Option<usize>, RuntimeError> {
    require_type_spec_arity(function, operator, arguments, 0, 1)?;
    let Some(size) = arguments.first() else {
        return Ok(None);
    };
    if is_type_wildcard(size) {
        return Ok(None);
    }
    let Value::Integer(size) = size else {
        return Err(type_error(function, "non-negative integer or *", size));
    };
    usize::try_from(*size).map(Some).map_err(|_| {
        invalid_type_spec(
            function,
            format!("{operator} type specifier requires a non-negative size"),
        )
    })
}

fn cons_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "CONS", arguments, 0, 2)?;
    let Some((car, cdr)) = cons_parts(value) else {
        return Ok(false);
    };
    if let Some(car_type) = arguments.first() {
        if !type_matches_designator(function, &car, car_type, environment)? {
            return Ok(false);
        }
    }
    if let Some(cdr_type) = arguments.get(1) {
        if !type_matches_designator(function, &cdr, cdr_type, environment)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn cons_parts(value: &Value) -> Option<(Value, Value)> {
    match value {
        Value::List(items) if !items.is_empty() => {
            let items = items.as_ref();
            let tail = if items.len() == 1 {
                Value::Nil
            } else {
                Value::list(items[1..].to_vec())
            };
            Some((items[0].clone(), tail))
        }
        Value::DottedList { items, tail } if !items.is_empty() => {
            Some((items[0].clone(), (*tail).as_ref().clone()))
        }
        _ => None,
    }
}

fn vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "VECTOR", arguments, 0, 2)?;
    let expected_size = arguments
        .get(1)
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    if expected_size.map_or(false, |size| size != items.len()) {
        return Ok(false);
    }
    if let Some(element_type) = arguments.first() {
        for item in &items {
            if !type_matches_element_spec(function, item, element_type, environment)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn simple_vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "SIMPLE-VECTOR", arguments, 0, 1)?;
    let expected_size = arguments
        .first()
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    Ok(expected_size.map_or(true, |size| size == items.len()))
}

fn bit_vector_type_matches(
    function: &str,
    value: &Value,
    arguments: &[Value],
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, "BIT-VECTOR", arguments, 0, 1)?;
    let expected_size = arguments
        .first()
        .map(|size| type_spec_size(function, size))
        .transpose()?
        .flatten();
    let Some(items) = value.vector_items() else {
        return Ok(false);
    };
    if expected_size.map_or(false, |size| size != items.len()) {
        return Ok(false);
    }
    Ok(items.iter().all(is_bit_value))
}

fn is_bit_vector_value(value: &Value) -> bool {
    value
        .vector_items()
        .is_some_and(|items| items.iter().all(is_bit_value))
}

fn is_bit_value(value: &Value) -> bool {
    matches!(value, Value::Integer(bit) if *bit == 0 || *bit == 1)
}

fn array_type_matches(
    function: &str,
    operator: &str,
    value: &Value,
    arguments: &[Value],
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    require_type_spec_arity(function, operator, arguments, 0, 2)?;
    let Some(actual_dimensions) = dimensions_for_array(value) else {
        return Ok(false);
    };
    if let Some(expected_dimensions) = arguments.get(1) {
        if !array_dimensions_match(function, expected_dimensions, &actual_dimensions)? {
            return Ok(false);
        }
    }
    let Some(elements) = array_elements(value) else {
        return Ok(false);
    };
    if let Some(element_type) = arguments.first() {
        for element in &elements {
            if !type_matches_element_spec(function, element, element_type, environment)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn type_matches_element_spec(
    function: &str,
    value: &Value,
    type_designator: &Value,
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    if is_type_wildcard(type_designator) {
        Ok(true)
    } else {
        type_matches_designator(function, value, type_designator, environment)
    }
}

fn type_spec_size(function: &str, value: &Value) -> Result<Option<usize>, RuntimeError> {
    if is_type_wildcard(value) {
        return Ok(None);
    }
    let Value::Integer(size) = value else {
        return Err(type_error(function, "non-negative integer or *", value));
    };
    usize::try_from(*size)
        .map(Some)
        .map_err(|_| invalid_type_spec(function, "sequence or array size must be non-negative"))
}

fn array_dimensions_match(
    function: &str,
    expected: &Value,
    actual: &[usize],
) -> Result<bool, RuntimeError> {
    if is_type_wildcard(expected) {
        return Ok(true);
    }
    match expected {
        Value::Nil | Value::Boolean(false) => Ok(actual.is_empty()),
        Value::Integer(rank) => {
            let rank = usize::try_from(*rank)
                .map_err(|_| invalid_type_spec(function, "array rank must be non-negative"))?;
            Ok(actual.len() == rank)
        }
        Value::List(dimensions) => {
            let dimensions = dimensions.as_ref();
            if dimensions.len() != actual.len() {
                return Ok(false);
            }
            for (dimension, actual) in dimensions.iter().zip(actual) {
                if is_type_wildcard(dimension) {
                    continue;
                }
                let Value::Integer(expected) = dimension else {
                    return Err(type_error(function, "array dimension or *", dimension));
                };
                let expected = usize::try_from(*expected).map_err(|_| {
                    invalid_type_spec(function, "array dimensions must be non-negative")
                })?;
                if expected != *actual {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        value => Err(type_error(function, "array dimensions", value)),
    }
}

fn is_type_wildcard(value: &Value) -> bool {
    value
        .symbol_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("*"))
}

fn type_matches(
    value: &Value,
    type_name: &str,
    environment: Option<&Environment>,
) -> Result<bool, RuntimeError> {
    let result = match type_name {
        "T" | "OBJECT" => true,
        "NIL" | "NULL" => matches!(value, Value::Nil | Value::Boolean(false)),
        "BOOLEAN" => matches!(value, Value::Nil | Value::Boolean(_)),
        "NUMBER" => matches!(
            value,
            Value::Integer(_) | Value::Rational(_) | Value::Float(_) | Value::Complex { .. }
        ),
        "REAL" => matches!(
            value,
            Value::Integer(_) | Value::Rational(_) | Value::Float(_)
        ),
        "COMPLEX" => matches!(value, Value::Complex { .. }),
        "RATIONAL" => matches!(value, Value::Integer(_) | Value::Rational(_)),
        "RATIO" => matches!(value, Value::Rational(_)),
        "INTEGER" | "FIXNUM" | "BIGNUM" => matches!(value, Value::Integer(_)),
        "BIT" => is_bit_value(value),
        "FLOAT" => matches!(value, Value::Float(_)),
        "CHARACTER" | "BASE-CHAR" | "STANDARD-CHAR" | "EXTENDED-CHAR" => {
            matches!(value, Value::Character(_))
        }
        "STRING" | "BASE-STRING" | "SIMPLE-STRING" | "SIMPLE-BASE-STRING" => {
            matches!(value, Value::String(_))
        }
        "STREAM" => matches!(value, Value::Stream(_)),
        "SYMBOL" => matches!(
            value,
            Value::Nil
                | Value::Boolean(_)
                | Value::Symbol(_)
                | Value::UninternedSymbol(_)
                | Value::Keyword(_)
                | Value::SymbolExact(_)
                | Value::KeywordExact(_)
        ),
        "PACKAGE" => matches!(value, Value::Package(_)),
        "ENVIRONMENT" => matches!(value, Value::Environment(_)),
        "KEYWORD" => matches!(value, Value::Keyword(_) | Value::KeywordExact(_)),
        "CONS" => matches!(value, Value::List(_) | Value::DottedList { .. }),
        "LIST" => matches!(value, Value::Nil | Value::Boolean(false) | Value::List(_)),
        "ATOM" => !matches!(value, Value::List(_) | Value::DottedList { .. }),
        "VECTOR" => value.vector_items().is_some(),
        "SIMPLE-VECTOR" => value.is_simple_vector(),
        "BIT-VECTOR" | "SIMPLE-BIT-VECTOR" => is_bit_vector_value(value),
        "ARRAY" => dimensions_for_array(value).is_some(),
        "SIMPLE-ARRAY" => simple_array_value(value),
        "HASH-TABLE" => matches!(value, Value::HashTable { .. }),
        "CONDITION" => matches!(value, Value::Condition(_)),
        "RESTART" => matches!(value, Value::Restart(_)),
        "ERROR" | "SERIOUS-CONDITION" | "WARNING" | "SIMPLE-CONDITION" | "SIMPLE-ERROR"
        | "SIMPLE-WARNING" | "ARITHMETIC-ERROR" | "DIVISION-BY-ZERO" | "TYPE-ERROR"
        | "PROGRAM-ERROR" | "PACKAGE-ERROR" | "READER-ERROR" | "COMPILER-ERROR" | "FILE-ERROR"
        | "UNBOUND-VARIABLE" | "CONTROL-ERROR" => value.condition_is_type(type_name),
        "STRUCTURE" => value.structure_name().is_some(),
        "SEQUENCE" => matches!(value, Value::Boolean(false)) || sequence_length(value).is_some(),
        "FUNCTION" | "COMPILED-FUNCTION" => matches!(value, Value::Function(_)),
        "GENERIC-FUNCTION" | "STANDARD-GENERIC-FUNCTION" => matches!(
            value,
            Value::Function(function) if matches!(function.as_ref(), crate::Function::Generic { .. })
        ),
        "UNBOUND" => matches!(value, Value::Unbound),
        "VALUES" => matches!(value, Value::Values(_)),
        "CLASS" => matches!(value, Value::Class(_)),
        "METHOD" | "STANDARD-METHOD" => matches!(value, Value::Method(_)),
        "STANDARD-OBJECT" => matches!(value, Value::Instance(_)),
        _ if environment
            .is_some_and(|environment| environment.lookup_condition(type_name).is_some()) =>
        {
            value.condition_is_type(type_name)
        }
        _ if value.instance_is_type(type_name) => true,
        _ if value.structure_is_type(type_name) => true,
        _ => {
            return Err(RuntimeError::InvalidForm {
                message: format!("unknown type designator {type_name}"),
                span: None,
            });
        }
    };
    Ok(result)
}

fn reverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "reverse", 1)?;
    reverse_list("reverse", &arguments[0])
}

fn nreverse(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "nreverse", 1)?;
    reverse_list("nreverse", &arguments[0])
}

fn reverse_list(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    let Some(mut items) = value.list_items() else {
        return Err(type_error(function, "list", value));
    };
    items.reverse();
    Ok(Value::list(items))
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
    match value {
        Value::List(items) => Value::list(items.iter().map(copy_tree_value).collect()),
        Value::DottedList { items, tail } => Value::dotted_list(
            items.iter().map(copy_tree_value).collect(),
            copy_tree_value(tail),
        ),
        _ => value.clone(),
    }
}

fn vector(arguments: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::vector(arguments.to_vec()))
}

fn make_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("make-array", "at least one", 0));
    }
    let dimensions = parse_array_dimensions("make-array", &arguments[0])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut fill_pointer = None;
    let mut element_type = None;
    let mut adjustable = false;
    let mut displaced_to = None;
    let mut displaced_index_offset = None;
    if (arguments.len() - 1) % 2 != 0 {
        return Err(arity(
            "make-array",
            "one dimension and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[1..].chunks_exact(2) {
        let name = array_option_name("make-array", &pair[0])?;
        match name.as_str() {
            "INITIAL-ELEMENT" => {
                if initial_contents.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_element = Some(pair[1].clone());
            }
            "INITIAL-CONTENTS" => {
                if initial_element.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-array cannot combine :initial-element and :initial-contents"
                            .to_string(),
                        span: None,
                    });
                }
                initial_contents = Some(pair[1].clone());
            }
            "FILL-POINTER" => fill_pointer = Some(pair[1].clone()),
            "ELEMENT-TYPE" => element_type = Some(pair[1].clone()),
            "ADJUSTABLE" => adjustable = !matches!(pair[1], Value::Nil),
            "DISPLACED-TO" => displaced_to = Some(pair[1].clone()),
            "DISPLACED-INDEX-OFFSET" => displaced_index_offset = Some(pair[1].clone()),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("make-array", &dimensions)?;
    if displaced_to.is_some() && (initial_element.is_some() || initial_contents.is_some()) {
        return Err(RuntimeError::InvalidForm {
            message:
                "make-array cannot combine :displaced-to with :initial-element or :initial-contents"
                    .to_string(),
            span: None,
        });
    }
    let displacement = displaced_array_arguments(
        "make-array",
        &dimensions,
        displaced_to,
        displaced_index_offset,
    )?;
    let logical_length = dimensions[0];
    let (displaced_to, displaced_index_offset, storage, elements) =
        if let Some((displaced_to, displaced_index_offset, storage)) = displacement {
            (displaced_to, displaced_index_offset, Some(storage), None)
        } else if let Some(contents) = initial_contents {
            let mut elements = Vec::with_capacity(total_size);
            flatten_array_contents("make-array", &contents, &dimensions, &mut elements)?;
            (None, 0, None, Some(elements))
        } else {
            (
                None,
                0,
                None,
                Some(vec![initial_element.unwrap_or(Value::Nil); total_size]),
            )
        };
    let element_type = element_type.unwrap_or_else(|| Value::symbol("T"));
    if dimensions.len() == 1 {
        let fill_pointer = fill_pointer
            .map(|value| array_fill_pointer("make-array", &value, logical_length))
            .transpose()?;
        Ok(if let Some(storage) = storage {
            Value::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
                storage,
                logical_length,
                fill_pointer,
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            )
        } else {
            Value::vector_with_fill_pointer_element_type_adjustable_and_displacement(
                elements.expect("non-displaced vector elements"),
                fill_pointer,
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            )
        })
    } else {
        if fill_pointer.is_some() {
            return Err(RuntimeError::InvalidForm {
                message: "make-array :fill-pointer requires a one-dimensional array".to_string(),
                span: None,
            });
        }
        Ok(if let Some(storage) = storage {
            Value::array_with_storage_element_type_adjustable_and_displacement(
                dimensions,
                storage,
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            )
        } else {
            Value::array_with_element_type_adjustable_and_displacement(
                dimensions,
                elements.expect("non-displaced array elements"),
                element_type,
                adjustable,
                displaced_to,
                displaced_index_offset,
            )
        })
    }
}

fn adjust_array(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity(
            "adjust-array",
            "array, dimensions, and keyword/value pairs",
            arguments.len(),
        ));
    }
    let source = &arguments[0];
    dimensions_for_array(source).ok_or_else(|| type_error("adjust-array", "array", source))?;
    let dimensions = parse_array_dimensions("adjust-array", &arguments[1])?;
    let mut initial_element = None;
    let mut initial_contents = None;
    let mut fill_pointer = None;
    let mut element_type = None;
    let mut displaced_to = None;
    let mut displaced_index_offset = None;
    if (arguments.len() - 2) % 2 != 0 {
        return Err(arity(
            "adjust-array",
            "array, dimensions, and keyword/value pairs",
            arguments.len(),
        ));
    }
    for pair in arguments[2..].chunks_exact(2) {
        let name = array_option_name("adjust-array", &pair[0])?;
        match name.as_str() {
            "INITIAL-ELEMENT" => {
                if initial_contents.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message:
                            "adjust-array cannot combine :initial-element and :initial-contents"
                                .to_string(),
                        span: None,
                    });
                }
                initial_element = Some(pair[1].clone());
            }
            "INITIAL-CONTENTS" => {
                if initial_element.is_some() {
                    return Err(RuntimeError::InvalidForm {
                        message:
                            "adjust-array cannot combine :initial-element and :initial-contents"
                                .to_string(),
                        span: None,
                    });
                }
                initial_contents = Some(pair[1].clone());
            }
            "FILL-POINTER" => fill_pointer = Some(pair[1].clone()),
            "ELEMENT-TYPE" => element_type = Some(pair[1].clone()),
            "DISPLACED-TO" => displaced_to = Some(pair[1].clone()),
            "DISPLACED-INDEX-OFFSET" => displaced_index_offset = Some(pair[1].clone()),
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("adjust-array does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    let total_size = array_total_size_for("adjust-array", &dimensions)?;
    if displaced_to.is_some() && (initial_element.is_some() || initial_contents.is_some()) {
        return Err(RuntimeError::InvalidForm {
            message:
                "adjust-array cannot combine :displaced-to with :initial-element or :initial-contents"
                    .to_string(),
            span: None,
        });
    }
    let displacement = displaced_array_arguments(
        "adjust-array",
        &dimensions,
        displaced_to,
        displaced_index_offset,
    )?;
    let logical_length = dimensions[0];
    let (displaced_to, displaced_index_offset, storage, elements) =
        if let Some((displaced_to, displaced_index_offset, storage)) = displacement {
            (displaced_to, displaced_index_offset, Some(storage), None)
        } else if let Some(contents) = initial_contents {
            let mut elements = Vec::with_capacity(total_size);
            flatten_array_contents("adjust-array", &contents, &dimensions, &mut elements)?;
            (None, 0, None, Some(elements))
        } else {
            let mut elements = array_elements(source).expect("array values carry elements");
            elements.truncate(total_size);
            if elements.len() < total_size {
                elements.resize(total_size, initial_element.unwrap_or(Value::Nil));
            }
            (None, 0, None, Some(elements))
        };
    let element_type = element_type.unwrap_or_else(|| {
        source
            .array_element_type_value()
            .expect("array values carry element type")
    });
    if dimensions.len() == 1 {
        let fill_pointer = if let Some(value) = fill_pointer {
            Some(array_fill_pointer("adjust-array", &value, logical_length)?)
        } else if let Some(existing) = source.vector_fill_pointer() {
            Some(array_fill_pointer(
                "adjust-array",
                &Value::Integer(existing as i64),
                logical_length,
            )?)
        } else {
            None
        };
        Ok(if let Some(storage) = storage {
            Value::vector_with_storage_fill_pointer_element_type_adjustable_and_displacement(
                storage,
                logical_length,
                fill_pointer,
                element_type,
                source.is_adjustable_array(),
                displaced_to,
                displaced_index_offset,
            )
        } else {
            Value::vector_with_fill_pointer_element_type_adjustable_and_displacement(
                elements.expect("non-displaced vector elements"),
                fill_pointer,
                element_type,
                source.is_adjustable_array(),
                displaced_to,
                displaced_index_offset,
            )
        })
    } else {
        if fill_pointer.is_some() {
            return Err(RuntimeError::InvalidForm {
                message: "adjust-array :fill-pointer requires a one-dimensional array".to_string(),
                span: None,
            });
        }
        Ok(if let Some(storage) = storage {
            Value::array_with_storage_element_type_adjustable_and_displacement(
                dimensions,
                storage,
                element_type,
                source.is_adjustable_array(),
                displaced_to,
                displaced_index_offset,
            )
        } else {
            Value::array_with_element_type_adjustable_and_displacement(
                dimensions,
                elements.expect("non-displaced array elements"),
                element_type,
                source.is_adjustable_array(),
                displaced_to,
                displaced_index_offset,
            )
        })
    }
}

fn aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("aref", "at least one", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("aref", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "aref",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    let index = array_coordinate_index("aref", &dimensions, &arguments[1..])?;
    array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("aref", index))
}

fn svref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "svref", 2)?;
    let index = index_argument("svref", &arguments[1])?;
    if !arguments[0].is_simple_vector() {
        return Err(type_error("svref", "simple-vector", &arguments[0]));
    }
    let items = arguments[0]
        .vector_items()
        .expect("simple vector has vector items");
    items
        .get(index)
        .cloned()
        .ok_or_else(|| out_of_bounds("svref", index))
}

fn bit_value(function: &str, value: &Value) -> Result<(), RuntimeError> {
    match value {
        Value::Integer(bit) if *bit == 0 || *bit == 1 => Ok(()),
        _ => Err(type_error(function, "bit", value)),
    }
}

fn bit(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("bit", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("bit", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "bit",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    let index = array_coordinate_index("bit", &dimensions, &arguments[1..])?;
    let value = array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("bit", index))?;
    bit_value("bit", &value)?;
    Ok(value)
}

fn sbit(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("sbit", "array and subscripts", 0));
    }
    if !simple_bit_array_value(&arguments[0]) {
        return Err(type_error("sbit", "simple bit array", &arguments[0]));
    }
    bit(arguments)
}

fn row_major_aref(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "row-major-aref", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("row-major-aref", "array", &arguments[0]))?;
    let index = index_argument("row-major-aref", &arguments[1])?;
    let total_size = array_total_size_for("row-major-aref", &dimensions)?;
    if index >= total_size {
        return Err(out_of_bounds("row-major-aref", index));
    }
    array_elements(&arguments[0])
        .and_then(|items| items.get(index).cloned())
        .ok_or_else(|| out_of_bounds("row-major-aref", index))
}

fn array_row_major_index(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("array-row-major-index", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-row-major-index", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "array-row-major-index",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    Ok(Value::Integer(
        array_coordinate_index("array-row-major-index", &dimensions, &arguments[1..])? as i64,
    ))
}

fn array_in_bounds_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("array-in-bounds-p", "array and subscripts", 0));
    }
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-in-bounds-p", "array", &arguments[0]))?;
    if arguments.len() != dimensions.len() + 1 {
        return Err(arity(
            "array-in-bounds-p",
            (dimensions.len() + 1).to_string(),
            arguments.len(),
        ));
    }
    for (dimension, value) in dimensions.iter().zip(&arguments[1..]) {
        if index_argument("array-in-bounds-p", value)? >= *dimension {
            return Ok(Value::Nil);
        }
    }
    Ok(Value::boolean(true))
}

fn array_element_type(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-element-type", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-element-type", "array", &arguments[0]))?;
    Ok(arguments[0]
        .array_element_type_value()
        .expect("array values carry element type"))
}

fn array_has_fill_pointer_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-has-fill-pointer-p", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-has-fill-pointer-p", "array", &arguments[0]))?;
    Ok(Value::boolean(array_has_fill_pointer_value(&arguments[0])))
}

fn adjustable_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "adjustable-array-p", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("adjustable-array-p", "array", &arguments[0]))?;
    Ok(Value::boolean(arguments[0].is_adjustable_array()))
}

fn array_displacement(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-displacement", 1)?;
    dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-displacement", "array", &arguments[0]))?;
    if let Some((displaced_to, displaced_index_offset)) = arguments[0].array_displacement_value() {
        Ok(Value::values(vec![
            displaced_to,
            Value::Integer(displaced_index_offset as i64),
        ]))
    } else {
        Ok(Value::values(vec![Value::Nil, Value::Integer(0)]))
    }
}

fn simple_array_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "simple-array-p", 1)?;
    Ok(Value::boolean(simple_array_value(&arguments[0])))
}

fn arrayp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "arrayp", 1)?;
    Ok(Value::boolean(
        dimensions_for_array(&arguments[0]).is_some(),
    ))
}

fn array_rank(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-rank", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-rank", "array", &arguments[0]))?;
    Ok(Value::Integer(dimensions.len() as i64))
}

fn array_dimensions(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimensions", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimensions", "array", &arguments[0]))?;
    Ok(Value::list(
        dimensions
            .into_iter()
            .map(|dimension| Value::Integer(dimension as i64))
            .collect(),
    ))
}

fn array_dimension(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-dimension", 2)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-dimension", "array", &arguments[0]))?;
    let index = index_argument("array-dimension", &arguments[1])?;
    dimensions
        .get(index)
        .copied()
        .map(|dimension| Value::Integer(dimension as i64))
        .ok_or_else(|| out_of_bounds("array-dimension", index))
}

fn array_total_size(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "array-total-size", 1)?;
    let dimensions = dimensions_for_array(&arguments[0])
        .ok_or_else(|| type_error("array-total-size", "array", &arguments[0]))?;
    Ok(Value::Integer(
        array_total_size_for("array-total-size", &dimensions)? as i64,
    ))
}

fn make_hash_table(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() % 2 != 0 {
        return Err(arity(
            "make-hash-table",
            "keyword/value pairs",
            arguments.len(),
        ));
    }
    let mut test = "EQL".to_string();
    for pair in arguments.chunks_exact(2) {
        let name = hash_table_option_name("make-hash-table", &pair[0])?;
        match name.as_str() {
            "TEST" => test = hash_table_test_name("make-hash-table", &pair[1])?,
            "SIZE" => {
                index_argument("make-hash-table", &pair[1])?;
            }
            "REHASH-SIZE" => {
                let value = number_argument("make-hash-table", &pair[1])?;
                if value.as_float() <= 0.0 {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-size must be positive".to_string(),
                        span: None,
                    });
                }
            }
            "REHASH-THRESHOLD" => {
                let value = number_argument("make-hash-table", &pair[1])?.as_float();
                if !(0.0..=1.0).contains(&value) {
                    return Err(RuntimeError::InvalidForm {
                        message: "make-hash-table :rehash-threshold must be between 0 and 1"
                            .to_string(),
                        span: None,
                    });
                }
            }
            "SYNCHRONIZED" => {
                if !matches!(pair[1], Value::Nil | Value::Boolean(_)) {
                    return Err(type_error(
                        "make-hash-table",
                        "boolean for :synchronized",
                        &pair[1],
                    ));
                }
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("make-hash-table does not support keyword :{name}"),
                    span: None,
                });
            }
        }
    }
    Ok(Value::hash_table(test))
}

fn gethash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 2 && arguments.len() != 3 {
        return Err(arity("gethash", "two or three", arguments.len()));
    }
    let table = &arguments[1];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("gethash", "hash-table", table));
    };
    let test = test.to_string();
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("gethash", "hash-table", table));
    };
    let key = &arguments[0];
    let found = entries
        .borrow()
        .iter()
        .find(|(stored_key, _)| hash_table_key_equal(&test, stored_key, key))
        .map(|(_, value)| value.clone());
    match found {
        Some(value) => Ok(Value::values(vec![value, Value::boolean(true)])),
        None => Ok(Value::values(vec![
            arguments.get(2).cloned().unwrap_or(Value::Nil),
            Value::Nil,
        ])),
    }
}

fn remhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "remhash", 2)?;
    let table = &arguments[1];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("remhash", "hash-table", table));
    };
    let test = test.to_string();
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("remhash", "hash-table", table));
    };
    let key = &arguments[0];
    let mut entries = entries.borrow_mut();
    let previous_length = entries.len();
    entries.retain(|(stored_key, _)| !hash_table_key_equal(&test, stored_key, key));
    Ok(Value::boolean(entries.len() != previous_length))
}

fn clrhash(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "clrhash", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("clrhash", "hash-table", table));
    };
    entries.borrow_mut().clear();
    Ok(table.clone())
}

fn hash_table_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-p", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::HashTable { .. }
    )))
}

fn hash_table_count(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-count", 1)?;
    let table = &arguments[0];
    let Some(entries) = table.hash_table_entries() else {
        return Err(type_error("hash-table-count", "hash-table", table));
    };
    Ok(Value::Integer(entries.borrow().len() as i64))
}

fn hash_table_test_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "hash-table-test", 1)?;
    let table = &arguments[0];
    let Some(test) = table.hash_table_test() else {
        return Err(type_error("hash-table-test", "hash-table", table));
    };
    Ok(Value::symbol(test))
}

fn hash_table_option_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => Ok(normalize_name(name)),
        other => Err(type_error(function, "keyword", other)),
    }
}

fn hash_table_test_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    let name = match value {
        Value::Keyword(name)
        | Value::Symbol(name)
        | Value::UninternedSymbol(name)
        | Value::SymbolExact(name)
        | Value::KeywordExact(name) => normalize_name(name),
        Value::Function(function_value) => match function_value.as_ref() {
            Function::Builtin { name, .. } | Function::Primitive { name } => normalize_name(name),
            _ => {
                return Err(type_error(
                    function,
                    "named hash-table test function",
                    value,
                ));
            }
        },
        other => return Err(type_error(function, "hash-table test designator", other)),
    };
    if matches!(name.as_str(), "EQ" | "EQL" | "EQUAL" | "EQUALP") {
        Ok(name)
    } else {
        Err(RuntimeError::InvalidForm {
            message: format!("{function} :test must be EQ, EQL, EQUAL, or EQUALP, got {name}"),
            span: None,
        })
    }
}

pub(crate) fn hash_table_key_equal(test: &str, left: &Value, right: &Value) -> bool {
    match test {
        "EQ" => left.eq_value(right),
        "EQUAL" => left.equal_value(right),
        "EQUALP" => equalp_value(left, right),
        _ => eql_value(left, right),
    }
}

fn parse_array_dimensions(function: &str, value: &Value) -> Result<Vec<usize>, RuntimeError> {
    match value {
        Value::Integer(_) => Ok(vec![index_argument(function, value)?]),
        Value::Nil => Ok(Vec::new()),
        Value::List(_) | Value::Vector { .. } => {
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
    dimensions.iter().try_fold(1_usize, |total, dimension| {
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
        Value::Vector { .. } => Some(vec![value.vector_length().expect("vector length")]),
        Value::Array { dimensions, .. } => Some(dimensions.as_ref().clone()),
        _ => None,
    }
}

fn array_elements(value: &Value) -> Option<Vec<Value>> {
    value.vector_items().or_else(|| value.array_items())
}

fn array_has_fill_pointer_value(value: &Value) -> bool {
    value.vector_fill_pointer().is_some()
}

fn simple_array_value(value: &Value) -> bool {
    match value {
        Value::Vector { .. } => {
            !array_has_fill_pointer_value(value)
                && !value.is_adjustable_array()
                && value.array_displacement_value().is_none()
        }
        Value::Array { .. } => {
            !value.is_adjustable_array() && value.array_displacement_value().is_none()
        }
        _ => false,
    }
}

fn simple_bit_array_value(value: &Value) -> bool {
    simple_array_value(value)
        && matches!(
            value.array_element_type_value(),
            Some(Value::Symbol(type_name)) if type_name.as_ref() == "BIT"
        )
}

fn displaced_array_arguments(
    function: &str,
    dimensions: &[usize],
    displaced_to: Option<Value>,
    displaced_index_offset: Option<Value>,
) -> Result<Option<(Option<Value>, usize, Rc<RefCell<Vec<Value>>>)>, RuntimeError> {
    match displaced_to {
        Some(displaced_to) => {
            dimensions_for_array(&displaced_to)
                .ok_or_else(|| type_error(function, "array", &displaced_to))?;
            let displaced_index_offset = match displaced_index_offset {
                Some(value) => index_argument(function, &value)?,
                None => 0,
            };
            let total_size = array_total_size_for(function, dimensions)?;
            let effective_offset = displaced_to
                .array_displacement_value()
                .map(|(_, offset)| offset)
                .unwrap_or(0)
                .checked_add(displaced_index_offset)
                .ok_or_else(|| RuntimeError::InvalidForm {
                    message: format!("{function} displacement is too large"),
                    span: None,
                })?;
            let source_storage = displaced_to
                .array_storage()
                .expect("array values carry shared storage");
            let source_size = source_storage.borrow().len();
            let end = effective_offset.checked_add(total_size).ok_or_else(|| {
                RuntimeError::InvalidForm {
                    message: format!("{function} displacement is too large"),
                    span: None,
                }
            })?;
            if end > source_size {
                return Err(RuntimeError::InvalidForm {
                    message: format!(
                        "{function} displacement range {}..{} is out of bounds for source size {}",
                        effective_offset, end, source_size
                    ),
                    span: None,
                });
            }
            Ok(Some((Some(displaced_to), effective_offset, source_storage)))
        }
        None => {
            if displaced_index_offset.is_some() {
                return Err(RuntimeError::InvalidForm {
                    message: format!("{function} :displaced-index-offset requires :displaced-to"),
                    span: None,
                });
            }
            Ok(None)
        }
    }
}

fn array_fill_pointer(function: &str, value: &Value, length: usize) -> Result<usize, RuntimeError> {
    if value
        .symbol_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("T"))
    {
        return Ok(length);
    }
    let fill_pointer = index_argument(function, value)?;
    if fill_pointer > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} :fill-pointer {fill_pointer} is out of bounds"),
            span: None,
        });
    }
    Ok(fill_pointer)
}

fn sequence_items(value: &Value) -> Option<Vec<Value>> {
    value.list_items().or_else(|| value.vector_items())
}

fn null(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "null", 1)?;
    Ok(Value::boolean(!arguments[0].is_truthy()))
}

fn atom(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "atom", 1)?;
    Ok(Value::boolean(!matches!(
        &arguments[0],
        Value::List(_) | Value::DottedList { .. }
    )))
}

fn consp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "consp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::List(_) | Value::DottedList { .. }
    )))
}

fn listp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "listp", 1)?;
    Ok(Value::boolean(matches!(
        &arguments[0],
        Value::Nil | Value::List(_)
    )))
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

fn complex(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "complex", 2)?;
    Ok(Value::complex(
        real_number_argument("complex", &arguments[0])?,
        real_number_argument("complex", &arguments[1])?,
    ))
}

fn conjugate(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "conjugate", 1)?;
    match numeric_argument("conjugate", &arguments[0])? {
        Numeric::Real(value) => number_to_value(value),
        Numeric::Complex { real, imag } => Ok(Value::complex(
            number_to_value(real)?,
            number_to_value(negate_number(imag)?)?,
        )),
    }
}

fn phase(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "phase", 1)?;
    match numeric_argument("phase", &arguments[0])? {
        Numeric::Real(value) => phase_real(value),
        Numeric::Complex { real, imag } => phase_complex(real, imag),
    }
}

fn phase_real(value: Number) -> Result<Value, RuntimeError> {
    let as_float = value.as_float();
    if as_float == 0.0 {
        return number_to_value(match value {
            Number::Float(_) => Number::Float(0.0),
            _ => Number::Integer(0),
        });
    }
    if as_float.is_sign_negative() {
        Ok(Value::Float(PI))
    } else {
        number_to_value(match value {
            Number::Float(_) => Number::Float(0.0),
            _ => Number::Integer(0),
        })
    }
}

fn phase_complex(real: Number, imag: Number) -> Result<Value, RuntimeError> {
    if real.as_float() == 0.0 && imag.as_float() == 0.0 {
        return Ok(Value::Integer(0));
    }
    Ok(Value::Float(imag.as_float().atan2(real.as_float())))
}

fn realpart(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "realpart", 1)?;
    match &arguments[0] {
        Value::Complex { real, .. } => Ok(real.as_ref().clone()),
        value if is_real_number(value) => Ok(value.clone()),
        value => Err(number_error("realpart", value)),
    }
}

fn imagpart(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "imagpart", 1)?;
    match &arguments[0] {
        Value::Complex { imag, .. } => Ok(imag.as_ref().clone()),
        Value::Float(_) => Ok(Value::Float(0.0)),
        value if is_real_number(value) => Ok(Value::Integer(0)),
        value => Err(number_error("imagpart", value)),
    }
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
        (
            Value::Vector {
                fill_pointer: left_fill_pointer,
                element_type: left_element_type,
                adjustable: left_adjustable,
                displaced_to: left_displaced_to,
                displaced_index_offset: left_displaced_index_offset,
                ..
            },
            Value::Vector {
                fill_pointer: right_fill_pointer,
                element_type: right_element_type,
                adjustable: right_adjustable,
                displaced_to: right_displaced_to,
                displaced_index_offset: right_displaced_index_offset,
                ..
            },
        ) => {
            let left = left.vector_items().expect("vector items");
            let right = right.vector_items().expect("vector items");
            left_fill_pointer == right_fill_pointer
                && left_adjustable == right_adjustable
                && left_element_type.equal_value(right_element_type)
                && left_displaced_index_offset == right_displaced_index_offset
                && match (left_displaced_to, right_displaced_to) {
                    (Some(left), Some(right)) => equalp_value(left, right),
                    (None, None) => true,
                    _ => false,
                }
                && left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equalp_value(left, right))
        }
        (
            Value::Array {
                dimensions: left_dimensions,
                element_type: left_element_type,
                adjustable: left_adjustable,
                displaced_to: left_displaced_to,
                displaced_index_offset: left_displaced_index_offset,
                ..
            },
            Value::Array {
                dimensions: right_dimensions,
                element_type: right_element_type,
                adjustable: right_adjustable,
                displaced_to: right_displaced_to,
                displaced_index_offset: right_displaced_index_offset,
                ..
            },
        ) => {
            let left_elements = left.array_items().expect("array items");
            let right_elements = right.array_items().expect("array items");
            left_dimensions == right_dimensions
                && left_adjustable == right_adjustable
                && left_element_type.equal_value(right_element_type)
                && left_displaced_index_offset == right_displaced_index_offset
                && match (left_displaced_to, right_displaced_to) {
                    (Some(left), Some(right)) => equalp_value(left, right),
                    (None, None) => true,
                    _ => false,
                }
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
    Ok(Value::symbol(
        arguments[0]
            .structure_name()
            .unwrap_or(arguments[0].type_name()),
    ))
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
        Value::Vector { .. } => {
            let values = value.vector_items().expect("vector items");
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
    let mut reader = Reader::new(&window);
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
    read_stream_form("read", arguments, false)
}

fn read_preserving_whitespace(arguments: &[Value]) -> Result<Value, RuntimeError> {
    read_stream_form("read-preserving-whitespace", arguments, true)
}

fn read_stream_form(
    function: &str,
    arguments: &[Value],
    preserving_whitespace: bool,
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
    let mut reader = Reader::new(&source);
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

fn make_string_input_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=3).contains(&arguments.len()) {
        return Err(arity("make-string-input-stream", "1 to 3", arguments.len()));
    }
    let source = match &arguments[0] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("make-string-input-stream", "a string", value)),
    };
    let length = source.chars().count();
    let start = match arguments.get(1) {
        Some(value) => stream_bound("make-string-input-stream", value, length)?,
        None => 0,
    };
    let end = match arguments.get(2) {
        Some(value) => stream_bound("make-string-input-stream", value, length)?,
        None => length,
    };
    if start > end {
        return Err(RuntimeError::InvalidForm {
            message: "make-string-input-stream start must not exceed end".to_string(),
            span: None,
        });
    }
    Ok(Value::string_input_stream(source, start, end))
}

fn stream_input_position(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "%stream-input-position", 1)?;
    let stream = input_stream_reference("%stream-input-position", arguments.first())?;
    let position = {
        let stream = stream.borrow();
        if !stream.is_input() {
            return Err(stream_state_error(
                "%stream-input-position",
                "an input stream",
            ));
        }
        stream
            .input_position()
            .ok_or_else(|| stream_state_error("%stream-input-position", "an open input stream"))?
    };
    let position = i64::try_from(position).map_err(|_| RuntimeError::NumericOverflow)?;
    Ok(Value::Integer(position))
}

fn stream_bound(function: &str, value: &Value, length: usize) -> Result<usize, RuntimeError> {
    let bound = integer_argument(function, value)?;
    let bound = usize::try_from(bound).map_err(|_| RuntimeError::InvalidForm {
        message: format!("{function} stream position must be non-negative"),
        span: None,
    })?;
    if bound > length {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} stream position is outside the string"),
            span: None,
        });
    }
    Ok(bound)
}

fn make_string_output_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "make-string-output-stream", 0)?;
    Ok(Value::string_output_stream())
}

fn pathname_argument(function: &str, value: &Value) -> Result<PathBuf, RuntimeError> {
    match value {
        Value::String(value) => Ok(PathBuf::from(value.as_ref())),
        value => Err(type_error(function, "a string pathname", value)),
    }
}

fn open_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.is_empty() {
        return Err(arity("open", "at least 1", arguments.len()));
    }
    if (arguments.len() - 1) % 2 != 0 {
        return Err(RuntimeError::InvalidForm {
            message: "open requires keyword/value pairs after the pathname".to_string(),
            span: None,
        });
    }
    let path = pathname_argument("open", &arguments[0])?;
    let mut direction = "INPUT".to_string();
    let mut if_does_not_exist = None;
    let mut if_exists = None;
    for pair in arguments[1..].chunks_exact(2) {
        let keyword = stream_keyword_name("open", &pair[0])?;
        match keyword.as_str() {
            "DIRECTION" => {
                direction = stream_keyword_name("open :direction", &pair[1])?;
            }
            "IF-DOES-NOT-EXIST" => {
                if_does_not_exist = Some(stream_keyword_name("open :if-does-not-exist", &pair[1])?);
            }
            "IF-EXISTS" => {
                if_exists = Some(stream_keyword_name("open :if-exists", &pair[1])?);
            }
            "ELEMENT-TYPE" | "EXTERNAL-FORMAT" => {}
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open does not recognize keyword :{keyword}"),
                    span: None,
                });
            }
        }
    }

    let if_does_not_exist = if_does_not_exist.unwrap_or_else(|| {
        if direction == "INPUT" || direction == "IO" {
            "ERROR".to_string()
        } else {
            "CREATE".to_string()
        }
    });
    let if_exists = if_exists.unwrap_or_else(|| "NEW-VERSION".to_string());
    match direction.as_str() {
        "INPUT" => open_input_file(&path, &if_does_not_exist),
        "OUTPUT" => open_output_file(&path, &if_does_not_exist, &if_exists),
        "PROBE" => {
            if path.exists() {
                Ok(Value::file_input_stream(String::new()))
            } else {
                Ok(Value::Nil)
            }
        }
        "IO" => open_io_file(&path, &if_does_not_exist, &if_exists),
        _ => Err(RuntimeError::InvalidForm {
            message: format!("open received unknown direction :{direction}"),
            span: None,
        }),
    }
}

fn probe_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "probe-file", 1)?;
    let path = pathname_argument("probe-file", &arguments[0])?;
    match std::fs::metadata(&path) {
        Ok(_) => Ok(arguments[0].clone()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::Nil),
        Err(error) => Err(RuntimeError::Io(format!(
            "probe-file {}: {error}",
            path.display()
        ))),
    }
}

fn delete_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "delete-file", 1)?;
    let path = pathname_argument("delete-file", &arguments[0])?;
    std::fs::remove_file(&path)
        .map_err(|error| RuntimeError::Io(format!("delete-file {}: {error}", path.display())))?;
    Ok(Value::boolean(true))
}

fn rename_file(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "rename-file", 2)?;
    let old_path = pathname_argument("rename-file", &arguments[0])?;
    let new_path = pathname_argument("rename-file", &arguments[1])?;
    let old_truename = std::fs::canonicalize(&old_path).map_err(|error| {
        RuntimeError::Io(format!("rename-file {}: {error}", old_path.display()))
    })?;
    std::fs::rename(&old_path, &new_path).map_err(|error| {
        RuntimeError::Io(format!(
            "rename-file {} to {}: {error}",
            old_path.display(),
            new_path.display()
        ))
    })?;
    let new_truename = std::fs::canonicalize(&new_path).map_err(|error| {
        RuntimeError::Io(format!("rename-file {}: {error}", new_path.display()))
    })?;
    Ok(Value::values(vec![
        arguments[1].clone(),
        Value::string(old_truename.to_string_lossy().to_string()),
        Value::string(new_truename.to_string_lossy().to_string()),
    ]))
}

fn file_write_date(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "file-write-date", 1)?;
    let path = pathname_argument("file-write-date", &arguments[0])?;
    let modified = std::fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| {
            RuntimeError::Io(format!("file-write-date {}: {error}", path.display()))
        })?;
    let seconds_since_unix = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| {
            RuntimeError::Io(format!("file-write-date {}: {error}", path.display()))
        })?;
    let seconds_since_unix = i64::try_from(seconds_since_unix.as_secs()).map_err(|_| {
        RuntimeError::Io(format!(
            "file-write-date {}: modification time is out of range",
            path.display()
        ))
    })?;
    let universal_time = seconds_since_unix
        .checked_add(2_208_988_800)
        .ok_or_else(|| {
            RuntimeError::Io(format!(
                "file-write-date {}: modification time is out of range",
                path.display()
            ))
        })?;
    Ok(Value::Integer(universal_time))
}

fn truename(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "truename", 1)?;
    let path = pathname_argument("truename", &arguments[0])?;
    let canonical = std::fs::canonicalize(&path)
        .map_err(|error| RuntimeError::Io(format!("truename {}: {error}", path.display())))?;
    Ok(Value::string(canonical.to_string_lossy().to_string()))
}

fn stream_keyword_name(function: &str, value: &Value) -> Result<String, RuntimeError> {
    match value {
        Value::Keyword(name) | Value::KeywordExact(name) => Ok(normalize_name(name)),
        value => Err(type_error(function, "a keyword", value)),
    }
}

fn open_input_file(path: &std::path::Path, if_does_not_exist: &str) -> Result<Value, RuntimeError> {
    if !path.exists() {
        match if_does_not_exist {
            "NIL" => return Ok(Value::Nil),
            "CREATE" => {
                std::fs::write(path, []).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?;
            }
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file does not exist",
                    path.display()
                )));
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!(
                        "open received unknown :if-does-not-exist value :{if_does_not_exist}"
                    ),
                    span: None,
                });
            }
        }
    }
    let source = std::fs::read_to_string(path)
        .map_err(|error| RuntimeError::Io(format!("open {}: {error}", path.display())))?;
    Ok(Value::file_input_stream(source))
}

fn open_output_file(
    path: &std::path::Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
    if path.exists() {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file already exists",
                    path.display()
                )));
            }
            "APPEND" => {
                let source = std::fs::read_to_string(path).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?;
                return Ok(Value::file_output_stream(path.to_path_buf(), source));
            }
            "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" | "SUPERSEDE" => {}
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open received unknown :if-exists value :{if_exists}"),
                    span: None,
                });
            }
        }
    } else {
        match if_does_not_exist {
            "CREATE" => {}
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file does not exist",
                    path.display()
                )));
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!(
                        "open received unknown :if-does-not-exist value :{if_does_not_exist}"
                    ),
                    span: None,
                });
            }
        }
    }
    Ok(Value::file_output_stream(path.to_path_buf(), String::new()))
}

fn open_io_file(
    path: &std::path::Path,
    if_does_not_exist: &str,
    if_exists: &str,
) -> Result<Value, RuntimeError> {
    let mut append = false;
    let source = if path.exists() {
        match if_exists {
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file already exists",
                    path.display()
                )));
            }
            "APPEND" => {
                append = true;
                std::fs::read_to_string(path).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?
            }
            "NEW-VERSION" | "RENAME" | "RENAME-AND-DELETE" | "OVERWRITE" | "SUPERSEDE" => {
                std::fs::read_to_string(path).map_err(|error| {
                    RuntimeError::Io(format!("open {}: {error}", path.display()))
                })?
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("open received unknown :if-exists value :{if_exists}"),
                    span: None,
                });
            }
        }
    } else {
        match if_does_not_exist {
            "CREATE" => String::new(),
            "NIL" => return Ok(Value::Nil),
            "ERROR" => {
                return Err(RuntimeError::Io(format!(
                    "open {}: file does not exist",
                    path.display()
                )));
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!(
                        "open received unknown :if-does-not-exist value :{if_does_not_exist}"
                    ),
                    span: None,
                });
            }
        }
    };
    Ok(Value::file_io_stream(path.to_path_buf(), source, append))
}

fn stream_reference<'a>(
    function: &str,
    value: &'a Value,
) -> Result<&'a Rc<RefCell<Stream>>, RuntimeError> {
    match value {
        Value::Stream(stream) => Ok(stream),
        value => Err(type_error(function, "a stream", value)),
    }
}

fn input_stream_reference<'a>(
    function: &str,
    value: Option<&'a Value>,
) -> Result<&'a Rc<RefCell<Stream>>, RuntimeError> {
    match value {
        Some(Value::Stream(stream)) => Ok(stream),
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} requires an explicit input stream; standard input is unavailable"
            ),
            span: None,
        }),
        Some(value) => Err(type_error(function, "an input stream", value)),
    }
}

fn stream_state_error(function: &str, expected: &str) -> RuntimeError {
    RuntimeError::InvalidForm {
        message: format!("{function} requires {expected}"),
        span: None,
    }
}

fn end_of_file_error(context: &'static str) -> RuntimeError {
    RuntimeError::Read(ReadError::new(
        ReadErrorKind::UnexpectedEnd { context },
        Span::new(0, 0),
    ))
}

fn peek_character(
    stream: &mut Stream,
    peek_type: Option<&Value>,
) -> Result<Option<char>, RuntimeError> {
    match peek_type {
        None
        | Some(Value::Nil)
        | Some(Value::Boolean(false))
        | Some(Value::Boolean(true))
        | Some(Value::Character(_)) => {}
        Some(value) => return Err(type_error("peek-char", "NIL, T, or a character", value)),
    }

    loop {
        let Some(character) = stream.peek_char() else {
            return Ok(None);
        };
        let matches = match peek_type {
            None | Some(Value::Nil) | Some(Value::Boolean(false)) => true,
            Some(Value::Boolean(true)) => !character.is_whitespace(),
            Some(Value::Character(expected)) => character == *expected,
            Some(_) => unreachable!("peek-char type was validated above"),
        };
        if matches {
            return Ok(Some(character));
        }
        let _ = stream.read_char();
    }
}

fn get_output_stream_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "get-output-stream-string", 1)?;
    let stream = stream_reference("get-output-stream-string", &arguments[0])?;
    let output = stream
        .borrow_mut()
        .take_output()
        .ok_or_else(|| stream_state_error("get-output-stream-string", "an output stream"))?;
    Ok(Value::string(output))
}

fn append_output_to_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "__ncl_append_output_to_string", 2)?;
    let Value::Vector {
        elements,
        fill_pointer: Some(fill_pointer),
        element_type,
        adjustable,
        ..
    } = &arguments[0]
    else {
        return Err(type_error(
            "__ncl_append_output_to_string",
            "vector with fill pointer",
            &arguments[0],
        ));
    };

    let mut combined = Vec::with_capacity(*fill_pointer);
    for item in elements.borrow().iter().take(*fill_pointer) {
        let Value::Character(_) = item else {
            return Err(type_error(
                "__ncl_append_output_to_string",
                "characters in vector with fill pointer",
                &item,
            ));
        };
        combined.push(item.clone());
    }

    let Value::String(output) = &arguments[1] else {
        return Err(type_error(
            "__ncl_append_output_to_string",
            "string",
            &arguments[1],
        ));
    };
    combined.extend(output.chars().map(Value::Character));
    let new_fill_pointer = combined.len();
    Ok(Value::vector_with_fill_pointer_element_type_and_adjustable(
        combined,
        Some(new_fill_pointer),
        element_type.as_ref().clone(),
        *adjustable,
    ))
}

fn read_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity("read-char", "0 to 4", arguments.len()));
    }
    let stream = input_stream_reference("read-char", arguments.first())?;
    let eof_error_p = arguments.get(1).map_or(true, Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-char", "an input stream"));
    }
    match stream.read_char() {
        Some(character) => Ok(Value::Character(character)),
        None if eof_error_p => Err(end_of_file_error("a character")),
        None => Ok(eof_value),
    }
}

fn peek_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 5 {
        return Err(arity("peek-char", "0 to 5", arguments.len()));
    }
    let (peek_type, stream_value, optional_index) =
        if matches!(arguments.first(), Some(Value::Stream(_))) {
            (None, arguments.first(), 1)
        } else {
            (arguments.first(), arguments.get(1), 2)
        };
    let stream = input_stream_reference("peek-char", stream_value)?;
    let eof_error_p = arguments.get(optional_index).map_or(true, Value::is_truthy);
    let eof_value = arguments
        .get(optional_index + 1)
        .cloned()
        .unwrap_or(Value::Nil);
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("peek-char", "an input stream"));
    }
    match peek_character(&mut stream, peek_type)? {
        Some(character) => Ok(Value::Character(character)),
        None if eof_error_p => Err(end_of_file_error("a character")),
        None => Ok(eof_value),
    }
}

fn unread_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("unread-char", "1 to 2", arguments.len()));
    }
    let character = match arguments[0] {
        Value::Character(character) => character,
        ref value => return Err(type_error("unread-char", "a character", value)),
    };
    let stream = input_stream_reference("unread-char", arguments.get(1))?;
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("unread-char", "an input stream"));
    }
    if !stream.unread_char(character) {
        return Err(stream_state_error(
            "unread-char",
            "the last character read from an open input stream",
        ));
    }
    Ok(Value::Nil)
}

fn read_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 4 {
        return Err(arity("read-line", "0 to 4", arguments.len()));
    }
    let stream = input_stream_reference("read-line", arguments.first())?;
    let eof_error_p = arguments.get(1).map_or(true, Value::is_truthy);
    let eof_value = arguments.get(2).cloned().unwrap_or(Value::Nil);
    let mut stream = stream.borrow_mut();
    if !stream.is_input() {
        return Err(stream_state_error("read-line", "an input stream"));
    }
    match stream.read_line() {
        Some((line, eof)) => Ok(Value::values(vec![
            Value::string(line),
            Value::boolean(eof),
        ])),
        None if eof_error_p => Err(end_of_file_error("a line")),
        None => Ok(Value::values(vec![eof_value, Value::boolean(true)])),
    }
}

fn write_destination(
    function: &str,
    destination: Option<&Value>,
    text: &str,
) -> Result<(), RuntimeError> {
    match destination {
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
            print!("{text}");
            Ok(())
        }
        Some(Value::Stream(stream)) => {
            if stream.borrow_mut().write(text) {
                Ok(())
            } else {
                Err(stream_state_error(function, "an open output stream"))
            }
        }
        Some(value) => Err(type_error(function, "NIL, T, or an output stream", value)),
    }
}

fn write_char(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-char", "1 to 2", arguments.len()));
    }
    let character = match arguments[0] {
        Value::Character(character) => character,
        ref value => return Err(type_error("write-char", "a character", value)),
    };
    write_destination("write-char", arguments.get(1), &character.to_string())?;
    Ok(Value::Character(character))
}

fn write_string(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-string", "1 to 2", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-string", "a string", value)),
    };
    write_destination("write-string", arguments.get(1), string)?;
    Ok(arguments[0].clone())
}

fn terpri(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("terpri", "0 to 1", arguments.len()));
    }
    write_destination("terpri", arguments.first(), "\n")?;
    Ok(Value::Nil)
}

fn fresh_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() > 1 {
        return Err(arity("fresh-line", "0 to 1", arguments.len()));
    }
    match arguments.first() {
        None | Some(Value::Nil) | Some(Value::Boolean(true)) => {
            println!();
            Ok(Value::boolean(true))
        }
        Some(Value::Stream(stream)) => stream
            .borrow_mut()
            .fresh_line()
            .map(Value::boolean)
            .ok_or_else(|| stream_state_error("fresh-line", "an open output stream")),
        Some(value) => Err(type_error(
            "fresh-line",
            "NIL, T, or an output stream",
            value,
        )),
    }
}

fn write_line(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&arguments.len()) {
        return Err(arity("write-line", "1 to 2", arguments.len()));
    }
    let string = match &arguments[0] {
        Value::String(value) => value,
        value => return Err(type_error("write-line", "a string", value)),
    };
    let mut line = String::with_capacity(string.len() + 1);
    line.push_str(string);
    line.push('\n');
    write_destination("write-line", arguments.get(1), &line)?;
    Ok(arguments[0].clone())
}

fn close_stream(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() != 1 && arguments.len() != 3 {
        return Err(arity("close", "1 or 3", arguments.len()));
    }
    let abort = if arguments.len() == 3 {
        if stream_keyword_name("close :abort", &arguments[1])? != "ABORT" {
            return Err(RuntimeError::InvalidForm {
                message: "close accepts only the :abort keyword".to_string(),
                span: None,
            });
        }
        arguments[2].is_truthy()
    } else {
        false
    };
    let stream = stream_reference("close", &arguments[0])?;
    stream
        .borrow_mut()
        .close(abort)
        .map_err(|error| RuntimeError::Io(format!("close: {error}")))?;
    Ok(Value::boolean(true))
}

fn streamp(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "streamp", 1)?;
    Ok(Value::boolean(matches!(&arguments[0], Value::Stream(_))))
}

fn input_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "input-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_input(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

fn output_stream_p(arguments: &[Value]) -> Result<Value, RuntimeError> {
    exact(arguments, "output-stream-p", 1)?;
    let result = match &arguments[0] {
        Value::Stream(stream) => stream.borrow().is_output(),
        _ => false,
    };
    Ok(Value::boolean(result))
}

fn format_value(arguments: &[Value]) -> Result<Value, RuntimeError> {
    if arguments.len() < 2 {
        return Err(arity("format", "at least 2", arguments.len()));
    }
    let control = match &arguments[1] {
        Value::String(value) => value.as_ref(),
        value => return Err(type_error("format", "a string control", value)),
    };
    let output = format_control(control, &arguments[2..])?;
    match &arguments[0] {
        Value::Nil => Ok(Value::string(output)),
        Value::Boolean(true) => {
            print!("{output}");
            Ok(Value::Nil)
        }
        Value::Stream(stream) => {
            if stream.borrow_mut().write(&output) {
                Ok(Value::Nil)
            } else {
                Err(stream_state_error("format", "an open output stream"))
            }
        }
        value => Err(type_error("format", "NIL or T as the destination", value)),
    }
}

pub(crate) fn format_control(control: &str, arguments: &[Value]) -> Result<String, RuntimeError> {
    let characters = control.chars().collect::<Vec<_>>();
    let (output, _, _) = format_control_characters(&characters, arguments, false)?;
    Ok(output)
}

#[derive(Clone, Copy)]
enum FormatParameter {
    Missing,
    Number(i64),
    Character(char),
}

#[derive(Clone, Copy)]
struct FormatTermination {
    colon_modifier: bool,
}

fn parse_format_parameters(
    characters: &[char],
    character_index: &mut usize,
    arguments: &[Value],
    argument_index: &mut usize,
) -> Result<Vec<FormatParameter>, RuntimeError> {
    let mut parameters = Vec::new();
    let mut current_parameter = None;
    let mut comma_seen = false;
    while *character_index < characters.len() {
        match characters[*character_index] {
            ',' => {
                parameters.push(current_parameter.take().unwrap_or(FormatParameter::Missing));
                comma_seen = true;
                *character_index += 1;
            }
            '\'' => {
                *character_index += 1;
                let character = characters.get(*character_index).copied().ok_or_else(|| {
                    RuntimeError::InvalidForm {
                        message: "format character parameter is missing its character".to_string(),
                        span: None,
                    }
                })?;
                current_parameter = Some(FormatParameter::Character(character));
                comma_seen = false;
                *character_index += 1;
            }
            '#' => {
                *character_index += 1;
                let remaining = arguments.len().saturating_sub(*argument_index);
                let remaining = i64::try_from(remaining).unwrap_or(i64::MAX);
                current_parameter = Some(FormatParameter::Number(remaining));
                comma_seen = false;
            }
            'v' | 'V' => {
                *character_index += 1;
                let argument = format_argument("format parameter", arguments, argument_index)?;
                current_parameter = Some(FormatParameter::Number(integer_argument(
                    "format parameter",
                    argument,
                )?));
                comma_seen = false;
            }
            '-' | '0'..='9' => {
                let start = *character_index;
                if characters[*character_index] == '-' {
                    *character_index += 1;
                }
                let digit_start = *character_index;
                while *character_index < characters.len()
                    && characters[*character_index].is_ascii_digit()
                {
                    *character_index += 1;
                }
                if digit_start == *character_index {
                    return Err(RuntimeError::InvalidForm {
                        message: "format numeric parameter needs digits".to_string(),
                        span: None,
                    });
                }
                let text = characters[start..*character_index]
                    .iter()
                    .collect::<String>();
                let value = text.parse::<i64>().map_err(|_| RuntimeError::InvalidForm {
                    message: format!("format numeric parameter is out of range: {text}"),
                    span: None,
                })?;
                current_parameter = Some(FormatParameter::Number(value));
                comma_seen = false;
            }
            _ => break,
        }
    }
    if let Some(parameter) = current_parameter {
        parameters.push(parameter);
    } else if comma_seen {
        parameters.push(FormatParameter::Missing);
    }
    Ok(parameters)
}

fn format_directive_prefix(
    characters: &[char],
    start: usize,
) -> Result<(usize, bool, bool), RuntimeError> {
    let mut directive_index = start;
    while directive_index < characters.len() {
        match characters[directive_index] {
            ',' | '#' | 'v' | 'V' => directive_index += 1,
            '\'' => {
                directive_index += 1;
                if directive_index >= characters.len() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format character parameter is missing its character".to_string(),
                        span: None,
                    });
                }
                directive_index += 1;
            }
            '-' | '0'..='9' => {
                if characters[directive_index] == '-' {
                    directive_index += 1;
                }
                let digit_start = directive_index;
                while directive_index < characters.len()
                    && characters[directive_index].is_ascii_digit()
                {
                    directive_index += 1;
                }
                if digit_start == directive_index {
                    return Err(RuntimeError::InvalidForm {
                        message: "format numeric parameter needs digits".to_string(),
                        span: None,
                    });
                }
            }
            _ => break,
        }
    }
    let mut colon_modifier = false;
    let mut at_sign_modifier = false;
    while directive_index < characters.len() {
        match characters[directive_index] {
            ':' => {
                colon_modifier = true;
                directive_index += 1;
            }
            '@' => {
                at_sign_modifier = true;
                directive_index += 1;
            }
            _ => break,
        }
    }
    Ok((directive_index, colon_modifier, at_sign_modifier))
}

fn format_control_characters(
    characters: &[char],
    arguments: &[Value],
    colon_iteration_last: bool,
) -> Result<(String, usize, Option<FormatTermination>), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    let mut character_index = 0;
    while character_index < characters.len() {
        let character = characters[character_index];
        character_index += 1;
        if character != '~' {
            output.push(character);
            continue;
        }

        let parameters = parse_format_parameters(
            characters,
            &mut character_index,
            arguments,
            &mut argument_index,
        )?;
        let mut colon_modifier = false;
        let mut at_sign_modifier = false;
        while character_index < characters.len() {
            match characters[character_index] {
                ':' => {
                    colon_modifier = true;
                    character_index += 1;
                }
                '@' => {
                    at_sign_modifier = true;
                    character_index += 1;
                }
                _ => break,
            }
        }
        let directive =
            characters
                .get(character_index)
                .copied()
                .ok_or_else(|| RuntimeError::InvalidForm {
                    message: "format control ends after a tilde".to_string(),
                    span: None,
                })?;
        character_index += 1;
        let directive = directive.to_ascii_uppercase();
        let supports_modifiers = matches!(
            directive,
            '{' | '['
                | '<'
                | 'A'
                | 'S'
                | 'C'
                | 'D'
                | 'B'
                | 'O'
                | 'X'
                | 'R'
                | 'F'
                | 'E'
                | 'G'
                | 'I'
                | 'P'
                | '$'
                | '^'
                | 'T'
                | 'W'
                | '?'
                | '_'
                | '('
                | '*'
        );
        if (colon_modifier || at_sign_modifier) && !supports_modifiers {
            return Err(RuntimeError::InvalidForm {
                message: format!("unsupported format modifier before ~{directive}"),
                span: None,
            });
        }
        match directive {
            'A' => {
                let argument = format_argument("~A", arguments, &mut argument_index)?;
                let mut formatted = String::new();
                if colon_modifier && matches!(argument, Value::Nil) {
                    formatted.push_str("()");
                } else {
                    append_aesthetic(&mut formatted, argument);
                }
                output.push_str(&format_text_field(
                    formatted,
                    &parameters,
                    at_sign_modifier,
                )?);
            }
            'S' => {
                let argument = format_argument("~S", arguments, &mut argument_index)?;
                output.push_str(&format_text_field(
                    argument.to_string(),
                    &parameters,
                    at_sign_modifier,
                )?);
            }
            '(' => {
                if !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format case conversion does not accept parameters".to_string(),
                        span: None,
                    });
                }
                let body_end = format_case_conversion_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let (formatted, consumed, termination) = format_control_characters(
                    body,
                    &arguments[argument_index..],
                    colon_iteration_last,
                )?;
                output.push_str(&format_case_conversion(
                    &formatted,
                    colon_modifier,
                    at_sign_modifier,
                ));
                argument_index += consumed;
                if let Some(termination) = termination {
                    return Ok((output, argument_index, Some(termination)));
                }
            }
            'D' | 'B' | 'O' | 'X' => {
                let argument =
                    format_argument("format integer directive", arguments, &mut argument_index)?;
                let integer = integer_argument("format", argument)?;
                let radix = match directive {
                    'D' => 10,
                    'B' => 2,
                    'O' => 8,
                    'X' => 16,
                    _ => unreachable!(),
                };
                output.push_str(&format_integer_directive(
                    integer,
                    radix,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'F' => {
                let argument = format_argument("~F", arguments, &mut argument_index)?;
                let value = number_argument("format", argument)?.as_float();
                output.push_str(&format_fixed_float_directive(
                    value,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'G' => {
                let argument = format_argument("~G", arguments, &mut argument_index)?;
                let value = number_argument("format", argument)?.as_float();
                output.push_str(&format_general_float_directive(
                    value,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'E' => {
                let argument = format_argument("~E", arguments, &mut argument_index)?;
                let value = number_argument("format", argument)?.as_float();
                output.push_str(&format_exponential_float_directive(
                    value,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            '$' => {
                let argument = format_argument("~$", arguments, &mut argument_index)?;
                let value = number_argument("format", argument)?.as_float();
                output.push_str(&format_dollar_float_directive(
                    value,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'P' => {
                if !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~P does not accept parameters".to_string(),
                        span: None,
                    });
                }
                let argument = if colon_modifier {
                    let index =
                        argument_index
                            .checked_sub(1)
                            .ok_or_else(|| RuntimeError::InvalidForm {
                                message: "format ~:P has no previous argument".to_string(),
                                span: None,
                            })?;
                    arguments
                        .get(index)
                        .ok_or_else(|| RuntimeError::InvalidForm {
                            message: "format ~:P has no previous argument".to_string(),
                            span: None,
                        })?
                } else {
                    format_argument("~P", arguments, &mut argument_index)?
                };
                let value = integer_argument("format", argument)?;
                if at_sign_modifier {
                    output.push_str(if value == 1 { "y" } else { "ies" });
                } else if value == 1 {
                    output.push_str("");
                } else {
                    output.push('s');
                }
            }
            'C' => {
                let argument = format_argument("~C", arguments, &mut argument_index)?;
                let Value::Character(character) = argument else {
                    return Err(type_error("format", "a character for ~C", argument));
                };
                output.push_str(&format_character_directive(
                    *character,
                    colon_modifier,
                    at_sign_modifier,
                ));
            }
            '%' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for repetition in 0..count {
                    if repetition == 0 && (output.is_empty() || output.ends_with('\n')) {
                        continue;
                    }
                    output.push('\n');
                }
            }
            '&' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for repetition in 0..count {
                    if repetition == 0 {
                        if !output.is_empty() && !output.ends_with('\n') {
                            output.push('\n');
                        }
                    } else {
                        output.push('\n');
                    }
                }
            }
            '|' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for _ in 0..count {
                    output.push('\x0c');
                }
            }
            '~' => {
                let count = format_parameter_count(&parameters, 0, 1)?;
                for _ in 0..count {
                    output.push('~');
                }
            }
            '\n' => {
                while matches!(
                    characters.get(character_index),
                    Some(character) if character.is_whitespace()
                ) {
                    character_index += 1;
                }
            }
            '_' => {
                if !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~_ does not accept parameters".to_string(),
                        span: None,
                    });
                }
            }
            'I' => {
                if at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~I does not support the at-sign modifier".to_string(),
                        span: None,
                    });
                }
                if parameters.len() > 1 {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~I accepts at most one parameter".to_string(),
                        span: None,
                    });
                }
                let _ = format_parameter_count(&parameters, 0, 0)?;
            }
            '*' => {
                if colon_modifier && at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~* does not support using colon and at-sign together"
                            .to_string(),
                        span: None,
                    });
                }
                if colon_modifier {
                    let count = format_parameter_count(&parameters, 0, 1)?;
                    argument_index = argument_index.checked_sub(count).ok_or_else(|| {
                        RuntimeError::InvalidForm {
                            message: "format ~:* has no previous argument".to_string(),
                            span: None,
                        }
                    })?;
                } else if at_sign_modifier {
                    let count = format_parameter_count(&parameters, 0, 0)?;
                    argument_index = count.min(arguments.len());
                } else {
                    let count = format_parameter_count(&parameters, 0, 1)?;
                    argument_index = argument_index.saturating_add(count).min(arguments.len());
                }
            }
            '?' => {
                if !parameters.is_empty() || colon_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~? only supports the at-sign modifier".to_string(),
                        span: None,
                    });
                }
                let nested_control = format_argument("~?", arguments, &mut argument_index)?;
                let nested_control = match nested_control {
                    Value::String(value) => value,
                    value => return Err(type_error("format", "a string for ~?", value)),
                };
                if at_sign_modifier {
                    let nested_characters = nested_control.chars().collect::<Vec<_>>();
                    let (formatted, consumed, termination) = format_control_characters(
                        &nested_characters,
                        &arguments[argument_index..],
                        false,
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                    if let Some(termination) = termination {
                        return Ok((output, argument_index, Some(termination)));
                    }
                } else {
                    let nested_arguments = format_argument("~?", arguments, &mut argument_index)?;
                    let nested_arguments = nested_arguments.list_items().ok_or_else(|| {
                        type_error("format", "a list of arguments for ~?", nested_arguments)
                    })?;
                    output.push_str(&format_control(&nested_control, &nested_arguments)?);
                }
            }
            '^' => {
                if format_escape_upward(
                    &parameters,
                    arguments,
                    argument_index,
                    colon_modifier,
                    colon_iteration_last,
                )? {
                    return Ok((
                        output,
                        argument_index,
                        Some(FormatTermination { colon_modifier }),
                    ));
                }
            }
            '{' => {
                let body_end = format_iteration_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let limit = format_iteration_limit(&parameters)?;
                if at_sign_modifier {
                    let (formatted, consumed) = format_iteration(
                        body,
                        &arguments[argument_index..],
                        colon_modifier,
                        limit,
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                } else {
                    let list = format_argument("~{", arguments, &mut argument_index)?;
                    let list = list
                        .list_items()
                        .ok_or_else(|| type_error("format", "a list for ~{", list))?;
                    let (formatted, _) = format_iteration(body, &list, colon_modifier, limit)?;
                    output.push_str(&formatted);
                }
            }
            '[' => {
                let body_end = format_choice_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let clauses = format_choice_clauses(body)?;
                if colon_modifier && at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format choice cannot use both : and @ modifiers".to_string(),
                        span: None,
                    });
                }
                if colon_modifier && clauses.len() != 2 {
                    return Err(RuntimeError::InvalidForm {
                        message: "boolean format choice needs two clauses".to_string(),
                        span: None,
                    });
                }
                if at_sign_modifier && clauses.len() != 1 {
                    return Err(RuntimeError::InvalidForm {
                        message: "at-sign format choice needs one clause".to_string(),
                        span: None,
                    });
                }
                if (colon_modifier || at_sign_modifier) && !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format choice parameters cannot be used with : or @ modifier"
                            .to_string(),
                        span: None,
                    });
                }
                if !colon_modifier && !at_sign_modifier && parameters.len() > 1 {
                    return Err(RuntimeError::InvalidForm {
                        message: "format choice accepts at most one parameter".to_string(),
                        span: None,
                    });
                }

                let selected_index = if colon_modifier {
                    let selector = format_argument("~[", arguments, &mut argument_index)?;
                    Some(usize::from(selector.is_truthy()))
                } else if at_sign_modifier {
                    let selector =
                        arguments
                            .get(argument_index)
                            .ok_or_else(|| RuntimeError::InvalidForm {
                                message: "format directive ~[ needs another argument".to_string(),
                                span: None,
                            })?;
                    if selector.is_truthy() {
                        Some(0)
                    } else {
                        argument_index += 1;
                        None
                    }
                } else {
                    let has_selector_parameter = matches!(
                        parameters.first().copied(),
                        Some(FormatParameter::Number(_)) | Some(FormatParameter::Character(_))
                    );
                    let index = if has_selector_parameter {
                        format_parameter_number(&parameters, 0, 0)?
                    } else {
                        let selector = format_argument("~[", arguments, &mut argument_index)?;
                        integer_argument("format choice", selector)?
                    };
                    usize::try_from(index).ok()
                };
                let selected_clause = selected_index.and_then(|index| {
                    clauses
                        .get(index)
                        .or_else(|| clauses.iter().find(|(_, default)| *default))
                });
                if let Some((clause, _)) = selected_clause {
                    let (formatted, consumed, termination) = format_control_characters(
                        clause,
                        &arguments[argument_index..],
                        colon_iteration_last,
                    )?;
                    output.push_str(&formatted);
                    argument_index += consumed;
                    if let Some(termination) = termination {
                        return Ok((output, argument_index, Some(termination)));
                    }
                } else if !colon_modifier && !at_sign_modifier {
                    if let Some((clause, _)) = clauses.iter().find(|(_, default)| *default) {
                        let (formatted, consumed, termination) = format_control_characters(
                            clause,
                            &arguments[argument_index..],
                            colon_iteration_last,
                        )?;
                        output.push_str(&formatted);
                        argument_index += consumed;
                        if let Some(termination) = termination {
                            return Ok((output, argument_index, Some(termination)));
                        }
                    }
                }
            }
            '<' => {
                if parameters.len() > 4 {
                    return Err(RuntimeError::InvalidForm {
                        message: "format justification accepts at most four parameters".to_string(),
                        span: None,
                    });
                }
                let body_end = format_justification_end(characters, character_index)?;
                let body = &characters[character_index..body_end];
                character_index = body_end + 2;
                let clauses = format_justification_clauses(body)?;
                let (formatted, consumed) = format_justification(
                    &clauses,
                    &arguments[argument_index..],
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                    colon_iteration_last,
                )?;
                output.push_str(&formatted);
                argument_index += consumed;
            }
            'R' => {
                let argument = format_argument("~R", arguments, &mut argument_index)?;
                let integer = integer_argument("format", argument)?;
                output.push_str(&format_radix_directive(
                    integer,
                    &parameters,
                    colon_modifier,
                    at_sign_modifier,
                )?);
            }
            'T' => {
                let column = format_parameter_count(&parameters, 0, 1)?;
                let increment = format_parameter_count(&parameters, 1, 1)?;
                if !colon_modifier {
                    let current_column = output
                        .rsplit('\n')
                        .next()
                        .unwrap_or_default()
                        .chars()
                        .count();
                    let spaces = if at_sign_modifier {
                        let relative_column = current_column.saturating_add(column);
                        let additional = if increment == 0 {
                            0
                        } else {
                            (increment - (relative_column % increment)) % increment
                        };
                        column.saturating_add(additional)
                    } else if current_column < column {
                        column - current_column
                    } else if increment == 0 {
                        0
                    } else {
                        increment - ((current_column - column) % increment)
                    };
                    output.extend(std::iter::repeat(' ').take(spaces));
                }
            }
            'W' => {
                if !parameters.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: "format ~W does not accept parameters".to_string(),
                        span: None,
                    });
                }
                let argument = format_argument("~W", arguments, &mut argument_index)?;
                output.push_str(&printed_value(argument, true));
            }
            '}' => {
                return Err(RuntimeError::InvalidForm {
                    message: "unexpected format iteration terminator ~}".to_string(),
                    span: None,
                });
            }
            _ => {
                return Err(RuntimeError::InvalidForm {
                    message: format!("unsupported format directive ~{directive}"),
                    span: None,
                });
            }
        }
    }
    Ok((output, argument_index, None))
}

fn format_iteration_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '{', "format iteration is missing ~}")
}

fn format_choice_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '[', "format choice is missing ~]")
}

fn format_justification_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(characters, start, '<', "format justification is missing ~>")
}

fn format_case_conversion_end(characters: &[char], start: usize) -> Result<usize, RuntimeError> {
    format_directive_end(
        characters,
        start,
        '(',
        "format case conversion is missing ~)",
    )
}

fn format_directive_end(
    characters: &[char],
    start: usize,
    opening: char,
    missing_message: &str,
) -> Result<usize, RuntimeError> {
    let mut stack = vec![opening];
    let mut index = start;
    while index < characters.len() {
        if characters[index] != '~' {
            index += 1;
            continue;
        }

        let (directive_index, _, _) = format_directive_prefix(characters, index + 1)?;
        let Some(directive) = characters.get(directive_index).copied() else {
            break;
        };
        match directive.to_ascii_uppercase() {
            '{' | '[' | '<' | '(' => stack.push(directive.to_ascii_uppercase()),
            '}' | ']' | '>' | ')' => {
                let expected_opening = match directive {
                    '}' => '{',
                    ']' => '[',
                    '>' => '<',
                    ')' => '(',
                    _ => unreachable!(),
                };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                    if stack.is_empty() {
                        return Ok(index);
                    }
                }
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    Err(RuntimeError::InvalidForm {
        message: missing_message.to_string(),
        span: None,
    })
}

fn format_choice_clauses<'a>(body: &'a [char]) -> Result<Vec<(&'a [char], bool)>, RuntimeError> {
    let mut clauses = Vec::new();
    let mut clause_start = 0;
    let mut default_clause = false;
    let mut stack = Vec::new();
    let mut index = 0;
    while index < body.len() {
        if body[index] != '~' {
            index += 1;
            continue;
        }

        let (directive_index, colon_modifier, _at_sign_modifier) =
            format_directive_prefix(body, index + 1)?;
        let Some(directive) = body.get(directive_index).copied() else {
            return Err(RuntimeError::InvalidForm {
                message: "format choice clause ends after a tilde".to_string(),
                span: None,
            });
        };
        let directive = directive.to_ascii_uppercase();
        match directive {
            '{' | '[' | '<' | '(' => stack.push(directive),
            '}' | ']' | '>' | ')' => {
                let expected_opening = match directive {
                    '}' => '{',
                    ']' => '[',
                    '>' => '<',
                    ')' => '(',
                    _ => unreachable!(),
                };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                } else if stack.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unexpected format choice terminator ~{directive}"),
                        span: None,
                    });
                }
            }
            ';' if stack.is_empty() => {
                clauses.push((&body[clause_start..index], default_clause));
                clause_start = directive_index + 1;
                default_clause = colon_modifier;
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    if !stack.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format choice contains an unclosed nested directive".to_string(),
            span: None,
        });
    }
    clauses.push((&body[clause_start..], default_clause));
    Ok(clauses)
}

fn format_justification_clauses<'a>(body: &'a [char]) -> Result<Vec<&'a [char]>, RuntimeError> {
    let mut clauses = Vec::new();
    let mut clause_start = 0;
    let mut stack = Vec::new();
    let mut index = 0;
    while index < body.len() {
        if body[index] != '~' {
            index += 1;
            continue;
        }

        let (directive_index, colon_modifier, at_sign_modifier) =
            format_directive_prefix(body, index + 1)?;
        let Some(directive) = body.get(directive_index).copied() else {
            return Err(RuntimeError::InvalidForm {
                message: "format justification clause ends after a tilde".to_string(),
                span: None,
            });
        };
        let directive = directive.to_ascii_uppercase();
        match directive {
            '{' | '[' | '<' | '(' => stack.push(directive),
            '}' | ']' | '>' | ')' => {
                let expected_opening = match directive {
                    '}' => '{',
                    ']' => '[',
                    '>' => '<',
                    ')' => '(',
                    _ => unreachable!(),
                };
                if stack.last().copied() == Some(expected_opening) {
                    stack.pop();
                } else if stack.is_empty() {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("unexpected format justification terminator ~{directive}"),
                        span: None,
                    });
                } else {
                    return Err(RuntimeError::InvalidForm {
                        message: format!("mismatched format justification terminator ~{directive}"),
                        span: None,
                    });
                }
            }
            ';' if stack.is_empty() => {
                if colon_modifier || at_sign_modifier {
                    return Err(RuntimeError::InvalidForm {
                        message: "format justification does not support modifiers on ~;"
                            .to_string(),
                        span: None,
                    });
                }
                clauses.push(&body[clause_start..index]);
                clause_start = directive_index + 1;
            }
            _ => {}
        }
        index = directive_index + 1;
    }
    if !stack.is_empty() {
        return Err(RuntimeError::InvalidForm {
            message: "format justification contains an unclosed nested directive".to_string(),
            span: None,
        });
    }
    clauses.push(&body[clause_start..]);
    Ok(clauses)
}

fn format_justification(
    clauses: &[&[char]],
    arguments: &[Value],
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
    colon_iteration_last: bool,
) -> Result<(String, usize), RuntimeError> {
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let column_increment = format_parameter_count(parameters, 1, 1)?;
    let minimum_padding = format_parameter_count(parameters, 2, 0)?;
    let pad_character = format_parameter_character(parameters, 3, ' ')?;
    if column_increment == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format justification column increment must be positive".to_string(),
            span: None,
        });
    }

    let mut pieces = Vec::new();
    let mut argument_index = 0;
    for clause in clauses {
        let (formatted, consumed, termination) =
            format_control_characters(clause, &arguments[argument_index..], colon_iteration_last)?;
        argument_index += consumed;
        if termination.is_some() {
            break;
        }
        pieces.push(formatted);
    }

    if pieces.is_empty() {
        return Ok((String::new(), argument_index));
    }

    let between_count = pieces.len().saturating_sub(1);
    let content_width = pieces.iter().fold(0usize, |width, piece| {
        width.saturating_add(piece.chars().count())
    });
    let required_width =
        content_width.saturating_add(minimum_padding.saturating_mul(between_count));
    let mut target_width = minimum_column.max(required_width);
    if target_width > minimum_column {
        let remainder = (target_width - minimum_column) % column_increment;
        if remainder != 0 {
            target_width = target_width.saturating_add(column_increment - remainder);
        }
    }
    let total_padding = target_width.saturating_sub(content_width);
    let base_between_padding = minimum_padding.saturating_mul(between_count);

    let leading_gap = if pieces.len() == 1 {
        colon_modifier || !at_sign_modifier
    } else {
        colon_modifier
    };
    let trailing_gap = at_sign_modifier;
    let gap_count = (if leading_gap { 1usize } else { 0usize })
        .saturating_add(between_count)
        .saturating_add(if trailing_gap { 1usize } else { 0usize });
    let distributed_padding = total_padding.saturating_sub(base_between_padding);
    let base_padding = if gap_count == 0 {
        0
    } else {
        distributed_padding / gap_count
    };
    let remainder = if gap_count == 0 {
        0
    } else {
        distributed_padding % gap_count
    };
    let mut gaps = vec![0usize; gap_count];
    for (index, gap) in gaps.iter_mut().enumerate() {
        *gap = base_padding.saturating_add(usize::from(index >= gap_count - remainder));
    }

    let mut gap_index = 0;
    if leading_gap {
        gap_index += 1;
    }
    for _ in 0..between_count {
        gaps[gap_index] = gaps[gap_index].saturating_add(minimum_padding);
        gap_index += 1;
    }

    let mut output = String::new();
    let append_padding = |output: &mut String, count: usize| {
        output.extend(std::iter::repeat(pad_character).take(count));
    };
    gap_index = 0;
    if leading_gap {
        append_padding(&mut output, gaps[gap_index]);
        gap_index += 1;
    }
    for (index, piece) in pieces.iter().enumerate() {
        output.push_str(piece);
        if index + 1 < pieces.len() {
            append_padding(&mut output, gaps[gap_index]);
            gap_index += 1;
        }
    }
    if trailing_gap {
        append_padding(&mut output, gaps[gap_index]);
    }
    Ok((output, argument_index))
}

fn format_case_conversion(text: &str, colon_modifier: bool, at_sign_modifier: bool) -> String {
    let mut output = String::new();
    if colon_modifier && at_sign_modifier {
        for character in text.chars() {
            output.extend(character.to_uppercase());
        }
        return output;
    }

    if colon_modifier {
        let mut word_start = true;
        for character in text.chars() {
            if character.is_alphanumeric() {
                if word_start {
                    output.extend(character.to_uppercase());
                } else {
                    output.extend(character.to_lowercase());
                }
                word_start = false;
            } else {
                output.push(character);
                word_start = true;
            }
        }
        return output;
    }

    if at_sign_modifier {
        let mut first_word = true;
        let mut word_start = true;
        for character in text.chars() {
            if first_word && character.is_whitespace() {
                first_word = false;
                output.push(character);
            } else if first_word && character.is_alphanumeric() {
                if word_start {
                    output.extend(character.to_uppercase());
                } else {
                    output.extend(character.to_lowercase());
                }
                word_start = false;
            } else {
                output.extend(character.to_lowercase());
            }
        }
        return output;
    }

    for character in text.chars() {
        output.extend(character.to_lowercase());
    }
    output
}

fn format_escape_upward(
    parameters: &[FormatParameter],
    arguments: &[Value],
    argument_index: usize,
    colon_modifier: bool,
    colon_iteration_last: bool,
) -> Result<bool, RuntimeError> {
    if parameters.is_empty() {
        return Ok(if colon_modifier {
            colon_iteration_last
        } else {
            argument_index >= arguments.len()
        });
    }
    if parameters.len() > 3 {
        return Err(RuntimeError::InvalidForm {
            message: "format ~^ accepts at most three parameters".to_string(),
            span: None,
        });
    }
    let values = parameters
        .iter()
        .map(|parameter| match parameter {
            FormatParameter::Missing => Ok(0),
            FormatParameter::Number(value) => Ok(*value),
            FormatParameter::Character(_) => Err(RuntimeError::InvalidForm {
                message: "format ~^ parameters must be numeric".to_string(),
                span: None,
            }),
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    Ok(match values.as_slice() {
        [value] => *value == 0,
        [first, second] => first == second,
        [first, second, third] => first <= second && second <= third,
        _ => unreachable!("format ~^ parameter count was checked"),
    })
}

fn format_iteration(
    body: &[char],
    arguments: &[Value],
    colon_modifier: bool,
    limit: Option<usize>,
) -> Result<(String, usize), RuntimeError> {
    let mut output = String::new();
    let mut argument_index = 0;
    let mut repetitions = 0;
    while argument_index < arguments.len() && limit.map_or(true, |limit| repetitions < limit) {
        let (consumed, termination) = if colon_modifier {
            let nested_arguments = arguments[argument_index].list_items().ok_or_else(|| {
                type_error(
                    "format",
                    "a list element for ~:{",
                    &arguments[argument_index],
                )
            })?;
            let (formatted, consumed, termination) = format_control_characters(
                body,
                &nested_arguments,
                argument_index + 1 >= arguments.len(),
            )?;
            output.push_str(&formatted);
            (consumed, termination)
        } else {
            let (formatted, consumed, termination) =
                format_control_characters(body, &arguments[argument_index..], false)?;
            output.push_str(&formatted);
            (consumed, termination)
        };
        argument_index += if colon_modifier { 1 } else { consumed.max(1) };
        repetitions += 1;
        if let Some(termination) = termination {
            if colon_modifier && !termination.colon_modifier {
                continue;
            }
            break;
        }
    }
    Ok((output, argument_index))
}

fn format_parameter_number(
    parameters: &[FormatParameter],
    index: usize,
    default: i64,
) -> Result<i64, RuntimeError> {
    match parameters
        .get(index)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => Ok(default),
        FormatParameter::Number(value) => Ok(value),
        FormatParameter::Character(_) => Err(RuntimeError::InvalidForm {
            message: format!("format parameter {index} must be numeric"),
            span: None,
        }),
    }
}

fn format_parameter_count(
    parameters: &[FormatParameter],
    index: usize,
    default: i64,
) -> Result<usize, RuntimeError> {
    let value = format_parameter_number(parameters, index, default)?;
    usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
        message: format!("format parameter {index} must be non-negative"),
        span: None,
    })
}

fn format_parameter_character(
    parameters: &[FormatParameter],
    index: usize,
    default: char,
) -> Result<char, RuntimeError> {
    match parameters
        .get(index)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => Ok(default),
        FormatParameter::Character(value) => Ok(value),
        FormatParameter::Number(_) => Err(RuntimeError::InvalidForm {
            message: format!("format parameter {index} must be a character"),
            span: None,
        }),
    }
}

fn format_iteration_limit(parameters: &[FormatParameter]) -> Result<Option<usize>, RuntimeError> {
    if parameters.is_empty() || matches!(parameters[0], FormatParameter::Missing) {
        Ok(None)
    } else {
        Ok(Some(format_parameter_count(parameters, 0, 0)?))
    }
}

fn format_text_field(
    text: String,
    parameters: &[FormatParameter],
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let column_increment = format_parameter_count(parameters, 1, 1)?;
    let minimum_padding = format_parameter_count(parameters, 2, 0)?;
    let padding_character = format_parameter_character(parameters, 3, ' ')?;
    if column_increment == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format column increment must be positive".to_string(),
            span: None,
        });
    }

    let width = text.chars().count();
    let mut target = minimum_column.max(width.saturating_add(minimum_padding));
    if target > minimum_column {
        let remainder = (target - minimum_column) % column_increment;
        if remainder != 0 {
            target += column_increment - remainder;
        }
    }
    let padding = target.saturating_sub(width);
    let mut formatted = String::new();
    if at_sign_modifier {
        formatted.extend(std::iter::repeat(padding_character).take(padding));
        formatted.push_str(&text);
    } else {
        formatted.push_str(&text);
        formatted.extend(std::iter::repeat(padding_character).take(padding));
    }
    Ok(formatted)
}

fn format_integer_directive(
    value: i64,
    radix: u32,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let padding_character = format_parameter_character(parameters, 1, ' ')?;
    let comma_character = format_parameter_character(parameters, 2, ',')?;
    let comma_interval = format_parameter_count(parameters, 3, 3)?;
    if colon_modifier && comma_interval == 0 {
        return Err(RuntimeError::InvalidForm {
            message: "format comma interval must be positive".to_string(),
            span: None,
        });
    }

    let mut digits = format_unsigned_integer(value.unsigned_abs(), radix);
    if colon_modifier {
        digits = format_grouped_digits(&digits, comma_character, comma_interval);
    }
    let mut formatted = String::new();
    if value < 0 {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&digits);
    let padding = minimum_column.saturating_sub(formatted.chars().count());
    let mut result = String::new();
    result.extend(std::iter::repeat(padding_character).take(padding));
    result.push_str(&formatted);
    Ok(result)
}

fn format_fixed_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "unsupported format modifier before ~F".to_string(),
            span: None,
        });
    }
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let fractional_digits = match parameters
        .get(1)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format fractional digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 1 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let scale = format_parameter_number(parameters, 2, 0)?;
    let scale = i32::try_from(scale).map_err(|_| RuntimeError::InvalidForm {
        message: "format scale factor is out of range".to_string(),
        span: None,
    })?;
    let overflow_character = match parameters
        .get(3)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Character(value) => Some(value),
        FormatParameter::Number(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 3 must be a character".to_string(),
                span: None,
            });
        }
    };
    let padding_character = format_parameter_character(parameters, 4, ' ')?;
    let scaled = value * 10_f64.powi(scale);
    let negative = scaled.is_sign_negative();
    let magnitude = scaled.abs();
    let mut digits = if let Some(fractional_digits) = fractional_digits {
        let mut digits = format!("{:.*}", fractional_digits, magnitude);
        if fractional_digits == 0 {
            digits.push('.');
        }
        digits
    } else {
        let mut digits = magnitude.to_string();
        if !digits.contains('.') && !digits.contains('e') && !digits.contains('E') {
            digits.push_str(".0");
        }
        digits
    };
    if let Some(fractional_digits) = fractional_digits {
        if minimum_column == fractional_digits.saturating_add(1) && digits.starts_with("0.") {
            digits.remove(0);
        }
    }

    let mut formatted = String::new();
    if negative {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&digits);

    let width = formatted.chars().count();
    if minimum_column > 0 && width > minimum_column {
        if let Some(overflow_character) = overflow_character {
            return Ok(std::iter::repeat(overflow_character)
                .take(minimum_column)
                .collect());
        }
        return Ok(formatted);
    }
    let padding = minimum_column.saturating_sub(width);
    let mut result = String::new();
    result.extend(std::iter::repeat(padding_character).take(padding));
    result.push_str(&formatted);
    Ok(result)
}

fn format_general_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "unsupported format modifier before ~G".to_string(),
            span: None,
        });
    }

    let parameter_at = |index| {
        parameters
            .get(index)
            .copied()
            .unwrap_or(FormatParameter::Missing)
    };
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let requested_fractional_digits = match parameter_at(1) {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format fractional digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 1 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let exponent_padding = match parameter_at(2) {
        FormatParameter::Missing => 4,
        FormatParameter::Number(value) => usize::try_from(value)
            .map_err(|_| RuntimeError::InvalidForm {
                message: "format exponent field count must be non-negative".to_string(),
                span: None,
            })?
            .checked_add(2)
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "format exponent field count is out of range".to_string(),
                span: None,
            })?,
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 2 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let exponent_character = match parameter_at(6) {
        FormatParameter::Missing => FormatParameter::Character('e'),
        parameter => parameter,
    };

    if !value.is_finite() {
        let exponential_parameters = vec![
            FormatParameter::Number(i64::try_from(minimum_column).map_err(|_| {
                RuntimeError::InvalidForm {
                    message: "format field width is out of range".to_string(),
                    span: None,
                }
            })?),
            FormatParameter::Missing,
            FormatParameter::Missing,
            parameter_at(3),
            parameter_at(4),
            parameter_at(5),
            exponent_character,
        ];
        return format_exponential_float_directive(
            value,
            &exponential_parameters,
            false,
            at_sign_modifier,
        );
    }

    let exponent = general_float_decimal_exponent(value);
    let fractional_digits = requested_fractional_digits.unwrap_or_else(|| {
        let q = general_float_default_fractional_digits(value, exponent);
        let minimum = usize::try_from(exponent.min(7).max(0)).unwrap_or(0);
        q.max(minimum).max(1)
    });
    let fixed_point =
        exponent >= 0 && fractional_digits >= usize::try_from(exponent).unwrap_or(usize::MAX);
    let fractional_digits =
        i64::try_from(fractional_digits).map_err(|_| RuntimeError::InvalidForm {
            message: "format fractional digit count is out of range".to_string(),
            span: None,
        })?;
    let exponent_padding =
        i64::try_from(exponent_padding).map_err(|_| RuntimeError::InvalidForm {
            message: "format exponent field count is out of range".to_string(),
            span: None,
        })?;
    let minimum_column = i64::try_from(minimum_column).map_err(|_| RuntimeError::InvalidForm {
        message: "format field width is out of range".to_string(),
        span: None,
    })?;

    if fixed_point {
        let exponent_as_usize = usize::try_from(exponent).unwrap_or(0);
        let fixed_fractional_digits = fractional_digits
            .checked_sub(i64::try_from(exponent_as_usize).unwrap_or(i64::MAX))
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "format fractional digit count is out of range".to_string(),
                span: None,
            })?;
        let fixed_width = minimum_column.saturating_sub(exponent_padding).max(0);
        let fixed_parameters = vec![
            FormatParameter::Number(fixed_width),
            FormatParameter::Number(fixed_fractional_digits),
            FormatParameter::Missing,
            parameter_at(4),
            parameter_at(5),
        ];
        let mut formatted =
            format_fixed_float_directive(value, &fixed_parameters, false, at_sign_modifier)?;
        formatted
            .extend(std::iter::repeat(' ').take(usize::try_from(exponent_padding).unwrap_or(0)));
        return Ok(formatted);
    }

    let exponential_parameters = vec![
        FormatParameter::Number(minimum_column),
        FormatParameter::Number(fractional_digits),
        FormatParameter::Missing,
        parameter_at(3),
        parameter_at(4),
        parameter_at(5),
        exponent_character,
    ];
    format_exponential_float_directive(value, &exponential_parameters, false, at_sign_modifier)
}

fn general_float_decimal_exponent(value: f64) -> i64 {
    if value == 0.0 {
        return 1;
    }
    let magnitude = value.abs();
    let mut exponent = magnitude.log10().floor() as i64 + 1;
    while magnitude < 10_f64.powi((exponent - 1) as i32) {
        exponent -= 1;
    }
    while magnitude >= 10_f64.powi(exponent as i32) {
        exponent += 1;
    }
    exponent
}

fn general_float_default_fractional_digits(value: f64, exponent: i64) -> usize {
    let decimal = value.abs().to_string();
    let mantissa = decimal
        .split_once('e')
        .or_else(|| decimal.split_once('E'))
        .map(|(mantissa, _)| mantissa)
        .unwrap_or(&decimal);
    let mut found_nonzero = false;
    let mut significant_digits = 0usize;
    for character in mantissa.chars() {
        if !character.is_ascii_digit() {
            continue;
        }
        if character != '0' || found_nonzero {
            found_nonzero = true;
            significant_digits = significant_digits.saturating_add(1);
        }
    }
    let significant_digits = significant_digits.max(1);
    let leading_decimal_places = if exponent < 0 {
        usize::try_from(exponent.unsigned_abs()).unwrap_or(usize::MAX)
    } else {
        0
    };
    significant_digits.saturating_add(leading_decimal_places)
}

fn format_dollar_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    let fractional_digits = format_parameter_count(parameters, 0, 2)?;
    let minimum_integer_digits = format_parameter_count(parameters, 1, 1)?;
    let minimum_column = format_parameter_count(parameters, 2, 0)?;
    let padding_character = format_parameter_character(parameters, 3, ' ')?;

    let negative = value.is_sign_negative();
    let magnitude = value.abs();
    let mut digits = format!("{:.*}", fractional_digits, magnitude);
    if fractional_digits == 0 {
        digits.push('.');
    }
    let (integer_part, fractional_part) =
        digits
            .split_once('.')
            .ok_or_else(|| RuntimeError::InvalidForm {
                message: "format ~$ could not produce a fixed-point number".to_string(),
                span: None,
            })?;

    let mut numeric = String::new();
    numeric.extend(
        std::iter::repeat('0')
            .take(minimum_integer_digits.saturating_sub(integer_part.chars().count())),
    );
    numeric.push_str(integer_part);
    numeric.push('.');
    numeric.push_str(fractional_part);

    let sign = if negative {
        Some('-')
    } else if at_sign_modifier {
        Some('+')
    } else {
        None
    };
    let sign_width = usize::from(sign.is_some());
    let padding = minimum_column.saturating_sub(sign_width + numeric.chars().count());
    let mut result = String::new();
    if colon_modifier {
        if let Some(sign) = sign {
            result.push(sign);
        }
        result.extend(std::iter::repeat(padding_character).take(padding));
    } else {
        result.extend(std::iter::repeat(padding_character).take(padding));
        if let Some(sign) = sign {
            result.push(sign);
        }
    }
    result.push_str(&numeric);
    Ok(result)
}

fn format_exponential_float_directive(
    value: f64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if colon_modifier {
        return Err(RuntimeError::InvalidForm {
            message: "unsupported format modifier before ~E".to_string(),
            span: None,
        });
    }
    let minimum_column = format_parameter_count(parameters, 0, 0)?;
    let requested_fractional_digits = match parameters
        .get(1)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format fractional digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 1 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let requested_exponent_digits = match parameters
        .get(2)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Number(value) => {
            Some(
                usize::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                    message: "format exponent digit count must be non-negative".to_string(),
                    span: None,
                })?,
            )
        }
        FormatParameter::Character(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 2 must be numeric".to_string(),
                span: None,
            });
        }
    };
    let scale = i32::try_from(format_parameter_number(parameters, 3, 1)?).map_err(|_| {
        RuntimeError::InvalidForm {
            message: "format scale factor is out of range".to_string(),
            span: None,
        }
    })?;
    if let Some(fractional_digits) = requested_fractional_digits {
        let invalid_positive_scale =
            scale > 0 && (scale as usize) >= fractional_digits.saturating_add(2);
        let invalid_negative_scale =
            scale < 0 && (scale.unsigned_abs() as usize) >= fractional_digits;
        if invalid_positive_scale || invalid_negative_scale {
            return Err(RuntimeError::InvalidForm {
                message: "format scale factor is incompatible with fractional digit count"
                    .to_string(),
                span: None,
            });
        }
    }
    let fractional_digits = requested_fractional_digits.unwrap_or_else(|| {
        let minimum = if scale > 0 {
            (scale as usize).saturating_sub(1)
        } else if scale < 0 {
            (scale.unsigned_abs() as usize).saturating_add(1)
        } else {
            0
        };
        6.max(minimum)
    });
    let significant_digits = if scale > 0 {
        fractional_digits.checked_add(1)
    } else if scale == 0 {
        Some(fractional_digits.max(1))
    } else {
        fractional_digits.checked_sub(scale.unsigned_abs() as usize)
    }
    .filter(|value| *value > 0)
    .ok_or_else(|| RuntimeError::InvalidForm {
        message: "format scale factor leaves no significant digits".to_string(),
        span: None,
    })?;
    let overflow_character = match parameters
        .get(4)
        .copied()
        .unwrap_or(FormatParameter::Missing)
    {
        FormatParameter::Missing => None,
        FormatParameter::Character(value) => Some(value),
        FormatParameter::Number(_) => {
            return Err(RuntimeError::InvalidForm {
                message: "format parameter 4 must be a character".to_string(),
                span: None,
            });
        }
    };
    let padding_character = format_parameter_character(parameters, 5, ' ')?;
    let exponent_character = format_parameter_character(parameters, 6, 'E')?;
    let apply_field = |formatted: String| {
        let width = formatted.chars().count();
        if minimum_column > 0 && width > minimum_column {
            if let Some(overflow_character) = overflow_character {
                return Ok(std::iter::repeat(overflow_character)
                    .take(minimum_column)
                    .collect());
            }
            return Ok(formatted);
        }
        let padding = minimum_column.saturating_sub(width);
        let mut result = String::new();
        result.extend(std::iter::repeat(padding_character).take(padding));
        result.push_str(&formatted);
        Ok(result)
    };

    if !value.is_finite() {
        let mut formatted = String::new();
        if value.is_sign_negative() {
            formatted.push('-');
        } else if at_sign_modifier {
            formatted.push('+');
        }
        formatted.push_str(if value.is_nan() { "NaN" } else { "Inf" });
        return apply_field(formatted);
    }

    let magnitude = value.abs();
    let scientific = format!("{:.*e}", significant_digits.saturating_sub(1), magnitude);
    let (coefficient, exponent_text) = scientific
        .split_once('e')
        .or_else(|| scientific.split_once('E'))
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: "format exponential conversion did not produce an exponent".to_string(),
            span: None,
        })?;
    let raw_exponent = exponent_text
        .parse::<i32>()
        .map_err(|_| RuntimeError::InvalidForm {
            message: "format exponential conversion produced an invalid exponent".to_string(),
            span: None,
        })?;
    let mut digits = coefficient
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<Vec<_>>();
    digits.truncate(significant_digits);
    digits.resize(significant_digits, '0');

    let mut mantissa = String::new();
    if scale > 0 {
        let digits_before_decimal = scale as usize;
        for index in 0..digits_before_decimal {
            mantissa.push(*digits.get(index).unwrap_or(&'0'));
        }
        mantissa.push('.');
        let digits_after_decimal =
            fractional_digits.saturating_sub(digits_before_decimal.saturating_sub(1));
        for index in 0..digits_after_decimal {
            mantissa.push(*digits.get(digits_before_decimal + index).unwrap_or(&'0'));
        }
    } else if scale == 0 {
        mantissa.push_str("0.");
        for index in 0..fractional_digits {
            mantissa.push(*digits.get(index).unwrap_or(&'0'));
        }
    } else {
        mantissa.push_str("0.");
        mantissa.extend(std::iter::repeat('0').take(scale.unsigned_abs() as usize));
        let significant_fractional_digits =
            fractional_digits.saturating_sub(scale.unsigned_abs() as usize);
        for index in 0..significant_fractional_digits {
            mantissa.push(*digits.get(index).unwrap_or(&'0'));
        }
    }
    if requested_fractional_digits.is_none() {
        if let Some(decimal_index) = mantissa.find('.') {
            while mantissa.len() > decimal_index + 2 && mantissa.ends_with('0') {
                mantissa.pop();
            }
        }
    }

    let exponent = i64::from(raw_exponent)
        .checked_sub(i64::from(scale) - 1)
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: "format exponent is out of range".to_string(),
            span: None,
        })?;
    let mut formatted = String::new();
    if value.is_sign_negative() {
        formatted.push('-');
    } else if at_sign_modifier {
        formatted.push('+');
    }
    formatted.push_str(&mantissa);
    formatted.push(exponent_character);
    if exponent < 0 {
        formatted.push('-');
    } else {
        formatted.push('+');
    }
    let exponent_magnitude = exponent.unsigned_abs().to_string();
    if let Some(exponent_width) = requested_exponent_digits {
        formatted.extend(
            std::iter::repeat('0')
                .take(exponent_width.saturating_sub(exponent_magnitude.chars().count())),
        );
    }
    formatted.push_str(&exponent_magnitude);
    apply_field(formatted)
}

fn format_grouped_digits(digits: &str, separator: char, interval: usize) -> String {
    if digits.is_empty() || interval == 0 {
        return digits.to_string();
    }
    let mut grouped = String::new();
    for (index, character) in digits.chars().enumerate() {
        if index != 0 && (digits.chars().count() - index) % interval == 0 {
            grouped.push(separator);
        }
        grouped.push(character);
    }
    grouped
}

fn format_character_directive(
    character: char,
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> String {
    let name = match character {
        '\0' => Some("Null"),
        '\x07' => Some("Bell"),
        '\x08' => Some("Backspace"),
        '\t' => Some("Tab"),
        '\n' => Some("Newline"),
        '\x0c' => Some("Page"),
        '\r' => Some("Return"),
        ' ' => Some("Space"),
        _ => None,
    };
    if at_sign_modifier {
        let mut result = String::from("#\\");
        if let Some(name) = name {
            result.push_str(name);
        } else {
            result.push(character);
        }
        result
    } else if colon_modifier {
        name.map(str::to_string)
            .unwrap_or_else(|| character.to_string())
    } else {
        character.to_string()
    }
}

fn format_radix_directive(
    value: i64,
    parameters: &[FormatParameter],
    colon_modifier: bool,
    at_sign_modifier: bool,
) -> Result<String, RuntimeError> {
    if let Some(parameter) = parameters.first().copied() {
        if !matches!(parameter, FormatParameter::Missing) {
            let radix = match parameter {
                FormatParameter::Number(value) => {
                    u32::try_from(value).map_err(|_| RuntimeError::InvalidForm {
                        message: "format radix must be between 2 and 36".to_string(),
                        span: None,
                    })?
                }
                FormatParameter::Missing => unreachable!(),
                FormatParameter::Character(_) => {
                    return Err(RuntimeError::InvalidForm {
                        message: "format radix must be numeric".to_string(),
                        span: None,
                    });
                }
            };
            if !(2..=36).contains(&radix) {
                return Err(RuntimeError::InvalidForm {
                    message: "format radix must be between 2 and 36".to_string(),
                    span: None,
                });
            }
            return format_integer_directive(
                value,
                radix,
                &parameters[1..],
                false,
                at_sign_modifier,
            );
        }
    }
    if at_sign_modifier {
        Ok(format_roman_number(value, colon_modifier))
    } else {
        Ok(format_english_number(value, colon_modifier))
    }
}

fn format_english_number(value: i64, ordinal: bool) -> String {
    if value < 0 {
        if value == i64::MIN {
            return format!(
                "minus {}",
                format_unsigned_integer(value.unsigned_abs(), 10)
            );
        }
        return format!(
            "minus {}",
            format_english_number(value.wrapping_neg(), ordinal)
        );
    }
    let magnitude = value as u64;
    if magnitude == 0 {
        return if ordinal {
            "zeroth".to_string()
        } else {
            "zero".to_string()
        };
    }
    const GROUPS: &[&str] = &[
        "",
        "thousand",
        "million",
        "billion",
        "trillion",
        "quadrillion",
    ];
    let mut chunks = Vec::new();
    let mut remainder = magnitude;
    while remainder != 0 {
        chunks.push(remainder % 1000);
        remainder /= 1000;
    }
    if chunks.len() > GROUPS.len() {
        return format_integer_radix(value, 10);
    }
    let ordinal_group = if ordinal {
        chunks.iter().position(|chunk| *chunk != 0)
    } else {
        None
    };
    let mut parts = Vec::new();
    for index in (0..chunks.len()).rev() {
        let chunk = chunks[index];
        if chunk == 0 {
            continue;
        }
        let group_is_ordinal = ordinal_group == Some(index);
        let mut part = if group_is_ordinal && index == 0 {
            english_under_thousand(chunk, true)
        } else {
            english_under_thousand(chunk, false)
        };
        if index != 0 {
            part.push(' ');
            part.push_str(GROUPS[index]);
            if group_is_ordinal {
                part.push_str("th");
            }
        }
        parts.push(part);
    }
    parts.join(" ")
}

fn english_under_thousand(value: u64, ordinal: bool) -> String {
    const CARDINALS: &[&str] = &[
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const ORDINALS: &[&str] = &[
        "zeroth",
        "first",
        "second",
        "third",
        "fourth",
        "fifth",
        "sixth",
        "seventh",
        "eighth",
        "ninth",
        "tenth",
        "eleventh",
        "twelfth",
        "thirteenth",
        "fourteenth",
        "fifteenth",
        "sixteenth",
        "seventeenth",
        "eighteenth",
        "nineteenth",
    ];
    const TENS: &[&str] = &[
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    const ORDINAL_TENS: &[&str] = &[
        "",
        "",
        "twentieth",
        "thirtieth",
        "fortieth",
        "fiftieth",
        "sixtieth",
        "seventieth",
        "eightieth",
        "ninetieth",
    ];
    if value < 20 {
        return if ordinal {
            ORDINALS[value as usize].to_string()
        } else {
            CARDINALS[value as usize].to_string()
        };
    }
    if value < 100 {
        let tens = value / 10;
        let ones = value % 10;
        if ones == 0 {
            return if ordinal {
                ORDINAL_TENS[tens as usize].to_string()
            } else {
                TENS[tens as usize].to_string()
            };
        }
        return format!(
            "{}-{}",
            TENS[tens as usize],
            english_under_thousand(ones, ordinal)
        );
    }
    let hundreds = value / 100;
    let remainder = value % 100;
    if remainder == 0 {
        if ordinal {
            format!("{} hundredth", CARDINALS[hundreds as usize])
        } else {
            format!("{} hundred", CARDINALS[hundreds as usize])
        }
    } else {
        format!(
            "{} hundred {}",
            CARDINALS[hundreds as usize],
            english_under_thousand(remainder, ordinal)
        )
    }
}

fn format_roman_number(value: i64, old_style: bool) -> String {
    if value == 0 {
        return "N".to_string();
    }
    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    if !old_style && magnitude > 3999 {
        return format_integer_radix(value, 10);
    }
    let numerals = [
        (1000_u64, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut remainder = magnitude;
    let mut result = String::new();
    if negative {
        result.push('-');
    }
    for (unit, numeral) in numerals {
        while remainder >= unit {
            result.push_str(numeral);
            remainder -= unit;
        }
    }
    result
}

fn format_argument<'a>(
    directive: &str,
    arguments: &'a [Value],
    argument_index: &mut usize,
) -> Result<&'a Value, RuntimeError> {
    let argument = arguments
        .get(*argument_index)
        .ok_or_else(|| RuntimeError::InvalidForm {
            message: format!("format directive {directive} needs another argument"),
            span: None,
        })?;
    *argument_index += 1;
    Ok(argument)
}

fn append_aesthetic(output: &mut String, value: &Value) {
    match value {
        Value::String(value) => output.push_str(value),
        Value::Character(value) => output.push(*value),
        Value::List(values) => {
            output.push('(');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            output.push(')');
        }
        Value::DottedList { items, tail } => {
            output.push('(');
            for (index, value) in items.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            if !items.is_empty() {
                output.push(' ');
            }
            output.push_str(". ");
            append_aesthetic(output, tail);
            output.push(')');
        }
        Value::Vector { .. } => {
            let values = value.vector_items().expect("vector items");
            output.push_str("#(");
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(' ');
                }
                append_aesthetic(output, value);
            }
            output.push(')');
        }
        _ => output.push_str(&value.to_string()),
    }
}

fn format_integer_radix(value: i64, radix: u32) -> String {
    let mut result = format_unsigned_integer(value.unsigned_abs(), radix);
    if value < 0 {
        result.insert(0, '-');
    }
    result
}

fn format_unsigned_integer(mut magnitude: u64, radix: u32) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if magnitude == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while magnitude != 0 {
        digits.push(DIGITS[(magnitude % u64::from(radix)) as usize] as char);
        magnitude /= u64::from(radix);
    }
    digits.iter().rev().collect()
}

#[derive(Clone, Copy)]
enum Number {
    Integer(i64),
    Rational(Rational),
    Float(f64),
}

#[derive(Clone, Copy)]
enum Numeric {
    Real(Number),
    Complex { real: Number, imag: Number },
}

impl Number {
    fn as_float(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Rational(value) => value.numerator() as f64 / value.denominator() as f64,
            Self::Float(value) => value,
        }
    }

    fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    fn exact_parts(self) -> Option<(i64, i64)> {
        match self {
            Self::Integer(value) => Some((value, 1)),
            Self::Rational(value) => Some((value.numerator(), value.denominator())),
            Self::Float(_) => None,
        }
    }
}

impl Numeric {
    fn into_complex(self) -> (Number, Number) {
        match self {
            Self::Real(value) => (value, Number::Integer(0)),
            Self::Complex { real, imag } => (real, imag),
        }
    }
}

impl Value {
    fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }
}

fn number(value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error("numeric operation", value)),
    }
}

fn number_argument(function: &str, value: &Value) -> Result<Number, RuntimeError> {
    match value {
        Value::Integer(value) => Ok(Number::Integer(*value)),
        Value::Rational(value) => Ok(Number::Rational(*value)),
        Value::Float(value) => Ok(Number::Float(*value)),
        value => Err(number_error(function, value)),
    }
}

fn numeric_argument(function: &str, value: &Value) -> Result<Numeric, RuntimeError> {
    match value {
        Value::Complex { real, imag } => Ok(Numeric::Complex {
            real: number_argument(function, real.as_ref())?,
            imag: number_argument(function, imag.as_ref())?,
        }),
        _ => Ok(Numeric::Real(number_argument(function, value)?)),
    }
}

fn number_to_value(number: Number) -> Result<Value, RuntimeError> {
    match number {
        Number::Integer(value) => Ok(Value::Integer(value)),
        Number::Rational(value) => Value::rational(
            i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Value::Float(value)),
    }
}

fn numeric_to_value(number: Numeric) -> Result<Value, RuntimeError> {
    match number {
        Numeric::Real(value) => number_to_value(value),
        Numeric::Complex { real, imag } => Ok(Value::complex(
            number_to_value(real)?,
            number_to_value(imag)?,
        )),
    }
}

fn square_root_numeric(number: Numeric) -> Result<Numeric, RuntimeError> {
    let value = match number {
        Numeric::Real(number) => square_root_real(number)?,
        Numeric::Complex { real, imag } => square_root_complex(real, imag)?,
    };

    numeric_argument("sqrt", &value)
}

fn canonicalize_number(number: Number) -> Number {
    match number {
        Number::Float(value) => canonicalize_float(value),
        value => value,
    }
}

fn canonicalize_numeric(number: Numeric) -> Numeric {
    match number {
        Numeric::Real(value) => Numeric::Real(canonicalize_number(value)),
        Numeric::Complex { real, imag } => {
            let real = canonicalize_number(real);
            let imag = canonicalize_number(imag);
            if imag.as_float() == 0.0 {
                Numeric::Real(real)
            } else {
                Numeric::Complex { real, imag }
            }
        }
    }
}

fn rational_number(numerator: i128, denominator: i128) -> Result<Number, RuntimeError> {
    let value = Rational::new(numerator, denominator)?;
    if value.denominator() == 1 {
        Ok(Number::Integer(value.numerator()))
    } else {
        Ok(Number::Rational(value))
    }
}

fn exact_binary(left: Number, right: Number, operation: char) -> Result<Number, RuntimeError> {
    let (left_numerator, left_denominator) = left
        .exact_parts()
        .expect("exact numeric operation received a float");
    let (right_numerator, right_denominator) = right
        .exact_parts()
        .expect("exact numeric operation received a float");
    let left_numerator = i128::from(left_numerator);
    let left_denominator = i128::from(left_denominator);
    let right_numerator = i128::from(right_numerator);
    let right_denominator = i128::from(right_denominator);
    let (numerator, denominator) = match operation {
        '+' => (
            left_numerator * right_denominator + right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '-' => (
            left_numerator * right_denominator - right_numerator * left_denominator,
            left_denominator * right_denominator,
        ),
        '*' => (
            left_numerator * right_numerator,
            left_denominator * right_denominator,
        ),
        '/' => (
            left_numerator * right_denominator,
            left_denominator * right_numerator,
        ),
        _ => unreachable!("unsupported exact numeric operation"),
    };
    rational_number(numerator, denominator)
}

fn negate_number(value: Number) -> Result<Number, RuntimeError> {
    match value {
        Number::Integer(value) => value
            .checked_neg()
            .map(Number::Integer)
            .ok_or(RuntimeError::NumericOverflow),
        Number::Rational(value) => rational_number(
            -i128::from(value.numerator()),
            i128::from(value.denominator()),
        ),
        Number::Float(value) => Ok(Number::Float(-value)),
    }
}

fn add_numbers(left: Number, right: Number) -> Result<Number, RuntimeError> {
    if left.is_float() || right.is_float() {
        Ok(Number::Float(left.as_float() + right.as_float()))
    } else {
        exact_binary(left, right, '+')
    }
}

fn subtract_numbers(left: Number, right: Number) -> Result<Number, RuntimeError> {
    if left.is_float() || right.is_float() {
        Ok(Number::Float(left.as_float() - right.as_float()))
    } else {
        exact_binary(left, right, '-')
    }
}

fn multiply_numbers(left: Number, right: Number) -> Result<Number, RuntimeError> {
    if left.is_float() || right.is_float() {
        Ok(Number::Float(left.as_float() * right.as_float()))
    } else {
        exact_binary(left, right, '*')
    }
}

fn divide_numbers(left: Number, right: Number) -> Result<Number, RuntimeError> {
    if right.as_float() == 0.0 {
        return Err(RuntimeError::DivisionByZero);
    }
    if left.is_float() || right.is_float() {
        Ok(Number::Float(left.as_float() / right.as_float()))
    } else {
        exact_binary(left, right, '/')
    }
}

fn negate_numeric(value: Numeric) -> Result<Numeric, RuntimeError> {
    match value {
        Numeric::Real(value) => Ok(Numeric::Real(negate_number(value)?)),
        Numeric::Complex { real, imag } => Ok(Numeric::Complex {
            real: negate_number(real)?,
            imag: negate_number(imag)?,
        }),
    }
}

fn add_numeric(left: Numeric, right: Numeric) -> Result<Numeric, RuntimeError> {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => Ok(Numeric::Real(add_numbers(left, right)?)),
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            Ok(Numeric::Complex {
                real: add_numbers(left_real, right_real)?,
                imag: add_numbers(left_imag, right_imag)?,
            })
        }
    }
}

fn subtract_numeric(left: Numeric, right: Numeric) -> Result<Numeric, RuntimeError> {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => {
            Ok(Numeric::Real(subtract_numbers(left, right)?))
        }
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            Ok(Numeric::Complex {
                real: subtract_numbers(left_real, right_real)?,
                imag: subtract_numbers(left_imag, right_imag)?,
            })
        }
    }
}

fn multiply_numeric(left: Numeric, right: Numeric) -> Result<Numeric, RuntimeError> {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => {
            Ok(Numeric::Real(multiply_numbers(left, right)?))
        }
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            let ac = multiply_numbers(left_real, right_real)?;
            let bd = multiply_numbers(left_imag, right_imag)?;
            let ad = multiply_numbers(left_real, right_imag)?;
            let bc = multiply_numbers(left_imag, right_real)?;
            Ok(Numeric::Complex {
                real: subtract_numbers(ac, bd)?,
                imag: add_numbers(ad, bc)?,
            })
        }
    }
}

fn divide_numeric(left: Numeric, right: Numeric) -> Result<Numeric, RuntimeError> {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => {
            Ok(Numeric::Real(divide_numbers(left, right)?))
        }
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            let denominator = add_numbers(
                multiply_numbers(right_real, right_real)?,
                multiply_numbers(right_imag, right_imag)?,
            )?;
            let real = add_numbers(
                multiply_numbers(left_real, right_real)?,
                multiply_numbers(left_imag, right_imag)?,
            )?;
            let imag = subtract_numbers(
                multiply_numbers(left_imag, right_real)?,
                multiply_numbers(left_real, right_imag)?,
            )?;
            Ok(Numeric::Complex {
                real: divide_numbers(real, denominator)?,
                imag: divide_numbers(imag, denominator)?,
            })
        }
    }
}

fn compare_number_values(left: Number, right: Number) -> Ordering {
    if left.is_float() || right.is_float() {
        return left
            .as_float()
            .partial_cmp(&right.as_float())
            .unwrap_or(Ordering::Equal);
    }
    let (left_numerator, left_denominator) = left
        .exact_parts()
        .expect("exact numeric comparison received a float");
    let (right_numerator, right_denominator) = right
        .exact_parts()
        .expect("exact numeric comparison received a float");
    (i128::from(left_numerator) * i128::from(right_denominator))
        .cmp(&(i128::from(right_numerator) * i128::from(left_denominator)))
}

fn numeric_equalp(left: Number, right: Number) -> bool {
    compare_number_values(left, right) == Ordering::Equal
}

fn numeric_equal_values(left: Numeric, right: Numeric) -> bool {
    match (left, right) {
        (Numeric::Real(left), Numeric::Real(right)) => numeric_equalp(left, right),
        (left, right) => {
            let (left_real, left_imag) = left.into_complex();
            let (right_real, right_imag) = right.into_complex();
            numeric_equalp(left_real, right_real) && numeric_equalp(left_imag, right_imag)
        }
    }
}

fn byte_spec_value(size: i64, position: i64) -> Value {
    Value::list(vec![
        Value::symbol("BYTE"),
        Value::Integer(size),
        Value::Integer(position),
    ])
}

pub(crate) fn parse_byte_spec(function: &str, value: &Value) -> Result<(u32, u32), RuntimeError> {
    let Some(items) = value.list_items() else {
        return Err(type_error(function, "a byte specifier", value));
    };
    let [operator, size, position] = items.as_slice() else {
        return Err(type_error(function, "a byte specifier", value));
    };
    if operator
        .symbol_name()
        .map(package::normalize_symbol_name)
        .as_deref()
        != Some("BYTE")
    {
        return Err(type_error(function, "a byte specifier", value));
    }
    let size = integer_argument(function, size)?;
    let position = integer_argument(function, position)?;
    validate_byte_bounds(function, size, position)?;
    Ok((size as u32, position as u32))
}

fn validate_byte_bounds(function: &str, size: i64, position: i64) -> Result<(), RuntimeError> {
    if size < 0 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} byte size must be non-negative, got {size}"),
            span: None,
        });
    }
    if position < 0 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} byte position must be non-negative, got {position}"),
            span: None,
        });
    }
    if position >= 64 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} byte position must be less than 64, got {position}"),
            span: None,
        });
    }
    if size > 64 - position {
        return Err(RuntimeError::InvalidForm {
            message: format!(
                "{function} byte size {size} at position {position} exceeds the 64-bit integer range"
            ),
            span: None,
        });
    }
    Ok(())
}

fn validate_bit_index(function: &str, index: i64) -> Result<(), RuntimeError> {
    if index < 0 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bit index must be non-negative, got {index}"),
            span: None,
        });
    }
    if index >= 64 {
        return Err(RuntimeError::InvalidForm {
            message: format!("{function} bit index must be less than 64, got {index}"),
            span: None,
        });
    }
    Ok(())
}

fn byte_mask(size: u32, position: u32) -> u64 {
    if size == 0 {
        0
    } else {
        (u64::MAX >> (64 - size)) << position
    }
}

fn extract_byte_field(integer: u64, size: u32, position: u32) -> u64 {
    if size == 0 {
        0
    } else {
        (integer >> position) & (u64::MAX >> (64 - size))
    }
}

fn integer_argument(function: &str, value: &Value) -> Result<i64, RuntimeError> {
    value
        .as_integer()
        .ok_or_else(|| type_error(function, "integer", value))
}

fn is_real_number(value: &Value) -> bool {
    matches!(
        value,
        Value::Integer(_) | Value::Rational(_) | Value::Float(_)
    )
}

fn real_number_argument(function: &str, value: &Value) -> Result<Value, RuntimeError> {
    if is_real_number(value) {
        Ok(value.clone())
    } else {
        Err(type_error(function, "real number", value))
    }
}

fn number_error(function: &str, value: &Value) -> RuntimeError {
    type_error(function, "number", value)
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
