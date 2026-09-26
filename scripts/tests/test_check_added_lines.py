import importlib.util
import sys
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).parents[1] / "check_added_lines.py"
spec = importlib.util.spec_from_file_location("check_added_lines", MODULE_PATH)
checker = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = checker
spec.loader.exec_module(checker)


def make_diff(path, added):
    lines = added.splitlines()
    return "\n".join(
        [
            f"diff --git a/{path} b/{path}",
            f"--- a/{path}",
            f"+++ b/{path}",
            f"@@ -1,0 +1,{len(lines)} @@",
            *(f"+{line}" for line in lines),
        ]
    )


class CheckAddedLinesTests(unittest.TestCase):
    def test_detects_each_rule(self):
        diff = make_diff(
            "packages/object/src/value.rs",
            """let _ = values[0];
let _ = 1 as u32;
let _ = option.unwrap();
match value { _ => {} }
let _ = Word::UNBOUND;
todo!();
let _ = unsafe { read(); };
""",
        )
        failures = checker.check_added_lines(diff)
        for rule in ("index", "as-cast", "panic", "wildcard", "unbound", "todo", "unsafe"):
            if rule == "unbound":
                self.assertTrue(any("[unbound]" in item for item in checker.unbound_warnings(checker.added_lines(diff))))
            else:
                self.assertTrue(any(f"[{rule}]" in item for item in failures), failures)

    def test_detects_em_dash_and_external_dependency(self):
        diff = make_diff("README.md", "A documented choice " + chr(0x2014) + " with a reason.")
        diff += "\n" + make_diff("packages/object/Cargo.toml", 'serde = "1"')
        failures = checker.check_added_lines(diff)
        self.assertTrue(any("[em-dash]" in item for item in failures), failures)
        self.assertTrue(any("[dependency]" in item for item in failures), failures)

    def test_avoids_literals_types_macros_and_similar_identifiers(self):
        diff = make_diff(
            "packages/object/src/value.rs",
            """let _typed: [u8; 4] = [0; 4];
let _vector = vec![1, 2];
let _guard = lock().unwrap_or_else(PoisonError::into_inner);
let _text = "x[0] as u8 unwrap()";
let _name = UNBOUNDARY;
use crate::thing as Alias;
let _converted = value as Alias;
""",
        )
        self.assertEqual(checker.check_added_lines(diff), [])

    def test_suppresses_test_paths_and_cfg_test_source_rules(self):
        self.assertEqual(
            checker.check_added_lines(make_diff("packages/object/tests/example.rs", "values[0]; todo!();")),
            [],
        )
        diff = make_diff(
            "packages/object/src/value.rs",
            """#[cfg(test)]
mod tests {
    fn test_it() { values[0]; }
}
let _ = values[0];
""",
        )
        failures = checker.check_added_lines(diff)
        self.assertEqual(sum("[index]" in item for item in failures), 1)

    def test_applies_allow_comment_to_following_line(self):
        diff = make_diff(
            "packages/object/src/value.rs",
            "// check-added-lines: allow(index) parser proves bounds\nvalues[0];\n1 as u32;",
        )
        failures = checker.check_added_lines(diff)
        self.assertNotIn("[index]", " ".join(failures))
        self.assertIn("[as-cast]", " ".join(failures))

    def test_previous_line_allow_requires_reason_and_is_single_line(self):
        diff = make_diff(
            "packages/object/src/value.rs",
            "// check-added-lines: allow(index) bounds checked above\nvalues[0];\nvalues[1];",
        )
        failures = checker.check_added_lines(diff)
        self.assertEqual(sum("[index]" in item for item in failures), 1)
        empty = make_diff(
            "packages/object/src/value.rs",
            "// check-added-lines: allow(index)\nvalues[0];",
        )
        self.assertTrue(any("[index]" in item for item in checker.check_added_lines(empty)))

    def test_allows_unsafe_in_sys_and_workspace_dependencies(self):
        self.assertEqual(
            checker.check_added_lines(make_diff("packages/sys/src/raw.rs", "unsafe {};")), []
        )
        self.assertEqual(
            checker.check_added_lines(
                make_diff("packages/object/Cargo.toml", 'local = { workspace = true }')
            ),
            [],
        )

    def test_ignores_non_added_lines_and_non_rust_source_rules(self):
        diff = "\n".join(
            [
                "diff --git a/src/lib.rs b/src/lib.rs",
                "--- a/src/lib.rs",
                "+++ b/src/lib.rs",
                "@@ -1,2 +1,2 @@",
                "-let _ = values[0];",
                " let _ = values[0];",
                "+let _ = 1;",
            ]
        )
        self.assertEqual(checker.check_added_lines(diff), [])
        self.assertEqual(checker.added_rust_lines(make_diff("README.md", "values[0];")), [])


if __name__ == "__main__":
    unittest.main()
