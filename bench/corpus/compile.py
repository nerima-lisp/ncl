"""Compile pinned Lisp sources and summarize compiler performance."""

from __future__ import annotations

import json
import math
import os
from pathlib import Path
import re
import shutil
import socket
import statistics
import subprocess
import sys
import tempfile
from typing import Any, Callable, Sequence

from ..statistics import compare_statistics, sample_statistics
from ..timing import MINIMUM_SAMPLE_SECONDS, measurement_metadata
from .fetch import CorpusError, SourceFile, fetch_corpus
from .report import aggregate_file_samples as _aggregate_file_samples
from .report import pass_profile as _parse_pass_profile
from .report import runtime_summary as _summarize_runtime


ROOT = Path(__file__).resolve().parents[2]
SUPERVISOR = Path(__file__).with_name("supervise.py")
SBCL_VERSION = "2.6.10"
DEFAULT_SAMPLE_RUNS = 5
MAX_SOURCE_BYTES = 32 * 1024 * 1024
MAX_DIAGNOSTIC_CHARS = 4_000
_TIME_MARKER = re.compile(
    r"^__NCL_BENCH_RESULT__\|2\|([0-9]+)\|([0-9]+)\|([0-9]+)$",
    re.MULTILINE,
)


class BenchmarkError(RuntimeError):
    """The benchmark harness could not produce a trustworthy report."""


def _safe_diagnostic(value: str, paths: Sequence[Path] = ()) -> str:
    text = value
    for path in sorted((str(path) for path in paths), key=len, reverse=True):
        text = text.replace(path, "<corpus>")
    text = text.replace(str(Path.home()), "<home>")
    if socket.gethostname():
        text = text.replace(socket.gethostname(), "<host>")
    text = re.sub(r"(?<![A-Za-z0-9:_])/(?:[^\s\"'<>|]+)", "<path>", text)
    text = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", text)
    text = "".join(character for character in text if character in "\n\t" or ord(character) >= 32)
    text = text[-MAX_DIAGNOSTIC_CHARS:]
    return text or "process produced no diagnostic output"


def _resolve_executable(value: str | Path) -> str:
    text = os.fspath(value)
    if os.sep not in text and (os.altsep is None or os.altsep not in text):
        resolved = shutil.which(text)
        if resolved is None:
            raise BenchmarkError(f"executable was not found: {text}")
        return resolved
    path = Path(text).resolve(strict=True)
    if not path.is_file() or not os.access(path, os.X_OK):
        raise BenchmarkError("benchmark executable is not a runnable file")
    return str(path)


def _supervised(command: Sequence[str], timeout: float) -> dict[str, Any]:
    invocation = [
        sys.executable,
        "-B",
        str(SUPERVISOR),
        "--timeout",
        str(timeout),
        "--",
        *command,
    ]
    try:
        completed = subprocess.run(
            invocation,
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=timeout + 15,
            check=False,
            env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise BenchmarkError("process supervisor did not complete") from error
    if completed.returncode != 0:
        raise BenchmarkError("process supervisor returned an invalid result")
    try:
        result = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise BenchmarkError("process supervisor returned malformed JSON") from error
    required = {
        "status", "return_code", "timed_out", "wall_seconds", "peak_rss_bytes",
        "stdout", "stderr", "output_truncated",
    }
    if not isinstance(result, dict) or set(result) != required:
        raise BenchmarkError("process supervisor returned an unexpected shape")
    if not isinstance(result["wall_seconds"], (int, float)) or not math.isfinite(result["wall_seconds"]):
        raise BenchmarkError("process supervisor returned an invalid duration")
    if not isinstance(result["stdout"], str) or not isinstance(result["stderr"], str):
        raise BenchmarkError("process supervisor returned invalid output")
    if not isinstance(result["output_truncated"], bool):
        raise BenchmarkError("process supervisor returned an invalid truncation flag")
    return result


def _startup_command(executable: str, suite: str) -> list[str]:
    if suite == "ncl":
        return [executable]
    return [executable, "--noinform", "--no-userinit", "--disable-debugger", "--non-interactive", "--eval", "(quit)"]


def _measure_startup(
    executable: str,
    suite: str,
    timeout: float,
    samples: int = 3,
    *,
    supervisor: Callable[[Sequence[str], float], dict[str, Any]] = _supervised,
) -> dict[str, Any]:
    durations: list[float] = []
    peaks: list[int] = []
    for _ in range(samples):
        record = supervisor(_startup_command(executable, suite), timeout)
        if record["status"] != "passed":
            raise BenchmarkError(f"{suite} runtime startup failed")
        durations.append(float(record["wall_seconds"]))
        peak = record["peak_rss_bytes"]
        if isinstance(peak, int) and peak >= 0:
            peaks.append(peak)
    return {
        "samples": durations,
        "median_seconds": statistics.median(durations),
        "minimum_seconds": min(durations),
        "peak_rss_bytes": max(peaks) if peaks else None,
    }


def _line_count(path: Path) -> int:
    size = path.stat().st_size
    if size > MAX_SOURCE_BYTES:
        raise BenchmarkError("corpus source exceeds the source-size limit")
    payload = path.read_bytes()
    return len(payload.splitlines())


def select_sources(sources: Sequence[SourceFile], mode: str) -> list[SourceFile]:
    ordered = sorted(sources, key=lambda item: (item.suite, item.relative_path))
    required_suites = {"ansi-test", "alexandria", "cl-ppcre", "asdf", "synthetic"}
    if {item.suite for item in ordered} != required_suites:
        raise BenchmarkError("compile corpus source groups do not match the required set")
    identities = [(item.suite, item.relative_path) for item in ordered]
    if len(set(identities)) != len(identities):
        raise BenchmarkError("compile corpus contains duplicate source paths")
    if mode == "full":
        return ordered
    if mode != "light":
        raise ValueError("mode must be light or full")
    selected: dict[str, SourceFile] = {}
    for item in ordered:
        selected.setdefault(item.suite, item)
    if set(selected) != required_suites:
        raise BenchmarkError("light corpus is missing a required source group")
    return [selected[key] for key in sorted(selected)]


def _lisp_string(value: str) -> str:
    if any(ord(character) < 32 for character in value):
        raise BenchmarkError("source path contains a control character")
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _write_compile_driver(path: Path, source: Path, repetitions: int = 1) -> None:
    if not isinstance(repetitions, int) or isinstance(repetitions, bool) or repetitions < 1:
        raise ValueError("repetitions must be a positive integer")
    source_literal = _lisp_string(str(source))
    compile_forms = "\n".join(
        f"    (compile-file #P{source_literal})" for _ in range(repetitions)
    )
    content = (
        "(let ((started (get-internal-real-time)))\n"
        "  (progn\n"
        f"{compile_forms}\n"
        "  )\n"
        "  (format t \"~&__NCL_BENCH_RESULT__|2|~D|~D|~D~%\"\n"
        f"          internal-time-units-per-second (- (get-internal-real-time) started) {repetitions}))\n"
    )
    path.write_text(content, encoding="utf-8", newline="\n")


def _compile_command(executable: str, suite: str, driver: Path, *, profile: bool) -> list[str]:
    if suite == "ncl":
        options = ["--opt-diagnostics"] if profile else []
        return [executable, *options, "--script", str(driver)]
    return [executable, "--noinform", "--no-userinit", "--disable-debugger", "--script", str(driver)]


def _timed_result(
    stdout: str,
    *,
    expected_repetitions: int | None = None,
    require_minimum_duration: bool = True,
) -> dict[str, float | int] | None:
    markers = list(_TIME_MARKER.finditer(stdout))
    if len(markers) != 1:
        return None
    units, ticks, repetitions = (int(value) for value in markers[0].groups())
    if units < 1 or ticks < 0 or repetitions < 1:
        return None
    if expected_repetitions is not None and repetitions != expected_repetitions:
        return None
    if require_minimum_duration and ticks < 1:
        return None
    try:
        elapsed_seconds = ticks / units
    except OverflowError:
        return None
    seconds = elapsed_seconds / repetitions
    if not math.isfinite(elapsed_seconds) or not math.isfinite(seconds):
        return None
    if require_minimum_duration and elapsed_seconds < MINIMUM_SAMPLE_SECONDS:
        return None
    return {
        "seconds": seconds,
        "elapsed_seconds": elapsed_seconds,
        "repetitions": repetitions,
    }


def _timed_seconds(stdout: str) -> float | None:
    result = _timed_result(stdout)
    return float(result["seconds"]) if result is not None else None


def _sample_file(
    executable: str,
    suite: str,
    source: SourceFile,
    driver: Path,
    timeout: float,
    *,
    profile: bool = False,
    supervisor: Callable[[Sequence[str], float], dict[str, Any]] = _supervised,
) -> dict[str, Any]:
    repetitions = 1
    total_wall_seconds = 0.0
    while True:
        _write_compile_driver(driver, source.path, repetitions)
        process = supervisor(_compile_command(executable, suite, driver, profile=profile), timeout)
        total_wall_seconds += float(process["wall_seconds"])
        timing = _timed_result(
            process["stdout"],
            expected_repetitions=repetitions,
            require_minimum_duration=False,
        )
        if (
            process["status"] == "passed"
            and process["output_truncated"] is False
            and timing is not None
            and timing["elapsed_seconds"] < MINIMUM_SAMPLE_SECONDS
        ):
            repetitions *= 2
            continue
        break
    seconds = float(timing["seconds"]) if timing is not None else None
    process_status = process["status"]
    successful = (
        process_status == "passed"
        and seconds is not None
        and process["output_truncated"] is False
        and timing is not None
        and timing["elapsed_seconds"] >= MINIMUM_SAMPLE_SECONDS
    )
    diagnostic_paths = (ROOT, source.path.parent, driver.parent, Path.home())
    reason = None
    if not successful:
        if process_status == "timeout":
            reason = "compile process timed out"
        elif process_status != "passed":
            reason = _safe_diagnostic(process["stderr"] or process["stdout"], diagnostic_paths)
        elif process["output_truncated"] is not False:
            reason = "compile process output was truncated"
        else:
            reason = "compiler did not emit a valid timing marker above the minimum duration"
    return {
        "suite": source.suite,
        "version": source.version,
        "file": source.relative_path,
        "line_count": _line_count(source.path),
        "status": "passed" if successful else process_status if process_status != "passed" else "failed",
        "compile_seconds": seconds,
        "compile_elapsed_seconds": timing["elapsed_seconds"] if successful else None,
        "compile_repetitions": timing["repetitions"] if successful else None,
        "lines_per_second": _line_count(source.path) / seconds if successful and seconds and seconds > 0 else None,
        "process_wall_seconds": total_wall_seconds,
        "peak_rss_bytes": process["peak_rss_bytes"],
        "reason": reason,
        "_stderr": process["stderr"],
    }


def _pass_profile(output: str, paths: Sequence[Path]) -> dict[str, Any]:
    return _parse_pass_profile(output, paths, _safe_diagnostic)


def _runtime_summary(version: str, startup: dict[str, Any], files: list[dict[str, Any]], binary_size: int | None) -> dict[str, Any]:
    try:
        return _summarize_runtime(version, startup, files, binary_size)
    except ValueError as error:
        raise BenchmarkError(str(error)) from error


def run_benchmark(
    ncl: str | Path,
    sbcl: str | Path,
    mode: str,
    *,
    ncl_profiler: str | Path | None = None,
    sample_runs: int = DEFAULT_SAMPLE_RUNS,
    cache_dir: Path | None = None,
    timeout: float = 60.0,
    fetcher: Callable[[Path, Path], list[SourceFile]] = fetch_corpus,
    supervisor: Callable[[Sequence[str], float], dict[str, Any]] = _supervised,
) -> dict[str, Any]:
    if mode not in {"light", "full"}:
        raise ValueError("mode must be light or full")
    if not isinstance(sample_runs, int) or isinstance(sample_runs, bool) or sample_runs < 1:
        raise ValueError("sample_runs must be positive")
    cache_root = cache_dir or ROOT / "target/bench-corpus-cache"
    cache_root.mkdir(mode=0o700, parents=True, exist_ok=True)
    ncl_exe = _resolve_executable(ncl)
    sbcl_exe = _resolve_executable(sbcl)
    ncl_profiler_exe = _resolve_executable(ncl_profiler) if ncl_profiler is not None else None
    ncl_version_record = supervisor([ncl_exe, "--version"], timeout)
    sbcl_version_record = supervisor([sbcl_exe, "--version"], timeout)
    for suite, record in (("ncl", ncl_version_record), ("sbcl", sbcl_version_record)):
        if (
            record.get("status") != "passed"
            or record.get("output_truncated") is not False
            or not isinstance(record.get("stdout"), str)
        ):
            raise BenchmarkError(f"{suite} version probe failed")
    ncl_version = _safe_diagnostic(ncl_version_record["stdout"].strip())
    sbcl_version = _safe_diagnostic(sbcl_version_record["stdout"].strip())
    if not ncl_version:
        raise BenchmarkError("NCL version probe returned no text")
    if not re.search(rf"\bSBCL\s+{re.escape(SBCL_VERSION)}\b", sbcl_version):
        raise BenchmarkError(f"SBCL {SBCL_VERSION} is required")
    ncl_size = Path(ncl_exe).stat().st_size if Path(ncl_exe).is_file() else None
    with tempfile.TemporaryDirectory(prefix="ncl-bench-corpus-", dir=cache_root) as temporary:
        work_dir = Path(temporary)
        sources = select_sources(fetcher(cache_root, work_dir), mode)
        measurement = measurement_metadata(SBCL_VERSION)
        driver = work_dir / "compile-driver.lisp"
        raw_results: dict[str, list[list[dict[str, Any]]]] = {
            "ncl": [[] for _ in sources],
            "sbcl": [[] for _ in sources],
        }
        executables = {"ncl": ncl_exe, "sbcl": sbcl_exe}
        run_order: list[dict[str, Any]] = []
        for repeat in range(sample_runs):
            for index, source in enumerate(sources):
                order = ("ncl", "sbcl") if (repeat + index) % 2 == 0 else ("sbcl", "ncl")
                run_order.append({
                    "phase": "compile",
                    "file": source.label,
                    "sample": repeat + 1,
                    "order": list(order),
                })
                for suite in order:
                    raw_results[suite][index].append(
                        _sample_file(executables[suite], suite, source, driver, timeout, supervisor=supervisor)
                    )
        results = {
            suite: [_aggregate_file_samples(samples) for samples in rows]
            for suite, rows in raw_results.items()
        }
        startup_samples: dict[str, list[dict[str, Any]]] = {"ncl": [], "sbcl": []}
        for repeat in range(sample_runs):
            order = ("ncl", "sbcl") if repeat % 2 == 0 else ("sbcl", "ncl")
            run_order.append({"phase": "startup", "sample": repeat + 1, "order": list(order)})
            for suite in order:
                startup_samples[suite].append(
                    _measure_startup(executables[suite], suite, timeout, samples=1, supervisor=supervisor)
                )
        startup = {
            suite: {
                "samples": [record["median_seconds"] for record in records],
                **sample_statistics([record["median_seconds"] for record in records]),
                "peak_rss_bytes": max(
                    (record["peak_rss_bytes"] for record in records if isinstance(record["peak_rss_bytes"], int)),
                    default=None,
                ),
            }
            for suite, records in startup_samples.items()
        }
        synthetic = next(source for source in sources if source.suite == "synthetic")
        if ncl_profiler_exe is None:
            pass_profile = {
                "status": "not-configured",
                "passes": [],
                "top_10": [],
                "allocation_bytes_available": False,
                "reason": "--ncl-profiler was not provided",
            }
        else:
            _write_compile_driver(driver, synthetic.path)
            profile = supervisor(_compile_command(ncl_profiler_exe, "ncl", driver, profile=True), timeout)
            profile_paths = (ROOT, synthetic.path.parent, driver.parent, Path.home())
            if profile.get("status") != "passed":
                profile_reason = _safe_diagnostic(
                    str(profile.get("stderr") or profile.get("stdout") or "profiler process failed"),
                    profile_paths,
                )
            elif profile.get("output_truncated") is not False:
                profile_reason = "profiler output was truncated or invalid"
            elif _timed_seconds(profile.get("stdout", "")) is None:
                profile_reason = "profiler did not emit its internal timing marker"
            elif not isinstance(profile.get("stderr"), str):
                profile_reason = "profiler returned invalid diagnostics"
            else:
                profile_output = "\n".join(
                    value for value in (profile.get("stdout", ""), profile["stderr"]) if isinstance(value, str)
                )
                pass_profile = _pass_profile(profile_output, profile_paths)
                profile_reason = None
            if profile_reason is not None:
                pass_profile = {
                    "status": "unavailable",
                    "passes": [],
                    "top_10": [],
                    "allocation_bytes_available": False,
                    "reason": profile_reason,
                }
        runtimes = {
            "ncl": _runtime_summary(ncl_version, startup["ncl"], results["ncl"], ncl_size),
            "sbcl": _runtime_summary(sbcl_version, startup["sbcl"], results["sbcl"], None),
        }
        case_comparisons = {}
        for ncl_file, sbcl_file in zip(runtimes["ncl"]["files"], runtimes["sbcl"]["files"]):
            label = f"{ncl_file['suite']}/{ncl_file['file']}"
            ncl_statistics = ncl_file if ncl_file["status"] == "passed" else {
                "median_seconds": None,
                "range_seconds": None,
            }
            sbcl_statistics = sbcl_file if sbcl_file["status"] == "passed" else {
                "median_seconds": None,
                "range_seconds": None,
            }
            case_comparisons[label] = compare_statistics(
                ncl_statistics, sbcl_statistics, noise_basis="ratio"
            )
        total_statistics = {
            runtime: sample_statistics(runtimes[runtime]["total_compile_seconds_samples"])
            if runtimes[runtime]["failed_file_count"] == 0
            else sample_statistics([])
            for runtime in runtimes
        }
        total_comparison = compare_statistics(
            total_statistics["ncl"], total_statistics["sbcl"], noise_basis="ratio"
        )
        complete = all(
            runtime["failed_file_count"] == 0 and runtime["peak_rss_available"]
            for runtime in runtimes.values()
        )
        if ncl_profiler_exe is not None:
            complete = complete and pass_profile["status"] == "measured" and pass_profile["allocation_bytes_available"]
        return {
            "schema_version": 1,
            "status": "complete" if complete else "partial",
            "mode": mode,
            "measurement": measurement,
            "sample_runs": sample_runs,
            "minimum_sample_seconds": MINIMUM_SAMPLE_SECONDS,
            "iteration_count_definition": "compile repetitions per source sample",
            "warmup_runs_per_source": 0,
            "startup_warmup_runs": 0,
            "process_topology": "fresh process per compile and startup sample",
            "order_scheme": "pair order alternates by sample plus sorted corpus index",
            "noise_width_definition": (
                "per-case and corpus-total range is max minus min; cross-runtime ratio noise "
                "uses the larger per-runtime relative range"
            ),
            "corpus_file_count": len(sources),
            "run_order": run_order,
            "runtimes": runtimes,
            "case_comparisons": case_comparisons,
            "total_comparison": total_comparison,
            "pass_profile": pass_profile,
            "allocation_metric": {
                "unit": "bytes",
                "basis": "requested Rust allocation bytes",
                "operations": ["alloc", "alloc_zeroed", "realloc"],
                "deallocations_subtracted": False,
                "thread_scope": "the thread running the optimization pass",
            },
        }
