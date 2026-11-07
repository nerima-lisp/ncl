#!/usr/bin/env python3
"""Measure NCL's per-LOAD RSS and VM mappings for generated function definitions."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import selectors
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[3]


def generate_source(path: Path, functions: int, assertion_count: int) -> None:
    digits = max(6, len(str(functions - 1)))
    with path.open("w", encoding="ascii") as source:
        for index in range(functions):
            source.write(f"(defun CODE-SPACE-{index:0{digits}d} () 0)\n")
        if assertion_count:
            source.write(f"(assert (= (CODE-SPACE-{functions - 1:0{digits}d}) 0))\n")


def linux_metrics(pid: int) -> tuple[int | None, int | None, int | None]:
    status = Path(f"/proc/{pid}/status").read_text(encoding="ascii")
    values = {
        line.split(":", 1)[0]: int(line.split()[1])
        for line in status.splitlines()
        if line.startswith(("VmRSS:", "VmHWM:"))
    }
    maps = Path(f"/proc/{pid}/maps").read_text(encoding="ascii").splitlines()
    return values.get("VmRSS"), values.get("VmHWM"), len(maps)


def mac_rss_kib(pid: int) -> int | None:
    result = subprocess.run(
        ["ps", "-o", "rss=", "-p", str(pid)],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None
    return int(result.stdout.strip())


def mac_vm_maps(pid: int) -> int | None:
    try:
        result = subprocess.run(
            ["vmmap", "-summary", str(pid)],
            check=False,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except subprocess.TimeoutExpired:
        return None
    if result.returncode != 0:
        return None
    for line in result.stdout.splitlines():
        fields = line.split()
        if fields and fields[0] == "TOTAL":
            return int(fields[-1])
    return None


def process_metrics(pid: int) -> tuple[int | None, int | None, int | None]:
    if sys.platform.startswith("linux"):
        return linux_metrics(pid)
    if sys.platform == "darwin":
        return mac_rss_kib(pid), None, None
    raise RuntimeError(f"unsupported measurement platform: {sys.platform}")


def run_once(
    binary: Path,
    source: Path,
    run_number: int,
    timeout: float,
    assertion_count: int,
    max_rss_increase_kib: int | None,
) -> None:
    process = subprocess.Popen(
        [str(binary)],
        cwd=ROOT,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=0,
    )
    assert process.stdin is not None
    assert process.stdout is not None
    assert process.stderr is not None

    selector = selectors.DefaultSelector()
    os.set_blocking(process.stdout.fileno(), False)
    os.set_blocking(process.stderr.fileno(), False)
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(process.stderr, selectors.EVENT_READ, "stderr")
    output = bytearray()
    errors = bytearray()
    prompt_count = 0
    load_rss_samples: list[int] = []
    map_samples: list[int] = []
    linux_hwm: int | None = None
    last_rss: int | None = None
    last_hwm: int | None = None
    last_maps: int | None = None
    baseline_rss: int | None = None
    baseline_maps: int | None = None
    phase = "startup"
    deadline = time.monotonic() + timeout

    def sample(*, enforce_limit: bool = True) -> None:
        nonlocal linux_hwm, last_rss, last_hwm, last_maps
        if process.poll() is not None:
            return
        rss, hwm, maps = process_metrics(process.pid)
        if rss is not None:
            last_rss = rss
            load_rss_samples.append(rss)
        if hwm is not None:
            linux_hwm = hwm
            last_hwm = hwm
        if maps is not None:
            last_maps = maps
            map_samples.append(maps)
        if (
            max_rss_increase_kib is not None
            and baseline_rss is not None
            and hwm is not None
            and enforce_limit
            and hwm - baseline_rss >= max_rss_increase_kib
        ):
            raise RuntimeError(
                f"RSS increase reached {hwm - baseline_rss} KiB, "
                f"limit is {max_rss_increase_kib} KiB"
            )

    def wait_for_prompt(target: int, *, measure: bool) -> None:
        nonlocal prompt_count
        while prompt_count < target:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"NCL did not reach REPL prompt {target}")
            for key, _ in selector.select(min(0.05, remaining)):
                try:
                    chunk = os.read(key.fileobj.fileno(), 4096)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                if key.data == "stdout":
                    output.extend(chunk)
                else:
                    errors.extend(chunk)
                    prompt_count = errors.count(b"> ")
            if measure:
                sample()
            if process.poll() is not None and prompt_count < target:
                raise RuntimeError(
                    f"NCL exited before REPL prompt {target} with status {process.returncode}: "
                    f"{errors.decode(errors='replace')}"
                )

    def record(status: str, *, reason: str | None = None) -> dict[str, object]:
        peak_rss = linux_hwm if sys.platform.startswith("linux") else max(load_rss_samples, default=None)
        peak_maps = max(map_samples, default=last_maps)
        return {
            "run": run_number,
            "status": status,
            "phase": phase,
            "reason": reason,
            "rss_baseline_kib": baseline_rss,
            "rss_last_kib": last_rss,
            "rss_hwm_kib": last_hwm,
            "rss_peak_kib": peak_rss,
            "rss_increase_kib": (
                peak_rss - baseline_rss
                if peak_rss is not None and baseline_rss is not None
                else None
            ),
            "rss_peak_method": (
                "VmHWM after LOAD"
                if sys.platform.startswith("linux")
                else "sampled ps RSS while loading"
            ),
            "vm_maps_baseline": baseline_maps,
            "vm_maps_last": last_maps,
            "vm_maps_peak": peak_maps,
            "vm_maps_source": (
                "/proc/<pid>/maps"
                if sys.platform.startswith("linux")
                else "vmmap -summary TOTAL"
                if sys.platform == "darwin"
                else None
            ),
            "child_exit_status": process.poll(),
            "stderr_tail": errors.decode(errors="replace")[-1000:],
        }

    try:
        wait_for_prompt(1, measure=False)
        baseline_maps = mac_vm_maps(process.pid) if sys.platform == "darwin" else None
        baseline_rss, _, linux_baseline_maps = process_metrics(process.pid)
        if linux_baseline_maps is not None:
            baseline_maps = linux_baseline_maps
        last_rss = baseline_rss
        last_maps = baseline_maps
        map_samples.append(baseline_maps) if baseline_maps is not None else None
        lisp_path = json.dumps(str(source), ensure_ascii=True)
        phase = "loading"
        process.stdin.write(f"(load {lisp_path})\n".encode("ascii"))
        process.stdin.flush()
        wait_for_prompt(2, measure=True)
        sample()
        post_load_maps = mac_vm_maps(process.pid) if sys.platform == "darwin" else None
        if post_load_maps is not None:
            last_maps = post_load_maps
            map_samples.append(post_load_maps)
        phase = "post-LOAD"
        print(json.dumps(record("pending")), flush=True)
        phase = "shutdown"
        process.stdin.close()
        process.wait(timeout=max(1.0, deadline - time.monotonic()))
        for stream, destination in ((process.stdout, output), (process.stderr, errors)):
            os.set_blocking(stream.fileno(), True)
            while chunk := os.read(stream.fileno(), 4096):
                destination.extend(chunk)
        if process.returncode != 0:
            raise RuntimeError(
                f"NCL exited with status {process.returncode}: {errors.decode(errors='replace')}"
            )
        load_value = output.decode(errors="replace").splitlines()
        load_value = next((line.strip() for line in reversed(load_value) if line.strip()), "")
        if load_value != "T":
            raise AssertionError(f"LOAD returned {load_value!r}, expected 'T'")
        print(json.dumps(record("complete") | {"load_result": load_value, "assertions": assertion_count}), flush=True)
    except Exception as error:
        try:
            sample(enforce_limit=False)
        except Exception:
            pass
        print(json.dumps(record("incomplete", reason=str(error))), flush=True)
        raise
    finally:
        selector.close()
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/ncl"))
    parser.add_argument("--functions", type=int, default=100_000)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--timeout-seconds", type=float, default=1200)
    parser.add_argument("--definitions-only", action="store_true")
    parser.add_argument("--max-rss-increase-kib", type=int)
    arguments = parser.parse_args()
    if (
        arguments.functions < 1
        or arguments.runs < 1
        or arguments.timeout_seconds <= 0
        or (arguments.max_rss_increase_kib is not None and arguments.max_rss_increase_kib < 1)
    ):
        parser.error("functions, runs, and timeout must be positive")

    binary = arguments.binary if arguments.binary.is_absolute() else ROOT / arguments.binary
    binary = binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error(f"NCL executable is missing or not executable: {binary}")
    if not (sys.platform.startswith("linux") or sys.platform == "darwin"):
        parser.error(f"unsupported measurement platform: {sys.platform}")

    output_dir = ROOT / "target" / "code-space-measure"
    output_dir.mkdir(parents=True, exist_ok=True)
    source = output_dir / f"functions-{arguments.functions}.lisp"
    assertion_count = 0 if arguments.definitions_only else 1
    generate_source(source, arguments.functions, assertion_count)
    expected_defuns = arguments.functions
    observed_defuns = sum(
        line.startswith("(defun CODE-SPACE-") for line in source.read_text(encoding="ascii").splitlines()
    )
    if observed_defuns != expected_defuns:
        raise AssertionError(f"generated {observed_defuns} defuns, expected {expected_defuns}")

    print(
        json.dumps(
            {
                "binary": str(binary),
                "source": str(source),
                "generated_defuns": observed_defuns,
                "generated_assertions": assertion_count,
                "runs": arguments.runs,
                "platform": sys.platform,
            }
        ),
        flush=True,
    )
    for run_number in range(1, arguments.runs + 1):
        run_once(
            binary,
            source,
            run_number,
            arguments.timeout_seconds,
            assertion_count,
            arguments.max_rss_increase_kib,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
