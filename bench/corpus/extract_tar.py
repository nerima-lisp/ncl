#!/usr/bin/env python3
"""Verify and safely extract one pinned tar archive."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import sys
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener

from .fetch import CorpusError, MAX_ARCHIVE_BYTES, _extract_tar


class _SourceForgeRedirectHandler(HTTPRedirectHandler):
    def redirect_request(self, request, file, code, message, headers, new_url):
        _validate_source_url(new_url)
        return super().redirect_request(request, file, code, message, headers, new_url)


def _validate_source_url(url: str) -> None:
    parsed = urlsplit(url)
    host = parsed.hostname or ""
    sourceforge_host = host == "downloads.sourceforge.net" or host.endswith(".dl.sourceforge.net")
    try:
        port = parsed.port
    except ValueError as error:
        raise CorpusError("SBCL archive URL has an invalid port") from error
    if (
        parsed.scheme != "https"
        or not sourceforge_host
        or parsed.username is not None
        or parsed.password is not None
        or port not in {None, 443}
    ):
        raise CorpusError("SBCL archive URL is not an allowed HTTPS source")


def _download(url: str, expected_sha256: str) -> bytes:
    _validate_source_url(url)
    request = Request(url, headers={"User-Agent": "ncl-benchmark/1"})
    digest = hashlib.sha256()
    chunks: list[bytes] = []
    byte_count = 0
    try:
        with build_opener(_SourceForgeRedirectHandler).open(request, timeout=60) as response:
            _validate_source_url(response.geturl())
            while True:
                chunk = response.read(64 * 1024)
                if not chunk:
                    break
                byte_count += len(chunk)
                if byte_count > MAX_ARCHIVE_BYTES:
                    raise CorpusError("SBCL archive exceeds the compressed-size limit")
                digest.update(chunk)
                chunks.append(chunk)
    except OSError as error:
        raise CorpusError("SBCL archive download failed") from error
    if digest.hexdigest() != expected_sha256:
        raise CorpusError("SBCL archive checksum mismatch")
    return b"".join(chunks)


def _positive_int(value: str) -> int:
    try:
        result = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    if result < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--member-list-sha256", required=True)
    parser.add_argument("--top-level", required=True)
    parser.add_argument("--max-unpacked-bytes", type=_positive_int, required=True)
    parser.add_argument("--max-file-bytes", type=_positive_int, required=True)
    args = parser.parse_args()
    try:
        payload = _download(args.url, args.sha256)
        _extract_tar(
            payload,
            args.destination,
            top_level=args.top_level,
            max_unpacked_bytes=args.max_unpacked_bytes,
            expected_member_list_sha256=args.member_list_sha256,
            max_file_bytes=args.max_file_bytes,
        )
    except (CorpusError, OSError, ValueError) as error:
        print(f"archive extraction failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
