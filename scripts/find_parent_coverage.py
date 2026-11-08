#!/usr/bin/env python3
"""Select the nearest base-branch ancestor with a coverage summary artifact."""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


class ParentCoverageError(ValueError):
    pass


def _artifact_entries(payload: Any) -> list[dict[str, Any]]:
    pages = payload if isinstance(payload, list) else [payload]
    entries = []
    for page_index, page in enumerate(pages):
        if not isinstance(page, dict) or not isinstance(page.get("artifacts"), list):
            raise ParentCoverageError(f"artifact listing page {page_index} has no artifact list")
        for artifact in page["artifacts"]:
            if not isinstance(artifact, dict):
                raise ParentCoverageError(f"artifact listing page {page_index} contains an invalid entry")
            entries.append(artifact)
    return entries


def select_parent_artifact(
    payload: Any,
    base_branch: str,
    ancestor_shas: list[str],
) -> dict[str, Any] | None:
    by_sha: dict[str, list[dict[str, Any]]] = {}
    for artifact in _artifact_entries(payload):
        workflow_run = artifact.get("workflow_run")
        if (
            artifact.get("name") != "coverage-summary"
            or artifact.get("expired") is not False
            or not isinstance(workflow_run, dict)
            or workflow_run.get("head_branch") != base_branch
        ):
            continue
        sha = workflow_run.get("head_sha")
        run_id = workflow_run.get("id")
        valid_run_id = (
            isinstance(run_id, int) and not isinstance(run_id, bool) and run_id > 0
        ) or (isinstance(run_id, str) and run_id.isdecimal() and int(run_id) > 0)
        if isinstance(sha, str) and re.fullmatch(r"[0-9a-fA-F]{40}", sha) and valid_run_id:
            by_sha.setdefault(sha, []).append(artifact)

    for sha in ancestor_shas:
        matches = by_sha.get(sha, [])
        if matches:
            newest = max(matches, key=lambda item: str(item.get("created_at", "")))
            workflow_run = newest["workflow_run"]
            return {
                "run_id": str(workflow_run["id"]),
                "sha": sha,
                "fallback": sha != (ancestor_shas[0] if ancestor_shas else ""),
            }
    return None


def _ancestors(merge_base: str) -> list[str]:
    result = subprocess.run(
        ["git", "rev-list", "--first-parent", merge_base],
        check=True,
        capture_output=True,
        text=True,
    )
    ancestors = [line for line in result.stdout.splitlines() if line]
    if not ancestors or ancestors[0] != merge_base:
        raise ParentCoverageError(f"could not enumerate ancestors starting at {merge_base}")
    return ancestors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--merge-base", required=True)
    parser.add_argument("--base-branch", required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--github-output", type=Path, required=True)
    args = parser.parse_args(argv)

    try:
        payload = json.loads(args.artifacts.read_text(encoding="utf-8"))
        ancestors = _ancestors(args.merge_base)
        selected = select_parent_artifact(payload, args.base_branch, ancestors)
        if selected is None:
            raise ParentCoverageError(
                f"no non-expired coverage-summary artifact exists on {args.base_branch} "
                f"at {args.merge_base} or its first-parent ancestors"
            )
        with args.github_output.open("a", encoding="utf-8") as stream:
            for key, value in selected.items():
                stream.write(f"{key}={str(value).lower() if isinstance(value, bool) else value}\n")
    except (OSError, UnicodeError, json.JSONDecodeError, subprocess.CalledProcessError, ParentCoverageError) as error:
        print(f"coverage parent error: {error}", file=sys.stderr)
        return 2

    print(
        f"Selected coverage-summary from run {selected['run_id']} at {selected['sha']} "
        f"(fallback={str(selected['fallback']).lower()})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
