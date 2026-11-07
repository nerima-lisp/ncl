"""Fetch fixed Lisp corpus sources and safely expand their pinned archives."""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
from pathlib import PureWindowsPath
import re
import shutil
import stat
import tarfile
import tempfile
from typing import Any
from urllib.parse import urlsplit
from urllib.request import Request, urlopen

from .generate_synthetic import generate_source


ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = ROOT / "bench/corpus/manifest.json"
MAX_ARCHIVE_BYTES = 16 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 50_000
MAX_FILE_BYTES = 16 * 1024 * 1024
ALLOWED_SOURCE_HOSTS = {
    "gitlab.common-lisp.net",
    "beta.quicklisp.org",
    "asdf.common-lisp.dev",
}
_SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")


class CorpusError(RuntimeError):
    """A pinned corpus could not be fetched or verified."""


@dataclass(frozen=True)
class SourceFile:
    suite: str
    version: str
    relative_path: str
    path: Path

    @property
    def label(self) -> str:
        return f"{self.suite}/{self.relative_path}"


def _manifest() -> dict[str, Any]:
    try:
        payload = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CorpusError("corpus manifest could not be read") from error
    if not isinstance(payload, dict) or set(payload) != {"schema_version", "sources"}:
        raise CorpusError("corpus manifest has an unexpected shape")
    if payload["schema_version"] != 1 or not isinstance(payload["sources"], dict):
        raise CorpusError("corpus manifest version is unsupported")
    return payload["sources"]


def _cache_path(cache_dir: Path, suite: str, source: dict[str, Any]) -> Path:
    version = str(source["version"])
    if not _SAFE_NAME.fullmatch(suite) or not _SAFE_NAME.fullmatch(version):
        raise CorpusError("corpus source identity is invalid")
    return cache_dir / f"{suite}-{version}-{str(source['sha256'])[:12]}.archive"


def _read_verified(path: Path, *, expected_sha256: str) -> bytes:
    nofollow = getattr(os, "O_NOFOLLOW", None)
    nonblock = getattr(os, "O_NONBLOCK", 0)
    if nofollow is None:
        raise CorpusError("secure corpus cache reads require O_NOFOLLOW")
    try:
        descriptor = os.open(path, os.O_RDONLY | nofollow | nonblock)
    except OSError as error:
        raise CorpusError(f"corpus cache entry could not be opened: {path.name}") from error
    with os.fdopen(descriptor, "rb") as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise CorpusError(f"corpus cache entry is not a regular file: {path.name}")
        payload = stream.read(MAX_ARCHIVE_BYTES + 1)
    if len(payload) > MAX_ARCHIVE_BYTES:
        raise CorpusError(f"corpus archive exceeds the compressed-size limit: {path.name}")
    if hashlib.sha256(payload).hexdigest() != expected_sha256:
        raise CorpusError(f"corpus cache checksum mismatch: {path.name}")
    return payload


def _download(source: dict[str, Any], destination: Path) -> bytes:
    url = source.get("url")
    expected_sha256 = source.get("sha256")
    if not isinstance(url, str) or not isinstance(expected_sha256, str):
        raise CorpusError("corpus source is missing its URL or checksum")
    parsed = urlsplit(url)
    if (
        parsed.scheme != "https"
        or parsed.hostname not in ALLOWED_SOURCE_HOSTS
        or parsed.username
        or parsed.password
    ):
        raise CorpusError("corpus URL must be an HTTPS URL without credentials")
    source_host = parsed.hostname
    request = Request(url, headers={"User-Agent": "ncl-benchmark/1"})
    temp_path: Path | None = None
    digest = hashlib.sha256()
    byte_count = 0
    try:
        with urlopen(request, timeout=60) as response:
            final = urlsplit(response.geturl())
            if final.scheme != "https" or final.hostname != source_host:
                raise CorpusError("corpus download redirected to an unexpected host")
            with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=".download-", delete=False) as output:
                temp_path = Path(output.name)
                while True:
                    chunk = response.read(64 * 1024)
                    if not chunk:
                        break
                    byte_count += len(chunk)
                    if byte_count > MAX_ARCHIVE_BYTES:
                        raise CorpusError("corpus archive exceeds the compressed-size limit")
                    digest.update(chunk)
                    output.write(chunk)
                output.flush()
                os.fsync(output.fileno())
        if digest.hexdigest() != expected_sha256:
            raise CorpusError(f"corpus checksum mismatch for {destination.name}")
        os.replace(temp_path, destination)
        temp_path = None
        return _read_verified(destination, expected_sha256=expected_sha256)
    except CorpusError:
        raise
    except OSError as error:
        raise CorpusError(f"corpus download failed for {destination.name}") from error
    finally:
        if temp_path is not None:
            try:
                temp_path.unlink()
            except OSError:
                pass


def _archive_bytes(source: dict[str, Any], suite: str, cache_dir: Path) -> bytes:
    cache_path = _cache_path(cache_dir, suite, source)
    expected_sha256 = source["sha256"]
    if cache_path.exists() or cache_path.is_symlink():
        return _read_verified(cache_path, expected_sha256=expected_sha256)
    return _download(source, cache_path)


def _member_path(name: str, top_level: str) -> tuple[str, ...]:
    if "\\" in name or "\x00" in name or PureWindowsPath(name).is_absolute():
        raise CorpusError("archive contains an invalid path")
    raw_parts = name.split("/")
    if raw_parts and raw_parts[-1] == "":
        raw_parts.pop()
    if any(part in {"", ".", ".."} for part in raw_parts):
        raise CorpusError("archive contains a path traversal member")
    path = PurePosixPath(name)
    if path.is_absolute() or not path.parts or any(part in {".", ".."} for part in path.parts):
        raise CorpusError("archive contains a path traversal member")
    if path.parts[0] != top_level:
        raise CorpusError("archive member is outside its pinned root")
    return path.parts[1:]


def _member_list_sha256(members: list[tarfile.TarInfo]) -> str:
    encoded = json.dumps(
        sorted(member.name for member in members),
        ensure_ascii=True,
        separators=(",", ":"),
    ).encode("ascii")
    return hashlib.sha256(encoded).hexdigest()


def _extract_tar(
    payload: bytes,
    destination: Path,
    *,
    top_level: str,
    max_unpacked_bytes: int,
    expected_member_list_sha256: str,
    max_file_bytes: int = MAX_FILE_BYTES,
) -> Path:
    destination.mkdir(mode=0o700, parents=False, exist_ok=False)
    try:
        if max_unpacked_bytes < 0 or max_file_bytes < 0:
            raise CorpusError("corpus archive size limits must be nonnegative")
        if not isinstance(expected_member_list_sha256, str) or not re.fullmatch(
            r"[0-9a-f]{64}", expected_member_list_sha256
        ):
            raise CorpusError("corpus archive member-list checksum is invalid")
        try:
            archive = tarfile.open(fileobj=io.BytesIO(payload), mode="r:*")
        except tarfile.TarError as error:
            raise CorpusError("corpus archive is not a valid tar file") from error
        with archive:
            members = []
            for member in archive:
                members.append(member)
                if len(members) > MAX_ARCHIVE_MEMBERS:
                    raise CorpusError("corpus archive has too many entries")
            seen: set[tuple[str, ...]] = set()
            total_declared = 0
            root_seen = False
            checked: list[tuple[tarfile.TarInfo, tuple[str, ...]]] = []
            path_kinds: dict[tuple[str, ...], str] = {}
            paths_with_descendants: set[tuple[str, ...]] = set()
            for member in members:
                relative = _member_path(member.name, top_level)
                if not relative:
                    if not member.isdir() or root_seen:
                        raise CorpusError("archive root member is invalid or duplicated")
                    root_seen = True
                    continue
                if relative in seen:
                    raise CorpusError("archive contains duplicate paths")
                seen.add(relative)
                if member.issym() or member.islnk() or member.isdev() or member.isfifo() or member.issparse():
                    raise CorpusError("archive contains a link or special file")
                if not (member.isdir() or member.isfile()):
                    raise CorpusError("archive contains an unsupported file type")
                if member.size < 0 or member.size > max_file_bytes:
                    raise CorpusError("archive member exceeds the file-size limit")
                kind = "directory" if member.isdir() else "file"
                for length in range(1, len(relative)):
                    parent = relative[:length]
                    if path_kinds.get(parent) == "file":
                        raise CorpusError("archive file path conflicts with a child member")
                    paths_with_descendants.add(parent)
                if kind == "file" and relative in paths_with_descendants:
                    raise CorpusError("archive file path conflicts with a child member")
                path_kinds[relative] = kind
                if member.isfile():
                    total_declared += member.size
                    if total_declared > max_unpacked_bytes:
                        raise CorpusError("corpus archive exceeds the unpacked-size limit")
                checked.append((member, relative))
            if not root_seen:
                raise CorpusError("archive is missing its expected root directory")
            if _member_list_sha256(members) != expected_member_list_sha256:
                raise CorpusError("corpus archive member list does not match its pin")
            total_written = 0
            for member, parts in checked:
                target = destination.joinpath(*parts)
                if member.isdir():
                    target.mkdir(mode=0o700, parents=True, exist_ok=True)
                    continue
                target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
                descriptor = os.open(target, flags, 0o600)
                copied = 0
                stream = archive.extractfile(member)
                if stream is None:
                    os.close(descriptor)
                    raise CorpusError("archive member could not be read")
                with os.fdopen(descriptor, "wb") as output, stream:
                    while True:
                        chunk = stream.read(64 * 1024)
                        if not chunk:
                            break
                        copied += len(chunk)
                        total_written += len(chunk)
                        if copied > member.size or total_written > max_unpacked_bytes:
                            raise CorpusError("archive exceeded the unpacked-size limit")
                        output.write(chunk)
                if copied != member.size:
                    raise CorpusError("archive member size did not match its header")
        return destination
    except BaseException:
        shutil.rmtree(destination, ignore_errors=True)
        raise


def fetch_corpus(cache_dir: Path, work_dir: Path) -> list[SourceFile]:
    cache_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    if not work_dir.is_dir():
        raise CorpusError("corpus work directory must already exist")
    sources = _manifest()
    expected_suites = {"ansi-test", "alexandria", "cl-ppcre", "asdf"}
    if set(sources) != expected_suites:
        raise CorpusError("corpus manifest source list is incomplete")
    files: list[SourceFile] = []
    for suite, source in sources.items():
        kind = source.get("kind")
        expected = source.get("sha256")
        if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
            raise CorpusError(f"corpus source checksum is invalid: {suite}")
        payload = _archive_bytes(source, suite, cache_dir)
        if kind == "tar.gz":
            root_name = source.get("top_level")
            unpacked = source.get("max_unpacked_bytes")
            if not isinstance(root_name, str) or not isinstance(unpacked, int):
                raise CorpusError(f"archive metadata is invalid: {suite}")
            root = _extract_tar(
                payload,
                work_dir / suite,
                top_level=root_name,
                max_unpacked_bytes=unpacked,
                expected_member_list_sha256=source.get("member_list_sha256", ""),
            )
            extensions = source.get("extensions")
            if not isinstance(extensions, list) or not all(isinstance(ext, str) for ext in extensions):
                raise CorpusError(f"source extensions are invalid: {suite}")
            selected = sorted(
                path for path in root.rglob("*")
                if path.is_file() and path.suffix.lower() in extensions
            )
            for path in selected:
                relative = path.relative_to(root).as_posix()
                files.append(SourceFile(suite, str(source["version"]), relative, path))
        elif kind == "file":
            filename = source.get("archive_name")
            if not isinstance(filename, str) or not _SAFE_NAME.fullmatch(filename):
                raise CorpusError("single-file corpus name is invalid")
            output = work_dir / filename
            with output.open("xb") as stream:
                stream.write(payload)
            files.append(SourceFile(suite, str(source["version"]), filename, output))
        else:
            raise CorpusError(f"corpus source kind is unsupported: {suite}")
    synthetic = work_dir / "synthetic-2000-defun.lisp"
    synthetic.write_text(generate_source(), encoding="utf-8", newline="\n")
    files.append(SourceFile("synthetic", "2000-defun", synthetic.name, synthetic))
    return files
