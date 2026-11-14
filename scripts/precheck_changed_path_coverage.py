#!/usr/bin/env python3
"""Run the CI changed-path coverage gate using package-scoped local coverage."""
from __future__ import annotations

import argparse
import copy
import json
import os
import subprocess
import sys
import tempfile
import time
import tomllib
from pathlib import Path

if __package__:
    from . import check_changed_path_coverage as coverage_gate
else:
    import check_changed_path_coverage as coverage_gate


SCRIPT_DIR = Path(__file__).resolve().parent
REPOSITORY = SCRIPT_DIR.parent
REPOSITORY_SLUG = "nerima-lisp/ncl"


class PrecheckError(ValueError):
    pass


def changed_crates(changed_paths: list[str], root: Path) -> list[str]:
    """Return package names for changed paths under the nearest package manifest."""
    repository = root.resolve()
    package_names: set[str] = set()
    manifest_cache: dict[Path, str | None] = {}

    for raw_path in changed_paths:
        relative = Path(raw_path)
        if (
            relative.suffix != ".rs"
            or relative.is_absolute()
            or ".." in relative.parts
            or not relative.parts
        ):
            continue
        current = repository / relative.parent

        while current == repository or current.is_relative_to(repository):
            manifest = current / "Cargo.toml"
            if manifest in manifest_cache:
                package = manifest_cache[manifest]
            elif manifest.is_file():
                try:
                    document = tomllib.loads(manifest.read_text(encoding="utf-8"))
                except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
                    raise PrecheckError(f"cannot read package manifest {manifest}: {error}") from error
                metadata = document.get("package")
                package = metadata.get("name") if isinstance(metadata, dict) else None
                if metadata is not None and (not isinstance(metadata, dict) or not isinstance(package, str)):
                    raise PrecheckError(f"package manifest {manifest} has no package name")
                manifest_cache[manifest] = package
            else:
                package = None
                manifest_cache[manifest] = None

            if package is not None:
                package_names.add(package)
                break
            if current == repository:
                break
            current = current.parent

    return sorted(package_names)


def _run_text(command: list[str], root: Path, label: str) -> str:
    try:
        result = subprocess.run(command, cwd=root, capture_output=True, text=True, check=False)
    except OSError as error:
        raise PrecheckError(f"{label} could not start: {error}") from error
    if result.returncode != 0:
        details = result.stderr.strip() or result.stdout.strip()
        suffix = f": {details}" if details else ""
        raise PrecheckError(f"{label} failed with exit status {result.returncode}{suffix}")
    return result.stdout.strip()


def _run_status(command: list[str], root: Path, label: str, env: dict[str, str] | None = None) -> int:
    try:
        return subprocess.run(command, cwd=root, env=env, check=False).returncode
    except OSError as error:
        print(f"coverage precheck error: {label} could not start: {error}", file=sys.stderr)
        return 2


def _empty_summary(path: Path) -> None:
    regions = {"count": 0, "covered": 0, "notcovered": 0, "percent": 100.0}
    payload = {"data": [{"files": [], "totals": {"regions": regions}}]}
    path.write_text(json.dumps(payload), encoding="utf-8")


def _candidate_revision(root: Path, revision: str) -> str:
    working_copy = revision == "@"
    return _run_text(
        [
            "jj",
            *([] if working_copy else ["--ignore-working-copy"]),
            "log",
            "--no-graph",
            "--no-pager",
            "-r",
            revision,
            "-T",
            "commit_id",
        ],
        root,
        f"reading candidate revision {revision}",
    )


def _base_revision(root: Path, base_branch: str) -> str:
    return _run_text(
        [
            "jj",
            "log",
            "--ignore-working-copy",
            "--no-graph",
            "--no-pager",
            "-r",
            f"{base_branch}@origin",
            "-T",
            "commit_id",
        ],
        root,
        f"reading {base_branch}@origin revision",
    )


def _merge_base_revision(root: Path, candidate: str, base: str) -> str:
    revisions = _run_text(
        [
            "jj",
            "log",
            "--ignore-working-copy",
            "--no-graph",
            "--no-pager",
            "-r",
            f"heads(::{candidate} & ::{base})",
            "-T",
            'commit_id ++ "\\n"',
        ],
        root,
        "finding the candidate/base merge base",
    ).splitlines()
    if len(revisions) != 1:
        raise PrecheckError(
            f"expected one best common ancestor for candidate and base, found {len(revisions)}"
        )
    return revisions[0]


def _download_parent_summary(
    root: Path,
    git_root: str,
    base_branch: str,
    merge_base: str,
    temporary: Path,
) -> Path:
    artifacts = temporary / "coverage-artifacts.json"
    artifact_payload = _run_text(
        [
            "gh",
            "api",
            "--paginate",
            "--slurp",
            f"repos/{REPOSITORY_SLUG}/actions/artifacts?name=coverage-summary&per_page=100",
        ],
        root,
        "listing coverage-summary artifacts with gh",
    )
    artifacts.write_text(artifact_payload, encoding="utf-8")

    selected = temporary / "coverage-parent-output"
    finder = SCRIPT_DIR / "find_parent_coverage.py"
    status = _run_status(
        [
            sys.executable,
            str(finder),
            "--merge-base",
            merge_base,
            "--base-branch",
            base_branch,
            "--artifacts",
            str(artifacts),
            "--github-output",
            str(selected),
        ],
        root,
        "selecting the parent coverage artifact",
        {**os.environ, "GIT_DIR": git_root},
    )
    if status != 0:
        raise PrecheckError(f"no usable parent coverage artifact was selected (exit status {status})")

    values = {}
    try:
        for line in selected.read_text(encoding="utf-8").splitlines():
            key, value = line.split("=", 1)
            values[key] = value
    except (OSError, UnicodeError, ValueError) as error:
        raise PrecheckError(f"cannot read selected parent artifact metadata: {error}") from error

    run_id = values.get("run_id")
    if not run_id:
        raise PrecheckError("parent artifact selector did not return a run id")
    destination = temporary / "parent"
    destination.mkdir()
    status = _run_status(
        [
            "gh",
            "run",
            "download",
            run_id,
            "--repo",
            REPOSITORY_SLUG,
            "--name",
            "coverage-summary",
            "--dir",
            str(destination),
        ],
        root,
        f"downloading coverage-summary from run {run_id}",
    )
    if status != 0:
        raise PrecheckError(f"could not download coverage-summary from run {run_id} (exit status {status})")
    summary = destination / "coverage-summary.json"
    if not summary.is_file():
        raise PrecheckError(f"run {run_id} artifact has no coverage-summary.json")
    return summary


def normalize_parent_summary(
    payload: object,
    changed_parent_paths: set[str],
    root: Path,
) -> object:
    normalized = copy.deepcopy(payload)
    repository = root.resolve()
    if isinstance(normalized, dict) and isinstance(normalized.get("data"), list):
        for dataset in normalized["data"]:
            if not isinstance(dataset, dict) or not isinstance(dataset.get("files"), list):
                continue
            for entry in dataset["files"]:
                if not isinstance(entry, dict) or not isinstance(entry.get("filename"), str):
                    continue
                filename = entry["filename"]
                path = Path(filename)
                if not path.is_absolute() or path.is_relative_to(repository):
                    continue
                matches = [
                    parent_path
                    for parent_path in changed_parent_paths
                    if filename.endswith("/" + parent_path)
                ]
                if matches:
                    entry["filename"] = max(matches, key=len)
    return normalized


def _normalize_parent_summary(
    summary: Path,
    changed: list,
    root: Path,
    destination: Path,
) -> None:
    try:
        payload = json.loads(summary.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise PrecheckError(f"cannot read parent coverage summary {summary}: {error}") from error

    parent_paths = {change.parent_path for change in changed if change.parent_path is not None}
    destination.write_text(
        json.dumps(normalize_parent_summary(payload, parent_paths, root)),
        encoding="utf-8",
    )


def _write_changed_paths(
    root: Path,
    git_root: str,
    merge_base: str,
    candidate: str,
    destination: Path,
) -> None:
    try:
        result = subprocess.run(
            [
                "git",
                f"--git-dir={git_root}",
                "diff",
                "--no-ext-diff",
                "--name-status",
                "--find-renames",
                "-z",
                merge_base,
                candidate,
            ],
            cwd=root,
            capture_output=True,
            check=False,
        )
    except OSError as error:
        raise PrecheckError(f"could not compute candidate changed paths: {error}") from error
    if result.returncode != 0:
        details = result.stderr.decode(errors="replace").strip()
        suffix = f": {details}" if details else ""
        raise PrecheckError(f"computing candidate changed paths failed with exit status {result.returncode}{suffix}")
    destination.write_bytes(result.stdout)


def _measure_candidate(root: Path, crates: list[str], destination: Path) -> int:
    if not crates:
        print("No changed path belongs to a Cargo package; skipping local package coverage.")
        _empty_summary(destination)
        return 0

    package_args = [argument for crate in crates for argument in ("--package", crate)]
    clean = ["cargo", "llvm-cov", "clean", "--locked", "--workspace"]
    started = time.monotonic()
    status = _run_status(clean, root, "cleaning stale LLVM coverage data")
    if status != 0:
        print(
            f"Package coverage preparation for {len(crates)} crate(s) "
            f"({', '.join(crates)}) took {time.monotonic() - started:.1f}s; exit status {status}."
        )
        return status

    env = os.environ.copy()
    env["CARGO_BUILD_JOBS"] = "6"
    env["RUST_TEST_THREADS"] = "6"
    command = [
        "cargo",
        "llvm-cov",
        "--locked",
        *package_args,
        "--all-features",
        "--all-targets",
        "--jobs",
        "6",
        "--json",
        "--summary-only",
        "--output-path",
        str(destination),
    ]
    status = _run_status(command, root, "measuring changed Cargo packages", env)
    print(
        f"Package coverage preparation and measurement for {len(crates)} crate(s) "
        f"({', '.join(crates)}) took {time.monotonic() - started:.1f}s; exit status {status}."
    )
    return status


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        epilog=(
            "Coverage is measured only for packages containing changed .rs paths; all changed paths still reach "
            "the gate checker. Local measurement uses cargo llvm-cov's default Cargo test runner. A precheck FAIL "
            "can become a CI PASS when workspace tests from other packages cover more regions. "
            "A precheck PASS predicts the CI coverage gate only with the same candidate, parent artifact, "
            "toolchain, target configuration, features, instrumented region sets, and deterministic package "
            "test selection; differences outside those conditions are not covered by this prediction. "
            "Python 3.11 or newer is required."
        ),
    )
    parser.add_argument("--base-branch", default="main", help="origin base branch (default: main)")
    parser.add_argument(
        "--candidate-rev",
        default="@",
        help="candidate jj revision to check (default: working copy @)",
    )
    parser.add_argument(
        "--source-root",
        type=Path,
        default=REPOSITORY,
        help="candidate source tree and Cargo workspace (default: this repository)",
    )
    args = parser.parse_args(argv)

    try:
        source_root = args.source_root.resolve()
        if not source_root.is_dir():
            raise PrecheckError(f"candidate source root is not a directory: {source_root}")
        if not (source_root / "Cargo.toml").is_file():
            raise PrecheckError(f"candidate source root has no Cargo.toml: {source_root}")
        git_root = _run_text(["jj", "git", "root"], REPOSITORY, "locating the Git object database")
        base = _base_revision(REPOSITORY, args.base_branch)
        candidate = _candidate_revision(REPOSITORY, args.candidate_rev)
        merge_base = _merge_base_revision(REPOSITORY, candidate, base)
        with tempfile.TemporaryDirectory(prefix="ncl-coverage-precheck-") as temporary_name:
            temporary = Path(temporary_name)
            changed_file = temporary / "changed-paths"
            _write_changed_paths(REPOSITORY, git_root, merge_base, candidate, changed_file)
            try:
                changes = coverage_gate.changed_paths(changed_file, source_root)
            except coverage_gate.CoverageInputError as error:
                raise PrecheckError(f"cannot parse candidate changed paths: {error}") from error
            downloaded_parent = _download_parent_summary(
                REPOSITORY,
                git_root,
                args.base_branch,
                merge_base,
                temporary,
            )
            parent = temporary / "normalized-parent-summary.json"
            _normalize_parent_summary(downloaded_parent, changes, source_root, parent)
            paths = [
                path
                for change in changes
                for path in (change.parent_path, change.candidate_path)
                if path is not None
            ]
            crates = changed_crates(paths, source_root)
            print(
                f"Candidate {candidate} is based on {args.base_branch}@origin {base} "
                f"(merge base {merge_base}); source root {source_root}."
            )
            candidate_summary = temporary / "coverage-summary.json"
            status = _measure_candidate(source_root, crates, candidate_summary)
            if status != 0:
                return status
            return _run_status(
                [
                    sys.executable,
                    str(SCRIPT_DIR / "check_changed_path_coverage.py"),
                    "--candidate",
                    str(candidate_summary),
                    "--parent",
                    str(parent),
                    "--changed-paths",
                    str(changed_file),
                    "--root",
                    str(source_root),
                ],
                source_root,
                "running the CI changed-path coverage checker",
            )
    except PrecheckError as error:
        print(f"coverage precheck error: {error}", file=sys.stderr)
        return 2
    except OSError as error:
        print(f"coverage precheck error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
