#![allow(missing_docs)]

use std::collections::BTreeSet;
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Classification {
    Ok,
    OutOfScope,
}

struct Probe {
    name: &'static str,
    classification: Classification,
}

const PROBES: &[Probe] = &[
    Probe {
        name: "NUMBERP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "INTEGERP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "RATIONALP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FLOATP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "REALP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "COMPLEXP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "+",
        classification: Classification::Ok,
    },
    Probe {
        name: "-",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "*",
        classification: Classification::Ok,
    },
    Probe {
        name: "/",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "=",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "EQ",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "EQL",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "/=",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "<",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: ">",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "<=",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: ">=",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "MAX",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "MIN",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "1+",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "1-",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ABS",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "SIGNUM",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ZEROP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "PLUSP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "MINUSP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "EVENP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ODDP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "MOD",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "REM",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "GCD",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LCM",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ISQRT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "EXP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "EXPT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "SQRT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "SIN",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "COS",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "TAN",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ASIN",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ACOS",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "SINH",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "COSH",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "TANH",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ASINH",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ACOSH",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ATANH",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "COMPLEX",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "CONJUGATE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "CIS",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "PHASE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "REALPART",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "IMAGPART",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOG",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ATAN",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FLOOR",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "CEILING",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "TRUNCATE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ROUND",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FFLOOR",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FCEILING",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FTRUNCATE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FROUND",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGAND",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGIOR",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGXOR",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGNOT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGEQV",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGNAND",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGNOR",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGANDC1",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGANDC2",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGORC1",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGORC2",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGTEST",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGBITP",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LOGCOUNT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "INTEGER-LENGTH",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "ASH",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "BYTE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "BYTE-SIZE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "BYTE-POSITION",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LDB",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "DPB",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "LDB-TEST",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "MASK-FIELD",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "DEPOSIT-FIELD",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "BOOLE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "NUMERATOR",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "DENOMINATOR",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "RATIONAL",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FLOAT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "DECODE-FLOAT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "INTEGER-DECODE-FLOAT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FLOAT-DIGITS",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FLOAT-PRECISION",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FLOAT-RADIX",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "SCALE-FLOAT",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "RATIONALIZE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "FLOAT-SIGN",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "RANDOM",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "MAKE-RANDOM-STATE",
        classification: Classification::OutOfScope,
    },
    Probe {
        name: "RANDOM-STATE-P",
        classification: Classification::OutOfScope,
    },
];

#[test]
fn table_contains_every_numbers_builtin_and_records_observed_scope() {
    assert_eq!(PROBES.len(), 104);
    let names = PROBES
        .iter()
        .map(|probe| probe.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), PROBES.len());
    assert_eq!(
        PROBES
            .iter()
            .filter(|probe| probe.classification == Classification::Ok)
            .count(),
        2
    );
    assert!(PROBES.iter().all(|probe| !probe.name.is_empty()));
}

#[test]
fn compiled_native_smoke_rows_match_the_recorded_results() {
    for probe in PROBES
        .iter()
        .filter(|probe| probe.classification == Classification::Ok)
    {
        let (expression, expected) = match probe.name {
            "+" => ("(+ 1 2)", "3"),
            "*" => ("(* 2 3)", "6"),
            _ => panic!("missing smoke expression for {}", probe.name),
        };
        let output = Command::new(env!("CARGO_BIN_EXE_ncl"))
            .args(["--eval", expression])
            .output()
            .unwrap_or_else(|error| panic!("{}: {error}", probe.name));
        assert!(output.status.success(), "{}: {:?}", probe.name, output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected,
            "{}",
            probe.name
        );
    }
}
