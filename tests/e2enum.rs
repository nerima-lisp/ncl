#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
    Probe("EXPT", "1.0"),
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
    Probe("BYTE-SIZE", "2"),
    Probe("BYTE-POSITION", "0"),
    Probe("LDB", "2"),
    Probe("DPB", "1"),
    Probe("LDB-TEST", "T"),
    Probe("MASK-FIELD", "2"),
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
