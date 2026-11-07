from __future__ import annotations

import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))

import run
from v3_programs import (
    ast_size,
    form_cost,
    generate_expression,
    is_valid_expression,
    render_form,
    render_program,
    structural_replacements,
)


class ProgramGenerationTests(unittest.TestCase):
    def test_seed_replays_the_same_expression(self):
        for seed in range(64):
            with self.subTest(seed=seed):
                self.assertEqual(generate_expression(seed), generate_expression(seed))

    def test_generated_expressions_are_bounded_and_renderable(self):
        forms = [generate_expression(seed) for seed in range(128)]
        sizes = [ast_size(form) for form in forms]

        self.assertEqual(len(forms), 128)
        self.assertEqual(max(sizes), 127)
        self.assertTrue(all(size <= 127 for size in sizes))
        self.assertTrue(all(is_valid_expression(form) for form in forms))
        self.assertTrue(all(render_form(form) for form in forms))
        self.assertGreater(len({render_form(form) for form in forms}), 100)

    def test_seed_six_keeps_arithmetic_operands_integer_typed(self):
        form = generate_expression(6)

        self.assertEqual(
            render_form(form),
            "(aref (vector 17 (flet ((fa386 (x0b35) (progn 6 14))) (fa386 (progn 16 15)))) 1)",
        )
        self.assertTrue(is_valid_expression(form))

    def test_validator_rejects_invalid_arithmetic_control_and_array_operands(self):
        invalid_forms = (
            ["+", 1, ["vector", 2]],
            ["if", ["=", ["vector", 1], 2], 3, 4],
            ["aref", ["cons", 1, 2], 0],
            ["aref", ["vector", 1], ["cons", 0, 1]],
            ["mod", 3, 0],
            ["let", [["fn", ["lambda", ["x"], ["+", 1, "x"]]]], ["funcall", "fn", ["vector", 2]]],
        )

        for form in invalid_forms:
            with self.subTest(form=render_form(form)):
                self.assertFalse(is_valid_expression(form))

    def test_speed_policy_is_explicit_in_lisp_source(self):
        self.assertEqual(
            render_program(["+", 1, 2], "speed3"),
            '(declaim (optimize (speed 3)))\n(format t "~S~%" (+ 1 2))\n',
        )
        self.assertNotIn("declaim", render_program(["+", 1, 2], "default"))
        self.assertNotIn("declaim", render_program(["+", 1, 2], "off"))

    def test_shrinker_returns_strictly_smaller_deterministic_candidates(self):
        form = ["+", ["*", 7, 8], 3]

        first = structural_replacements(form)
        second = structural_replacements(form)

        self.assertEqual(first, second)
        self.assertTrue(first)
        self.assertTrue(all(form_cost(candidate) < form_cost(form) for candidate in first))
        self.assertIn(["+", 7, 3], first)

    def test_shrinker_candidates_preserve_generated_lisp_grammar_and_scope(self):
        forms = [generate_expression(seed) for seed in range(128)]
        operators = set()

        def record_operators(form):
            if isinstance(form, list):
                if form and isinstance(form[0], str):
                    operators.add(form[0])
                for item in form:
                    record_operators(item)

        for seed, form in enumerate(forms):
            self.assertTrue(is_valid_expression(form), f"invalid generated expression at seed {seed}")
            record_operators(form)
            for candidate in structural_replacements(form):
                self.assertTrue(
                    is_valid_expression(candidate),
                    f"invalid shrink candidate at seed {seed}: {render_form(candidate)}",
                )

        self.assertTrue({"let", "flet", "labels", "lambda", "funcall", "aref", "vector", "cons"} <= operators)

        let_form = ["let", [["x", 1]], ["+", "x", 2]]
        self.assertNotIn([["x", 1]], structural_replacements(let_form))


class RunnerTests(unittest.TestCase):
    def test_policy_commands_are_explicit(self):
        self.assertEqual(
            run._command("ncl", "/tmp/ncl", "off", "program.lisp"),
            ["/tmp/ncl", "--optimization=off", "--script", "program.lisp"],
        )
        self.assertEqual(
            run._command("ncl", "/tmp/ncl", "default", "program.lisp"),
            ["/tmp/ncl", "--script", "program.lisp"],
        )
        self.assertEqual(
            run._command("ncl", "/tmp/ncl", "speed3", "program.lisp"),
            ["/tmp/ncl", "--script", "program.lisp"],
        )
        self.assertEqual(
            run._command("sbcl", "/tmp/sbcl", "speed3", "program.lisp"),
            ["/tmp/sbcl", "--noinform", "--disable-debugger", "--script", "program.lisp"],
        )

    def test_oracle_version_must_match_the_pinned_release(self):
        with tempfile.TemporaryDirectory() as directory:
            ncl = Path(directory) / "ncl"
            sbcl = Path(directory) / "sbcl"
            ncl.touch()
            sbcl.touch()
            with patch.object(run, "_resolve_binary", side_effect=lambda value: value):
                with self.assertRaisesRegex(RuntimeError, "SBCL 2.6.10 is required"):
                    run.run_suite(
                        seed_count=1,
                        seed_start=0,
                        ncl=str(ncl),
                        sbcl=str(sbcl),
                        repro_dir=Path(directory) / "repros",
                        timeout=1,
                        version=lambda binary, engine: "SBCL 2.6.9" if engine == "sbcl" else "ncl 0.1",
                    )

    def test_oracle_version_accepts_the_pinned_vendor_build_only(self):
        run._require_sbcl_version("SBCL 2.6.10")
        run._require_sbcl_version("SBCL 2.6.10.roswell")
        for version in ("SBCL 2.6.9", "SBCL 2.6.100", "SBCL 2.6.10.roswell2"):
            with self.subTest(version=version):
                with self.assertRaisesRegex(RuntimeError, "SBCL 2.6.10 is required"):
                    run._require_sbcl_version(version)

    def test_oracle_crash_is_fatal_and_does_not_create_a_differential_repro(self):
        with tempfile.TemporaryDirectory() as directory:
            repro_dir = Path(directory) / "repros"
            calls = []

            def fake_execute(engine, binary, policy, form, timeout):
                calls.append(engine)
                return run.Execution("crash", "", -6, "oracle signal")

            with patch.object(run, "_resolve_binary", side_effect=lambda value: value):
                with self.assertRaisesRegex(RuntimeError, "SBCL failed"):
                    run.run_suite(
                        seed_count=1,
                        seed_start=0,
                        ncl="ncl",
                        sbcl="sbcl",
                        repro_dir=repro_dir,
                        timeout=1,
                        execute=fake_execute,
                        version=lambda binary, engine: "SBCL 2.6.10" if engine == "sbcl" else "ncl 0.1",
                    )

            self.assertEqual(calls, ["sbcl"])
            self.assertFalse(repro_dir.exists())

    def test_ncl_launch_error_is_fatal_and_does_not_create_a_differential_repro(self):
        with tempfile.TemporaryDirectory() as directory:
            repro_dir = Path(directory) / "repros"

            def fake_execute(engine, binary, policy, form, timeout):
                if engine == "sbcl":
                    return run.Execution("passed", "42", 0, "")
                return run.Execution("launch_error", "", None, "not executable")

            with patch.object(run, "_resolve_binary", side_effect=lambda value: value):
                with self.assertRaisesRegex(RuntimeError, "NCL runner failed"):
                    run.run_suite(
                        seed_count=1,
                        seed_start=0,
                        ncl="ncl",
                        sbcl="sbcl",
                        repro_dir=repro_dir,
                        timeout=1,
                        execute=fake_execute,
                        version=lambda binary, engine: "SBCL 2.6.10" if engine == "sbcl" else "ncl 0.1",
                    )

            self.assertFalse(repro_dir.exists())

    def test_runner_returns_nonzero_for_oracle_errors(self):
        with patch.object(run, "run_suite", side_effect=RuntimeError("SBCL oracle failed")):
            with patch.object(sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit) as raised:
                    run.main([
                        "--seed-count", "1",
                        "--ncl", "ncl",
                        "--sbcl", "sbcl",
                        "--repro-dir", "repros",
                    ])

        self.assertEqual(raised.exception.code, 1)

    def test_mismatch_fails_and_writes_a_minimized_reproduction(self):
        with tempfile.TemporaryDirectory() as directory:
            repro_dir = Path(directory) / "repros"

            def fake_execute(engine, binary, policy, form, timeout):
                expression = render_form(form)
                if engine == "sbcl":
                    return run.Execution("passed", f"oracle:{expression}", 0, "")
                if policy == "off" and "aref" in expression:
                    return run.Execution("passed", f"ncl:{expression}", 0, "")
                return run.Execution("passed", f"oracle:{expression}", 0, "")

            with patch.object(run, "_resolve_binary", side_effect=lambda value: value):
                report = run.run_suite(
                    seed_count=1,
                    seed_start=6,
                    ncl="ncl",
                    sbcl="sbcl",
                    repro_dir=repro_dir,
                    timeout=1,
                    execute=fake_execute,
                    version=lambda binary, engine: "SBCL 2.6.10" if engine == "sbcl" else "ncl 0.1",
                )

            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["programs"], 1)
            self.assertEqual(report["comparisons"], 3)
            self.assertEqual(len(report["mismatches"]), 1)
            mismatch = report["mismatches"][0]
            source_path = Path(mismatch["source_path"])
            metadata_path = Path(mismatch["metadata_path"])
            self.assertTrue(source_path.is_file())
            self.assertTrue(metadata_path.is_file())
            source = source_path.read_text(encoding="utf-8")
            self.assertIn('(format t "~S~%" ', source)
            metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
            self.assertEqual(metadata["seed"], 6)
            self.assertEqual(metadata["policy"], "off")
            self.assertNotEqual(metadata["expression"], render_form(generate_expression(6)))
            self.assertIn("aref", metadata["expression"])
            self.assertTrue(metadata["ncl"]["output"].startswith("ncl:"))
            self.assertTrue(metadata["oracle"]["output"].startswith("oracle:"))

    def test_crashes_are_differential_failures(self):
        self.assertTrue(run._is_mismatch(
            run.Execution("passed", "1", 0, ""),
            run.Execution("crash", "", -6, "signal"),
        ))
        with self.assertRaisesRegex(RuntimeError, "SBCL oracle failed"):
            run._is_mismatch(
                run.Execution("crash", "", -6, "signal"),
                run.Execution("crash", "", -6, "signal"),
            )


if __name__ == "__main__":
    unittest.main()
