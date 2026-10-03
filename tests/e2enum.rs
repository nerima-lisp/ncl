#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn assert_eval_with_stress(runtime: &mut ncl_runtime::Runtime, source: &str, expected: &str) {
    runtime.set_gc_stress(true);
    runtime.set_strict_forwarding(true);
    let value = runtime
        .compile(source)
        .unwrap_or_else(|error| panic!("compile failed for {source}: {error:?}"));
    assert_eq!(runtime.format_result(value), expected, "{source}");
}

const CHILD_TIMEOUT: Duration = Duration::from_secs(30);

struct Probe(&'static str, &'static str);

const PROBES: &[Probe] = &[
    Probe("NUMBERP", "T"),
    Probe("INTEGERP", "T"),
    Probe("RATIONALP", "T"),
    Probe("FLOATP", "NIL"),
    Probe("REALP", "T"),
    Probe("COMPLEXP", "NIL"),
    Probe("+", "3"),
    Probe("-", "2"),
    Probe("*", "6"),
    Probe("/", "2"),
    Probe("=", "NIL"),
    Probe("EQ", "NIL"),
    Probe("EQL", "NIL"),
    Probe("/=", "T"),
    Probe("<", "T"),
    Probe(">", "NIL"),
    Probe("<=", "T"),
    Probe(">=", "NIL"),
    Probe("MAX", "2"),
    Probe("MIN", "1"),
    Probe("1+", "2"),
    Probe("1-", "0"),
    Probe("ABS", "1"),
    Probe("SIGNUM", "1"),
    Probe("ZEROP", "NIL"),
    Probe("PLUSP", "T"),
    Probe("MINUSP", "NIL"),
    Probe("EVENP", "NIL"),
    Probe("ODDP", "T"),
    Probe("MOD", "1"),
    Probe("REM", "1"),
    Probe("GCD", "1"),
    Probe("LCM", "1"),
    Probe("ISQRT", "1"),
    Probe("EXP", "2.718281828459045"),
    Probe("EXPT", "1"),
    Probe("SQRT", "1.0"),
    Probe("SIN", "0.8414709848078965"),
    Probe("COS", "0.5403023058681398"),
    Probe("TAN", "1.557407724654902"),
    Probe("ASIN", "1.5707963267948966"),
    Probe("ACOS", "0.0"),
    Probe("SINH", "1.1752011936438014"),
    Probe("COSH", "1.5430806348152437"),
    Probe("TANH", "0.7615941559557649"),
    Probe("ASINH", "0.8813735870195429"),
    Probe("ACOSH", "0.0"),
    Probe("ATANH", "#<DOUBLE-FLOAT NaN>"),
    Probe("COMPLEX", "#C(1.0 2.0)"),
    Probe("CONJUGATE", "1"),
    Probe("CIS", "#C(0.5403023058681398 0.8414709848078965)"),
    Probe("PHASE", "0.0"),
    Probe("REALPART", "1.0"),
    Probe("IMAGPART", "0.0"),
    Probe("LOG", "0.0"),
    Probe("ATAN", "0.7853981633974483"),
    Probe("FLOOR", "1"),
    Probe("CEILING", "1"),
    Probe("TRUNCATE", "1"),
    Probe("ROUND", "1"),
    Probe("FFLOOR", "1.0"),
    Probe("FCEILING", "1.0"),
    Probe("FTRUNCATE", "1.0"),
    Probe("FROUND", "1.0"),
    Probe("LOGAND", "1"),
    Probe("LOGIOR", "1"),
    Probe("LOGXOR", "1"),
    Probe("LOGNOT", "-2"),
    Probe("LOGEQV", "1"),
    Probe("LOGNAND", "-1"),
    Probe("LOGNOR", "-4"),
    Probe("LOGANDC1", "2"),
    Probe("LOGANDC2", "1"),
    Probe("LOGORC1", "-2"),
    Probe("LOGORC2", "-3"),
    Probe("LOGTEST", "NIL"),
    Probe("LOGBITP", "T"),
    Probe("LOGCOUNT", "1"),
    Probe("INTEGER-LENGTH", "1"),
    Probe("ASH", "4"),
    Probe("BYTE", "8589934593"),
    Probe("BYTE-SIZE", "1"),
    Probe("BYTE-POSITION", "0"),
    Probe("LDB", "0"),
    Probe("DPB", "1"),
    Probe("LDB-TEST", "NIL"),
    Probe("MASK-FIELD", "0"),
    Probe("DEPOSIT-FIELD", "1"),
    Probe("BOOLE", "-4"),
    Probe("NUMERATOR", "1"),
    Probe("DENOMINATOR", "1"),
    Probe("RATIONAL", "1"),
    Probe("FLOAT", "3.0"),
    Probe("DECODE-FLOAT", "0.5"),
    Probe("INTEGER-DECODE-FLOAT", "4503599627370496"),
    Probe("FLOAT-DIGITS", "53"),
    Probe("FLOAT-PRECISION", "53"),
    Probe("FLOAT-RADIX", "2"),
    Probe("SCALE-FLOAT", "4.0"),
    Probe("RATIONALIZE", "1"),
    Probe("FLOAT-SIGN", "1.0"),
    Probe("RANDOM", "0"),
    Probe("MAKE-RANDOM-STATE", "T"),
    Probe("RANDOM-STATE-P", "NIL"),
];

struct XFail {
    name: &'static str,
    source: &'static str,
    stderr: &'static str,
    exit_code: i32,
}
const XFAILS: &[XFail] = &[];

fn expression(name: &str) -> String {
    match name {
        "+" => "(+ 1 2)".to_owned(),
        "-" => "(- 3 1)".to_owned(),
        "*" => "(* 2 3)".to_owned(),
        "/" => "(/ 2 1)".to_owned(),
        "=" | "/=" | "<" | ">" | "<=" | ">=" | "MAX" | "MIN" | "EQ" | "EQL" | "MOD" | "REM"
        | "EXPT" | "COMPLEX" | "LOGNAND" | "LOGNOR" | "LOGANDC1" | "LOGANDC2" | "LOGORC1"
        | "LOGORC2" | "LOGTEST" | "LOGBITP" | "ASH" | "BYTE" | "LDB" | "LDB-TEST"
        | "MASK-FIELD" => format!("({name} 1 2)"),
        "DPB" | "DEPOSIT-FIELD" | "BOOLE" => format!("({name} 1 2 3)"),
        "DECODE-FLOAT"
        | "INTEGER-DECODE-FLOAT"
        | "FLOAT-DIGITS"
        | "FLOAT-PRECISION"
        | "FLOAT-RADIX" => format!("({name} 1.0)"),
        "SCALE-FLOAT" => "(scale-float 1.0 2)".to_owned(),
        "FLOAT-SIGN" => "(float-sign 1.0)".to_owned(),
        "RANDOM" => "(random 1)".to_owned(),
        "MAKE-RANDOM-STATE" => "(random-state-p (make-random-state nil))".to_owned(),
        "RANDOM-STATE-P" => "(random-state-p nil)".to_owned(),
        "FLOAT" => "(float 3 1.0)".to_owned(),
        _ => format!("({name} 1)"),
    }
}

fn run_ncl(source: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ncl"))
        .args(["--eval", source])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{source}: spawn failed: {error}"));
    let deadline = Instant::now() + CHILD_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .unwrap_or_else(|error| panic!("{source}: collect failed: {error}"));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                child
                    .kill()
                    .unwrap_or_else(|error| panic!("{source}: timeout kill failed: {error}"));
                let _ = child.wait();
                panic!("{source}: ncl did not exit within {CHILD_TIMEOUT:?}");
            }
            Err(error) => panic!("{source}: poll failed: {error}"),
        }
    }
}

#[test]
fn every_registered_builtin_is_executed_and_asserts_ansi_output() {
    assert_eq!(PROBES.len(), 104);
    let names = PROBES.iter().map(|probe| probe.0).collect::<BTreeSet<_>>();
    assert_eq!(names.len(), PROBES.len());
    for probe in PROBES {
        let source = expression(probe.0);
        let output = run_ncl(&source);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}: stderr={}",
            probe.0,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "{}: unexpected stderr: {}",
            probe.0,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim_end(),
            probe.1,
            "{}: {source}",
            probe.0
        );
    }
}

#[test]
fn known_non_working_cases_remain_explicit_xfails() {
    for case in XFAILS {
        let output = run_ncl(case.source);
        assert_eq!(
            output.status.code(),
            Some(case.exit_code),
            "{}: exit status",
            case.name
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(case.stderr),
            "{}: stderr={:?}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn assert_eval(source: &str, expected: &str) {
    let output = run_ncl(source);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{source}: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{source}: unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim_end(),
        expected,
        "{source}"
    );
}

/// `+`/`-`/`*`/`<` compile a dedicated two-argument fast path
/// (`packages/compiler/front/src/lower/expr.rs`); every other arity falls
/// through to an ordinary call. Both paths must agree with the interpreted
/// `funcall`/`apply` dispatch across every numeric type.
#[test]
fn native_fast_path_handles_every_arity_and_numeric_type() {
    for (source, expected) in [
        ("(+)", "0"),
        ("(+ 5)", "5"),
        ("(+ 1 2 3)", "6"),
        ("(+ 5 2 1)", "8"),
        ("(- 5)", "-5"),
        ("(- 5 2 1)", "2"),
        ("(*)", "1"),
        ("(* 5)", "5"),
        ("(* 2 3 4)", "24"),
        ("(* 5 2 1)", "10"),
        ("(< 1 2 3)", "T"),
        ("(< 1 3 2)", "NIL"),
        ("(+ 1.5 2)", "3.5"),
        ("(+ 2 1.5)", "3.5"),
        ("(* 2 5.0)", "10.0"),
        ("(- 0 5.0)", "-5.0"),
        ("(+ (/ 1 2) (/ 1 3))", "5/6"),
        ("(* (/ 1 3) 3)", "1"),
        ("(= 1 1.0)", "T"),
        ("(> (/ 1 2) 0.6)", "NIL"),
        ("(< (/ 1 2) 0.6)", "T"),
        ("(funcall #'+ 1 2 3)", "6"),
        ("(apply #'* '(2 3 4))", "24"),
    ] {
        assert_eval(source, expected);
    }
}

/// Overflowing the fixnum range inside the compiled two-argument fast path
/// must promote to a bignum instead of signaling a native `Overflow`
/// failure (`packages/sys/src/native_builtins.rs`, previously fatal).
#[test]
fn fixnum_overflow_promotes_to_bignum() {
    for (source, expected) in [
        ("(* most-positive-fixnum 2)", "9223372036854775806"),
        ("(+ most-positive-fixnum 1)", "4611686018427387904"),
        ("(- most-negative-fixnum 1)", "-4611686018427387905"),
        (
            "(loop with r = 1 for i from 1 to 30 do (setq r (* r i)) finally (return r))",
            "265252859812191058636308480000000",
        ),
    ] {
        assert_eval(source, expected);
    }
}

/// `EXPT` must return an exact result for a rational base and an integer
/// exponent (`packages/lib/numbers/src/transcendental.rs`); only a float or
/// complex operand should fall back to the transcendental float path.
#[test]
fn expt_is_exact_for_rational_base_and_integer_exponent() {
    for (source, expected) in [
        ("(expt 2 10)", "1024"),
        ("(expt 2 100)", "1267650600228229401496703205376"),
        ("(expt (/ 2 3) 2)", "4/9"),
        ("(expt 2 -2)", "1/4"),
        ("(expt 2 0)", "1"),
        ("(expt 2.0 0.5)", "1.414213562373095"),
    ] {
        assert_eval(source, expected);
    }
}

/// Bignum, ratio, and complex literals must compile as constant-table heap
/// objects instead of the unconditional `Unsupported("quoted number")`
/// (`packages/compiler/front/src/lower/literal.rs`).
#[test]
fn bignum_ratio_and_complex_literals_compile() {
    for (source, expected) in [
        ("'12345678901234567890123", "12345678901234567890123"),
        ("(quote 1/3)", "1/3"),
        ("'#c(1 2)", "#C(1 2)"),
        ("(+ 1/3 1)", "4/3"),
        ("(- '12345678901234567890123 1)", "12345678901234567890122"),
        ("(car '(1/3 2/3))", "1/3"),
    ] {
        assert_eval(source, expected);
    }
}

/// The same overflow-driven bignum promotions, re-run under `gc_stress` +
/// `strict_forwarding` in-process so a collection landing mid-allocation
/// cannot leave a stale `Word` behind.
///
/// This intentionally sticks to *computed* bignums (arithmetic overflow and
/// `expt`), not literal bignums/ratios: reading any quoted literal at all
/// (including a plain `'(1 2 3)`, unrelated to this lane) already fails
/// under `gc_stress` with `Front(Object(Storage(ThreadNotRegistered)))`, a
/// pre-existing gap in the reader/front-end literal-freezing path
/// (`packages/compiler/front/src/form.rs:130-152`, `packages/object/src/cons.rs:14,30`)
/// outside the numbers lane's scope.
#[test]
fn bignum_producing_arithmetic_survives_gc_stress_and_strict_forwarding() {
    let mut runtime = ncl_runtime::Runtime::new()
        .unwrap_or_else(|error| panic!("runtime initialization failed: {error:?}"));
    for (source, expected) in [
        ("(* most-positive-fixnum 2)", "9223372036854775806"),
        ("(+ most-positive-fixnum 1)", "4611686018427387904"),
        ("(expt 2 100)", "1267650600228229401496703205376"),
        (
            "(loop with r = 1 for i from 1 to 30 do (setq r (* r i)) finally (return r))",
            "265252859812191058636308480000000",
        ),
    ] {
        assert_eval_with_stress(&mut runtime, source, expected);
    }
}
