"""Shared timing policy and environment metadata for benchmark runners."""

from __future__ import annotations

import os
import platform
import re
from typing import Any


MINIMUM_SAMPLE_SECONDS = 0.1


def measurement_metadata(
    sbcl_version: str,
    *,
    ncl_sha: str | None = None,
    minimum_sample_seconds: float | None = MINIMUM_SAMPLE_SECONDS,
) -> dict[str, Any]:
    revision = ncl_sha or os.environ.get("NCL_BENCH_REVISION") or os.environ.get("GITHUB_SHA")
    revision = revision.strip() if revision else ""
    if not re.fullmatch(r"[0-9a-f]{40,64}", revision):
        revision = ""
    getloadavg = getattr(os, "getloadavg", None)
    try:
        load_average = list(getloadavg()) if getloadavg is not None else None
    except OSError:
        load_average = None
    metadata = {
        "ncl_sha": revision or None,
        "sbcl_version": sbcl_version,
        "host": f"{platform.system()} {platform.machine()}",
        "load_average": load_average,
    }
    if minimum_sample_seconds is not None:
        metadata["minimum_sample_seconds"] = minimum_sample_seconds
    return metadata
