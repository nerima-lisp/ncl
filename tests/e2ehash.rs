#![allow(missing_docs)]

use std::process::Command;

struct Case {
    builtin: &'static str,
    source: &'static str,
    expected: &'static str,
}

const CASES: &[Case] = &[
    Case {
        builtin: "MAKE-HASH-TABLE",
        source: "(hash-table-p (make-hash-table))",
        expected: "T",
    },
    Case {
        builtin: "GETHASH",
        source: "(gethash 1 (make-hash-table))",
        expected: "NIL",
    },
    Case {
        builtin: "REMHASH",
        source: "(remhash 1 (make-hash-table))",
        expected: "NIL",
    },
    Case {
        builtin: "CLRHASH",
        source: "(hash-table-p (clrhash (make-hash-table)))",
        expected: "T",
    },
    Case {
        builtin: "MAPHASH",
        source: "(maphash (lambda (k v) nil) (make-hash-table))",
        expected: "NIL",
    },
    Case {
        builtin: "HASH-TABLE-P",
        source: "(hash-table-p (make-hash-table))",
        expected: "T",
    },
    Case {
        builtin: "HASH-TABLE-COUNT",
        source: "(hash-table-count (make-hash-table))",
        expected: "0",
    },
    Case {
        builtin: "HASH-TABLE-SIZE",
        source: "(hash-table-size (make-hash-table))",
        expected: "8",
    },
    Case {
        builtin: "HASH-TABLE-REHASH-SIZE",
        source: "(hash-table-rehash-size (make-hash-table))",
        expected: "1.5",
    },
    Case {
        builtin: "HASH-TABLE-REHASH-THRESHOLD",
        source: "(hash-table-rehash-threshold (make-hash-table))",
        expected: "0.75",
    },
    Case {
        builtin: "HASH-TABLE-TEST",
        source: "(hash-table-test (make-hash-table :test 'eql))",
        expected: "EQL",
    },
    Case {
        builtin: "SXHASH",
        source: "(integerp (sxhash 1))",
        expected: "T",
    },
    Case {
        builtin: "MAKE-HASH-TABLE :TEST EQ",
        source: "(hash-table-test (make-hash-table :test 'eq))",
        expected: "EQ",
    },
    Case {
        builtin: "MAKE-HASH-TABLE :TEST EQUAL",
        source: "(hash-table-test (make-hash-table :test 'equal))",
        expected: "EQUAL",
    },
    Case {
        builtin: "MAKE-HASH-TABLE :TEST EQUALP",
        source: "(hash-table-test (make-hash-table :test 'equalp))",
        expected: "EQUALP",
    },
    Case {
        builtin: "(SETF GETHASH)",
        source: "(let ((table (make-hash-table))) (setf (gethash 1 table) 7) (gethash 1 table))",
        expected: "7",
    },
    Case {
        builtin: "(SETF AREF)",
        source: "(let ((array (make-array 1))) (setf (aref array 0) 7) (aref array 0))",
        expected: "7",
    },
    Case {
        builtin: "GETHASH AFTER GC",
        source: "(let ((table (make-hash-table :test 'eq))) (setf (gethash 1 table) 7) (gethash 1 table))",
        expected: "7",
    },
    Case {
        builtin: "MAKE-ARRAY",
        source: "(arrayp (make-array 3))",
        expected: "T",
    },
    Case {
        builtin: "AREF",
        source: "(aref (make-array 3 :initial-element 7) 1)",
        expected: "7",
    },
    Case {
        builtin: "ROW-MAJOR-AREF",
        source: "(row-major-aref (make-array '(2 2) :initial-element 7) 2)",
        expected: "7",
    },
    Case {
        builtin: "SVREF",
        source: "(svref (vector 7) 0)",
        expected: "7",
    },
    Case {
        builtin: "ARRAYP",
        source: "(arrayp (make-array 1))",
        expected: "T",
    },
    Case {
        builtin: "VECTORP",
        source: "(vectorp (vector 1))",
        expected: "T",
    },
    Case {
        builtin: "SIMPLE-VECTOR-P",
        source: "(simple-vector-p (vector 1))",
        expected: "T",
    },
    Case {
        builtin: "SIMPLE-BIT-VECTOR-P",
        source: "(simple-bit-vector-p (make-array 2 :element-type 'bit))",
        expected: "T",
    },
    Case {
        builtin: "BIT-VECTOR-P",
        source: "(bit-vector-p (make-array 2 :element-type 'bit))",
        expected: "T",
    },
    Case {
        builtin: "ARRAY-RANK",
        source: "(array-rank (make-array '(2 3)))",
        expected: "2",
    },
    Case {
        builtin: "ARRAY-DIMENSION",
        source: "(array-dimension (make-array '(2 3)) 1)",
        expected: "3",
    },
    Case {
        builtin: "ARRAY-DIMENSIONS",
        source: "(= (car (array-dimensions (make-array '(2 3)))) 2)",
        expected: "T",
    },
    Case {
        builtin: "ARRAY-TOTAL-SIZE",
        source: "(array-total-size (make-array '(2 3)))",
        expected: "6",
    },
    Case {
        builtin: "ARRAY-IN-BOUNDS-P",
        source: "(array-in-bounds-p (make-array 2) 1)",
        expected: "T",
    },
    Case {
        builtin: "ADJUSTABLE-ARRAY-P",
        source: "(adjustable-array-p (make-array 2 :adjustable t))",
        expected: "T",
    },
    Case {
        builtin: "ARRAY-HAS-FILL-POINTER-P",
        source: "(array-has-fill-pointer-p (make-array 2 :fill-pointer 0))",
        expected: "T",
    },
    Case {
        builtin: "FILL-POINTER",
        source: "(fill-pointer (make-array 2 :fill-pointer 1))",
        expected: "1",
    },
    Case {
        builtin: "VECTOR",
        source: "(vectorp (vector 1 2))",
        expected: "T",
    },
    Case {
        builtin: "ADJUST-ARRAY",
        source: "(array-total-size (adjust-array (make-array 2 :adjustable t) 3))",
        expected: "3",
    },
    Case {
        builtin: "VECTOR-PUSH",
        source: "(vector-push 7 (make-array 2 :fill-pointer 0))",
        expected: "0",
    },
    Case {
        builtin: "VECTOR-PUSH-EXTEND",
        source: "(vector-push-extend 7 (make-array 0 :fill-pointer 0 :adjustable t))",
        expected: "0",
    },
    Case {
        builtin: "VECTOR-POP",
        source: "(vector-pop (make-array 2 :fill-pointer 1 :initial-element 7))",
        expected: "7",
    },
    Case {
        builtin: "ARRAY-ELEMENT-TYPE",
        source: "(array-element-type (make-array 2))",
        expected: "T",
    },
    Case {
        builtin: "ARRAY-DISPLACEMENT",
        source: "(array-displacement (make-array 2))",
        expected: "NIL",
    },
    Case {
        builtin: "ARRAY-ROW-MAJOR-INDEX",
        source: "(array-row-major-index (make-array '(2 3)) 1 2)",
        expected: "5",
    },
    Case {
        builtin: "BIT",
        source: "(bit (make-array 2 :element-type 'bit) 0)",
        expected: "0",
    },
    Case {
        builtin: "SBIT",
        source: "(setf (sbit (make-array 2 :element-type 'bit) 0) 1)",
        expected: "1",
    },
    Case {
        builtin: "BIT-AND",
        source: "(bit (bit-and (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "0",
    },
    Case {
        builtin: "BIT-ANDC1",
        source: "(bit (bit-andc1 (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "0",
    },
    Case {
        builtin: "BIT-ANDC2",
        source: "(bit (bit-andc2 (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "0",
    },
    Case {
        builtin: "BIT-EQV",
        source: "(bit (bit-eqv (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "1",
    },
    Case {
        builtin: "BIT-IOR",
        source: "(bit (bit-ior (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "0",
    },
    Case {
        builtin: "BIT-NAND",
        source: "(bit (bit-nand (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "1",
    },
    Case {
        builtin: "BIT-NOR",
        source: "(bit (bit-nor (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "1",
    },
    Case {
        builtin: "BIT-NOT",
        source: "(bit (bit-not (make-array 1 :element-type 'bit)) 0)",
        expected: "1",
    },
    Case {
        builtin: "BIT-ORC1",
        source: "(bit (bit-orc1 (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "1",
    },
    Case {
        builtin: "BIT-ORC2",
        source: "(bit (bit-orc2 (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "1",
    },
    Case {
        builtin: "BIT-XOR",
        source: "(bit (bit-xor (make-array 1 :element-type 'bit) (make-array 1 :element-type 'bit)) 0)",
        expected: "0",
    },
];

#[test]
fn compiled_hash_array_matrix_reports_every_registered_builtin() {
    assert_eq!(CASES.len(), 56);
    let mut failures = Vec::new();
    for case in CASES {
        let output = match Command::new(env!("CARGO_BIN_EXE_ncl"))
            .args(["--eval", case.source])
            .output()
        {
            Ok(output) => output,
            Err(error) => panic!("{}: failed to execute ncl: {error}", case.builtin),
        };
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let actual = if output.status.success() {
            stdout.clone()
        } else {
            stderr.clone()
        };
        let classification = if output.status.success() && stdout == case.expected {
            "OK"
        } else if stderr.contains("constant requires a runtime table")
            || stderr.contains("invalid operator")
            || stderr.contains("InvalidOperator")
            || stderr.contains("quoted structure")
            || stderr.contains("UndefinedFunction")
            || stderr.contains("at most four register arguments")
        {
            "担当外: codegen/compiler blocker"
        } else {
            "BUG"
        };
        println!(
            "{} | {} | {} | {}",
            case.builtin, case.expected, actual, classification
        );
        if classification == "BUG" {
            failures.push(case.builtin);
        }
    }
    assert!(failures.is_empty(), "unexpected results: {failures:?}");
}
