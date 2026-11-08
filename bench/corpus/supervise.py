#!/usr/bin/env python3
"""Run one Lisp process and return bounded output and its peak RSS."""

from __future__ import annotations

import json
import os
import resource
import selectors
import signal
import subprocess
import sys
import time
from typing import Any


MAX_OUTPUT_BYTES = 2 * 1024 * 1024
READ_CHUNK_BYTES = 64 * 1024


def _peak_rss_bytes() -> int | None:
    try:
        value = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    except (AttributeError, OSError, ValueError):
        return None
    if value < 0:
        return None
    return int(value if sys.platform == "darwin" else value * 1024)


def _kill_process_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def run(command: list[str], timeout: float) -> dict[str, Any]:
    started = time.perf_counter()
    deadline = started + timeout
    status = "failed"
    returncode: int | None = None
    timed_out = False
    output = {"stdout": bytearray(), "stderr": bytearray()}
    output_sizes = {"stdout": 0, "stderr": 0}
    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
    except OSError as error:
        status = "spawn-error"
        output["stderr"].extend(
            f"{type(error).__name__}: process could not be started".encode()
        )
    else:
        assert process.stdout is not None
        assert process.stderr is not None
        streams = {process.stdout: "stdout", process.stderr: "stderr"}
        with selectors.DefaultSelector() as selector:
            for stream in streams:
                os.set_blocking(stream.fileno(), False)
                selector.register(stream, selectors.EVENT_READ)
            while selector.get_map() or process.poll() is None:
                remaining = deadline - time.perf_counter()
                if remaining <= 0:
                    timed_out = True
                    status = "timeout"
                    _kill_process_group(process)
                    deadline = float("inf")
                    remaining = float("inf")
                if selector.get_map():
                    events = selector.select(min(max(remaining, 0), 0.1))
                else:
                    time.sleep(min(max(remaining, 0), 0.01))
                    events = []
                for key, _ in events:
                    stream = key.fileobj
                    chunk = os.read(stream.fileno(), READ_CHUNK_BYTES)
                    if not chunk:
                        selector.unregister(stream)
                        stream.close()
                        continue
                    name = streams[stream]
                    output_sizes[name] += len(chunk)
                    room = MAX_OUTPUT_BYTES - len(output[name])
                    if room > 0:
                        output[name].extend(chunk[:room])
                if process.poll() is not None and not selector.get_map():
                    break
            returncode = process.wait()
            if not timed_out:
                status = "passed" if returncode == 0 else "failed"
    elapsed = max(0.0, time.perf_counter() - started)
    stdout = bytes(output["stdout"]).decode("utf-8", errors="replace")
    stderr = bytes(output["stderr"]).decode("utf-8", errors="replace")
    return {
        "status": status,
        "return_code": returncode,
        "timed_out": timed_out,
        "wall_seconds": elapsed,
        "peak_rss_bytes": _peak_rss_bytes() if returncode is not None or timed_out else None,
        "stdout": stdout,
        "stderr": stderr,
        "output_truncated": any(size > MAX_OUTPUT_BYTES for size in output_sizes.values()),
    }


def main(argv: list[str] | None = None) -> int:
    arguments = list(sys.argv[1:] if argv is None else argv)
    if "--" not in arguments:
        print("supervisor requires `-- <command>`", file=sys.stderr)
        return 2
    separator = arguments.index("--")
    if separator != 2 or len(arguments) <= separator + 1 or arguments[0] != "--timeout":
        print("usage: supervise.py --timeout SECONDS -- COMMAND [ARG ...]", file=sys.stderr)
        return 2
    try:
        timeout = float(arguments[1])
    except ValueError:
        print("timeout must be a finite positive number", file=sys.stderr)
        return 2
    if not 0 < timeout < float("inf"):
        print("timeout must be a finite positive number", file=sys.stderr)
        return 2
    result = run(arguments[separator + 1 :], timeout)
    json.dump(result, sys.stdout, sort_keys=True, allow_nan=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
