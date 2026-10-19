import importlib.util
import sys
import tempfile
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
            self.assertTrue(any(f"[{rule}]" in item for item in failures), failures)

    def test_detects_unsupported_and_word_tables(self):
        failures = checker.check_added_lines(make_diff(
            "packages/object/src/value.rs",
            "let _table: HashMap<Word, Word> = HashMap::new();\n"
            "return Err(ObjectError::Unsupported);",
        ))
        self.assertTrue(any("[word-table]" in item for item in failures), failures)
        self.assertTrue(any("[unsupported]" in item for item in failures), failures)

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

    def test_detects_indexing_through_mutable_bindings(self):
        failures = checker.check_added_lines(make_diff(
            "packages/object/src/value.rs",
            "let mut values = vec![1];\nvalues[0] = 2;",
        ))
        self.assertEqual(
            [item for item in failures if "[index]" in item],
            ["packages/object/src/value.rs:2: [index] unchecked indexing"],
        )

    def test_ignores_slice_patterns_and_slice_types(self):
        diff = make_diff(
            "packages/object/src/value.rs",
            "let [mut quality, mut value] = elements.as_slice();\n"
            "fn descriptor(required: &'static [Parameter]) {}\n"
            "fn borrowed<'a>(optional: &'a [Parameter]) {}",
        )
        self.assertNotIn("[index]", " ".join(checker.check_added_lines(diff)))

    def test_suppresses_cfg_test_external_module_filename(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            source = root / "packages/object/src"
            source.mkdir(parents=True)
            (root / "packages/object/Cargo.toml").write_text("[package]\nname = \"object\"\n")
            (source / "lib.rs").write_text("#[cfg(test)] mod value_tests;\n")
            (source / "value_tests.rs").write_text("values[0];\npanic!();\nUnsupported;\n")
            previous_root = checker.ROOT
            checker.ROOT = root
            try:
                failures = checker.check_added_lines(make_diff(
                    "packages/object/src/value_tests.rs",
                    "values[0];\npanic!();\nUnsupported;",
                ))
            finally:
                checker.ROOT = previous_root
        self.assertEqual(failures, [])

    def test_checks_external_test_module_filename_without_cfg_test(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            source = root / "packages/object/src"
            source.mkdir(parents=True)
            (root / "packages/object/Cargo.toml").write_text("[package]\nname = \"object\"\n")
            (source / "lib.rs").write_text("mod value_tests;\n")
            (source / "value_tests.rs").write_text(
                "let _ = values[0];\n"
                "let _ = 1 as u32;\n"
                "let _ = value.unwrap();\n"
                "panic!();\n"
                "let _ = Word::UNBOUND;\n"
                "todo!();\n"
                "let _ = unsafe { 1 };\n"
                "let _table: HashMap<Word, Word> = HashMap::new();\n"
                "return Err(ObjectError::Unsupported);\n"
                "match value { _ => {} }\n"
            )
            previous_root = checker.ROOT
            checker.ROOT = root
            try:
                failures = checker.check_added_lines(make_diff(
                    "packages/object/src/value_tests.rs",
                    "let _ = values[0];\n"
                    "let _ = 1 as u32;\n"
                    "let _ = value.unwrap();\n"
                    "panic!();\n"
                    "let _ = Word::UNBOUND;\n"
                    "todo!();\n"
                    "let _ = unsafe { 1 };\n"
                    "let _table: HashMap<Word, Word> = HashMap::new();\n"
                    "return Err(ObjectError::Unsupported);\n"
                    "match value { _ => {} }",
                ))
            finally:
                checker.ROOT = previous_root
        tags = " ".join(failures)
        for rule in (
            "index", "as-cast", "panic", "unbound", "todo", "unsafe",
            "word-table", "unsupported", "wildcard",
        ):
            self.assertIn(f"[{rule}]", tags)

    def test_external_module_declaration_does_not_mask_production_code(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            source = root / "packages/object/src"
            source.mkdir(parents=True)
            (root / "packages/object/Cargo.toml").write_text(
                "[package]\nname = \"object\"\n"
            )
            code = "#[cfg(test)] mod tests;\nfn production() { values[0]; }"
            (source / "lib.rs").write_text(code)
            (source / "tests.rs").write_text("")
            previous_root = checker.ROOT
            checker.ROOT = root
            try:
                failures = checker.check_added_lines(
                    make_diff("packages/object/src/lib.rs", code)
                )
            finally:
                checker.ROOT = previous_root
        self.assertIn("[index]", " ".join(failures))

    def test_suppresses_test_paths_and_cfg_test_source_rules(self):
        self.assertEqual(
            checker.check_added_lines(make_diff("packages/object/tests/example.rs", "values[0]; todo!();")),
            [],
        )
        self.assertIn(
            "[index]",
            " ".join(checker.check_added_lines(
                make_diff("packages/object/src/tests/example.rs", "values[0];")
            )),
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

    def test_checks_cargo_tests_file_imported_by_production_module(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            source = root / "packages/object/src"
            integration_tests = root / "packages/object/tests"
            source.mkdir(parents=True)
            integration_tests.mkdir(parents=True)
            (root / "packages/object/Cargo.toml").write_text(
                "[package]\nname = \"object\"\n"
            )
            (source / "lib.rs").write_text(
                '#[path = "../tests/value_tests.rs"] mod value_tests;\n'
            )
            (integration_tests / "value_tests.rs").write_text("values[0];\n")
            previous_root = checker.ROOT
            checker.ROOT = root
            try:
                failures = checker.check_added_lines(make_diff(
                    "packages/object/tests/value_tests.rs",
                    "values[0];",
                ))
            finally:
                checker.ROOT = previous_root
        self.assertIn("[index]", " ".join(failures))

    def test_checks_cargo_tests_file_imported_from_another_package(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            source = root / "packages/object/src"
            integration_tests = root / "packages/fixture/tests"
            source.mkdir(parents=True)
            integration_tests.mkdir(parents=True)
            (root / "Cargo.toml").write_text(
                '[workspace]\nmembers = ["packages/object", "packages/fixture"]\n'
            )
            (root / "packages/object/Cargo.toml").write_text(
                "[package]\nname = \"object\"\n"
            )
            (root / "packages/fixture/Cargo.toml").write_text(
                "[package]\nname = \"fixture\"\n"
            )
            (source / "lib.rs").write_text(
                '#[path = "../../fixture/tests/value_tests.rs"] mod value_tests;\n'
            )
            (integration_tests / "value_tests.rs").write_text("values[0];\n")
            previous_root = checker.ROOT
            checker.ROOT = root
            try:
                failures = checker.check_added_lines(make_diff(
                    "packages/fixture/tests/value_tests.rs",
                    "values[0];",
                ))
            finally:
                checker.ROOT = previous_root
        self.assertIn("[index]", " ".join(failures))

    def test_checks_cfg_test_named_source_imported_from_another_package(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            object_source = root / "packages/object/src"
            fixture_source = root / "packages/fixture/src"
            object_source.mkdir(parents=True)
            fixture_source.mkdir(parents=True)
            (root / "Cargo.toml").write_text(
                '[workspace]\nmembers = ["packages/object", "packages/fixture"]\n'
            )
            (root / "packages/object/Cargo.toml").write_text(
                "[package]\nname = \"object\"\n"
            )
            (root / "packages/fixture/Cargo.toml").write_text(
                "[package]\nname = \"fixture\"\n"
            )
            (object_source / "lib.rs").write_text(
                "#[cfg(test)] mod value_tests;\n"
            )
            (object_source / "value_tests.rs").write_text("values[0];\n")
            (fixture_source / "lib.rs").write_text(
                '#[path = "../../object/src/value_tests.rs"] mod shared_tests;\n'
            )
            previous_root = checker.ROOT
            checker.ROOT = root
            try:
                failures = checker.check_added_lines(make_diff(
                    "packages/object/src/value_tests.rs",
                    "values[0];",
                ))
            finally:
                checker.ROOT = previous_root
        self.assertIn("[index]", " ".join(failures))

    def test_checks_mixed_cfg_test_and_production_parents_in_workspace(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            object_source = root / "packages/object/src"
            fixture_source = root / "packages/fixture/src"
            fixture_tests = root / "packages/fixture/tests"
            object_source.mkdir(parents=True)
            fixture_source.mkdir(parents=True)
            fixture_tests.mkdir(parents=True)
            (root / "Cargo.toml").write_text(
                '[workspace]\nmembers = ["packages/object", "packages/fixture"]\n'
            )
            (root / "packages/object/Cargo.toml").write_text(
                "[package]\nname = \"object\"\n"
            )
            (root / "packages/fixture/Cargo.toml").write_text(
                "[package]\nname = \"fixture\"\n"
            )
            (object_source / "lib.rs").write_text(
                '#[path = "../../fixture/tests/value_tests.rs"] mod shared_tests;\n'
            )
            (fixture_source / "lib.rs").write_text(
                '#[cfg(test)] #[path = "../tests/value_tests.rs"] mod value_tests;\n'
            )
            (fixture_tests / "value_tests.rs").write_text("values[0];\n")
            previous_root = checker.ROOT
            checker.ROOT = root
            try:
                failures = checker.check_added_lines(make_diff(
                    "packages/fixture/tests/value_tests.rs",
                    "values[0];",
                ))
            finally:
                checker.ROOT = previous_root
        self.assertIn("[index]", " ".join(failures))

    def test_checks_cargo_tests_file_included_with_concat_literals(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            source = root / "packages/object/src"
            integration_tests = root / "packages/object/tests"
            source.mkdir(parents=True)
            integration_tests.mkdir(parents=True)
            (root / "packages/object/Cargo.toml").write_text(
                "[package]\nname = \"object\"\n"
            )
            (source / "lib.rs").write_text(
                'include!(concat!("../", "tests/", "value_tests.rs"));\n'
            )
            (integration_tests / "value_tests.rs").write_text("values[0];\n")
            previous_root = checker.ROOT
            checker.ROOT = root
            try:
                failures = checker.check_added_lines(make_diff(
                    "packages/object/tests/value_tests.rs",
                    "values[0];",
                ))
            finally:
                checker.ROOT = previous_root
        self.assertIn("[index]", " ".join(failures))

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
