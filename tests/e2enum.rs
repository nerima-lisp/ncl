#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Classification {
    Ok,
    Crash,
    TypedError,
    NonTypedFailure,
}

struct Probe {
    name: &'static str,
    classification: Classification,
}

macro_rules! probe {
    ($name:literal) => {
        Probe {
            name: $name,
            classification: Classification::NonTypedFailure,
        }
    };
}

const PROBES: &[Probe] = &[
    probe!("NUMBERP"),
    probe!("INTEGERP"),
    probe!("RATIONALP"),
    probe!("FLOATP"),
    probe!("REALP"),
    probe!("COMPLEXP"),
    probe!("+"),
    probe!("-"),
    probe!("*"),
    probe!("/"),
    probe!("="),
    probe!("EQ"),
    probe!("EQL"),
    probe!("/="),
    probe!("<"),
    probe!(">"),
    probe!("<="),
    probe!(">="),
    probe!("MAX"),
    probe!("MIN"),
    probe!("1+"),
    probe!("1-"),
    probe!("ABS"),
    probe!("SIGNUM"),
    probe!("ZEROP"),
    probe!("PLUSP"),
    probe!("MINUSP"),
    probe!("EVENP"),
    probe!("ODDP"),
    probe!("MOD"),
    probe!("REM"),
    probe!("GCD"),
    probe!("LCM"),
    probe!("ISQRT"),
    probe!("EXP"),
    probe!("EXPT"),
    probe!("SQRT"),
    probe!("SIN"),
    probe!("COS"),
    probe!("TAN"),
    probe!("ASIN"),
    probe!("ACOS"),
    probe!("SINH"),
    probe!("COSH"),
    probe!("TANH"),
    probe!("ASINH"),
    probe!("ACOSH"),
    probe!("ATANH"),
    probe!("COMPLEX"),
    probe!("CONJUGATE"),
    probe!("CIS"),
    probe!("PHASE"),
    probe!("REALPART"),
    probe!("IMAGPART"),
    probe!("LOG"),
    probe!("ATAN"),
    probe!("FLOOR"),
    probe!("CEILING"),
    probe!("TRUNCATE"),
    probe!("ROUND"),
    probe!("FFLOOR"),
    probe!("FCEILING"),
    probe!("FTRUNCATE"),
    probe!("FROUND"),
    probe!("LOGAND"),
    probe!("LOGIOR"),
    probe!("LOGXOR"),
    probe!("LOGNOT"),
    probe!("LOGEQV"),
    probe!("LOGNAND"),
    probe!("LOGNOR"),
    probe!("LOGANDC1"),
    probe!("LOGANDC2"),
    probe!("LOGORC1"),
    probe!("LOGORC2"),
    probe!("LOGTEST"),
    probe!("LOGBITP"),
    probe!("LOGCOUNT"),
    probe!("INTEGER-LENGTH"),
    probe!("ASH"),
    probe!("BYTE"),
    probe!("BYTE-SIZE"),
    probe!("BYTE-POSITION"),
    probe!("LDB"),
    probe!("DPB"),
    probe!("LDB-TEST"),
    probe!("MASK-FIELD"),
    probe!("DEPOSIT-FIELD"),
    probe!("BOOLE"),
    probe!("NUMERATOR"),
    probe!("DENOMINATOR"),
    probe!("RATIONAL"),
    probe!("FLOAT"),
    probe!("DECODE-FLOAT"),
    probe!("INTEGER-DECODE-FLOAT"),
    probe!("FLOAT-DIGITS"),
    probe!("FLOAT-PRECISION"),
    probe!("FLOAT-RADIX"),
    probe!("SCALE-FLOAT"),
    probe!("RATIONALIZE"),
    probe!("FLOAT-SIGN"),
    probe!("RANDOM"),
    probe!("MAKE-RANDOM-STATE"),
    probe!("RANDOM-STATE-P"),
];

fn expression(probe: &Probe) -> String {
    match probe.name {
        "+" => "(+ 1 2)".to_owned(),
        "-" => "(- 3 1)".to_owned(),
        "*" => "(* 2 3)".to_owned(),
        "/" => "(/ 2 1)".to_owned(),
        "=" | "/=" | "<" | ">" | "<=" | ">=" | "MAX" | "MIN" => {
            format!("({} 1 2)", probe.name)
        }
        "EQ" | "EQL" | "MOD" | "REM" | "EXPT" | "COMPLEX" | "LOGNAND" | "LOGNOR" | "LOGANDC1"
        | "LOGANDC2" | "LOGORC1" | "LOGORC2" | "LOGTEST" | "LOGBITP" | "ASH" | "BYTE" | "LDB"
        | "LDB-TEST" | "MASK-FIELD" => format!("({} 1 2)", probe.name),
        "DPB" | "DEPOSIT-FIELD" | "BOOLE" => format!("({} 1 2 3)", probe.name),
        "DECODE-FLOAT"
        | "INTEGER-DECODE-FLOAT"
        | "FLOAT-DIGITS"
        | "FLOAT-PRECISION"
        | "FLOAT-RADIX"
        | "SCALE-FLOAT"
        | "FLOAT-SIGN" => {
            format!("({} 1.0 2)", probe.name)
        }
        "RANDOM" => "(random 10)".to_owned(),
        "MAKE-RANDOM-STATE" => "(make-random-state nil)".to_owned(),
        "RANDOM-STATE-P" => "(random-state-p nil)".to_owned(),
        "FLOAT" => "(float 3 1.0)".to_owned(),
        _ => format!("({} 1)", probe.name),
    }
}

fn classify(status: ExitStatus, stderr: &[u8]) -> Classification {
    if status.success() {
        Classification::Ok
    } else if status.signal().is_some() {
        Classification::Crash
    } else if String::from_utf8_lossy(stderr).contains("TypeError")
        || String::from_utf8_lossy(stderr).contains("object error")
    {
        Classification::TypedError
    } else {
        Classification::NonTypedFailure
    }
}

#[test]
fn every_registered_builtin_is_executed_and_classified() {
    assert_eq!(PROBES.len(), 104);
    let names = PROBES
        .iter()
        .map(|probe| probe.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), PROBES.len());
    for probe in PROBES {
        let source = expression(probe);
        let mut child = Command::new(env!("CARGO_BIN_EXE_ncl"))
            .args(["--eval", &source])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|error| panic!("{}: {error}", probe.name));
        let deadline = Instant::now() + Duration::from_millis(100);
        let actual = loop {
            match child.try_wait() {
                Ok(Some(status)) => break classify(status, &[]),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                Ok(None) => {
                    if let Err(error) = child.kill() {
                        panic!("ncl timeout kill failed: {error}");
                    }
                    if let Err(error) = child.wait() {
                        panic!("ncl timeout wait failed: {error}");
                    }
                    break Classification::NonTypedFailure;
                }
                Err(error) => panic!("{}: {error}", probe.name),
            }
        };
        assert_eq!(actual, probe.classification, "{}: {source}", probe.name);
    }
}
