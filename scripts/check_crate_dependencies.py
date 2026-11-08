#!/usr/bin/env python3
"""Check workspace production dependencies against the crate edge table."""

from __future__ import annotations

import csv
import glob
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EDGE_TABLE = Path("docs/src/design/crate-dependencies.tsv")
EDGE_COLUMNS = ("from", "to", "kind", "target")
EXCEPTION_TABLE = Path("docs/src/design/crate-dependency-exceptions.tsv")
EXCEPTION_COLUMNS = (*EDGE_COLUMNS, "owner_lane")


def load_toml(path: Path, errors: list[str]) -> dict:
    try:
        with path.open("rb") as source:
            return tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        errors.append(f"{path}: cannot read Cargo.toml: {error}")
        return {}


def workspace_manifests(root: Path, workspace: dict, errors: list[str]) -> list[Path]:
    manifests = {root / "Cargo.toml"}
    members = workspace.get("workspace", {}).get("members", [])
    if not isinstance(members, list):
        errors.append(f"{root / 'Cargo.toml'}: workspace.members must be an array")
        return sorted(manifests)

    for member in members:
        if not isinstance(member, str):
            errors.append(f"{root / 'Cargo.toml'}: workspace member must be a string")
            continue
        matches = sorted(glob.glob(str(root / member), recursive=True))
        if not matches:
            errors.append(f"{root / 'Cargo.toml'}: workspace member matched no paths: {member}")
            continue
        for match in matches:
            path = Path(match)
            manifest = path if path.name == "Cargo.toml" else path / "Cargo.toml"
            if not manifest.is_file():
                errors.append(f"{manifest}: workspace member has no Cargo.toml")
                continue
            manifests.add(manifest)
    return sorted(manifests)


def allowed_edges(path: Path, errors: list[str]) -> set[tuple[str, str, str, str]]:
    edges = set()
    try:
        with path.open(encoding="utf-8", newline="") as source:
            rows = csv.reader(source, delimiter="\t")
            header = next(rows, None)
            if tuple(header or ()) != EDGE_COLUMNS:
                errors.append(f"{path}: expected TSV columns {' '.join(EDGE_COLUMNS)}")
                return edges
            for line_number, row in enumerate(rows, 2):
                if len(row) != len(EDGE_COLUMNS):
                    errors.append(f"{path}:{line_number}: expected {len(EDGE_COLUMNS)} columns")
                    continue
                source_crate, target_crate, kind, target = row
                if not source_crate or not target_crate or not kind:
                    errors.append(f"{path}:{line_number}: source, destination and kind are required")
                    continue
                edges.add((source_crate, target_crate, kind, target))
    except OSError as error:
        errors.append(f"{path}: cannot read edge table: {error}")
    return edges


def dependency_exceptions(
    path: Path, errors: list[str]
) -> dict[tuple[str, str, str, str], str]:
    exceptions = {}
    try:
        with path.open(encoding="utf-8", newline="") as source:
            rows = csv.reader(source, delimiter="\t")
            header = next(rows, None)
            if tuple(header or ()) != EXCEPTION_COLUMNS:
                errors.append(
                    f"{path}: expected TSV columns {' '.join(EXCEPTION_COLUMNS)}"
                )
                return exceptions
            for line_number, row in enumerate(rows, 2):
                if len(row) != len(EXCEPTION_COLUMNS):
                    errors.append(
                        f"{path}:{line_number}: expected {len(EXCEPTION_COLUMNS)} columns"
                    )
                    continue
                source_crate, target_crate, kind, target, owner_lane = row
                if not source_crate or not target_crate or kind != "normal" or not owner_lane:
                    errors.append(
                        f"{path}:{line_number}: source, destination, normal kind and owner lane are required"
                    )
                    continue
                edge = (source_crate, target_crate, kind, target)
                if edge in exceptions:
                    errors.append(
                        f"{path}:{line_number}: duplicate exception {source_crate} -> {target_crate}"
                    )
                    continue
                exceptions[edge] = owner_lane
    except OSError as error:
        errors.append(f"{path}: cannot read exception table: {error}")
    return exceptions


def dependency_tables(manifest: dict):
    # The edge table records production dependencies; dev-only edges remain lane-specific.
    dependencies = manifest.get("dependencies", {})
    if isinstance(dependencies, dict):
        for name, spec in dependencies.items():
            yield name, spec, ""

    targets = manifest.get("target", {})
    if isinstance(targets, dict):
        for target, config in targets.items():
            if not isinstance(config, dict):
                continue
            dependencies = config.get("dependencies", {})
            if isinstance(dependencies, dict):
                for name, spec in dependencies.items():
                    yield name, spec, target


def target_package(
    name: str,
    spec: object,
    manifest_path: Path,
    root: Path,
    workspace_dependencies: dict,
    workspace_packages: set[str],
    errors: list[str],
) -> str | None:
    if not isinstance(spec, dict):
        spec = {}

    base = manifest_path.parent
    if spec.get("workspace") is True:
        workspace_spec = workspace_dependencies.get(name)
        if workspace_spec is None:
            errors.append(f"{manifest_path}: workspace dependency {name} is not declared")
            return None
        if isinstance(workspace_spec, dict):
            merged = dict(workspace_spec)
            merged.update({key: value for key, value in spec.items() if key != "workspace"})
            spec = merged
        else:
            spec = {"version": workspace_spec}
        base = root

    path = spec.get("path")
    package_name = spec.get("package", name)
    if path is not None:
        target_manifest = (base / path / "Cargo.toml").resolve()
        target_data = load_toml(target_manifest, errors)
        package_name = target_data.get("package", {}).get("name")
        if not isinstance(package_name, str):
            errors.append(f"{target_manifest}: path dependency has no package name")
            return None
        return package_name

    if package_name in workspace_packages:
        return package_name
    return None


def check_dependencies(root: Path) -> tuple[int, int, list[str]]:
    errors = []
    root_manifest = root / "Cargo.toml"
    workspace = load_toml(root_manifest, errors)
    manifests = workspace_manifests(root, workspace, errors)
    edges = allowed_edges(root / EDGE_TABLE, errors)
    exceptions = dependency_exceptions(root / EXCEPTION_TABLE, errors)

    package_names = {}
    package_manifests = {}
    for manifest_path in manifests:
        manifest = load_toml(manifest_path, errors)
        name = manifest.get("package", {}).get("name")
        if not isinstance(name, str):
            if manifest_path == root_manifest and "workspace" in manifest:
                continue
            errors.append(f"{manifest_path}: workspace package has no package name")
            continue
        if name in package_manifests:
            errors.append(f"{manifest_path}: duplicate workspace package name {name}")
            continue
        package_names[name] = manifest
        package_manifests[name] = manifest_path

    workspace_dependencies = workspace.get("workspace", {}).get("dependencies", {})
    if not isinstance(workspace_dependencies, dict):
        errors.append(f"{root_manifest}: workspace.dependencies must be a table")
        workspace_dependencies = {}

    checked_edges = 0
    observed_edges = set()
    for source_name, source_manifest in package_names.items():
        manifest_path = package_manifests[source_name]
        for dependency, spec, target in dependency_tables(source_manifest):
            target_name = target_package(
                dependency,
                spec,
                manifest_path,
                root,
                workspace_dependencies,
                set(package_names),
                errors,
            )
            if target_name is None:
                continue
            checked_edges += 1
            edge = (source_name, target_name, "normal", target)
            observed_edges.add(edge)
            if edge not in edges and edge not in exceptions:
                location = manifest_path.relative_to(root)
                target_label = f" under {target}" if target else ""
                errors.append(
                    f"{location}: dependency {source_name} -> {target_name}{target_label} "
                    "is not allowed by the edge table or exception table"
                )

    for edge, owner_lane in exceptions.items():
        source_name, target_name, _, target = edge
        target_label = f" under {target}" if target else ""
        if edge in edges:
            errors.append(
                f"exception {source_name} -> {target_name}{target_label} owned by {owner_lane} "
                "is stale: the edge is now allowed by the edge table"
            )
        elif edge not in observed_edges:
            errors.append(
                f"exception {source_name} -> {target_name}{target_label} owned by {owner_lane} "
                "is stale: the dependency is no longer declared"
            )

    return len(package_names), checked_edges, errors


def main() -> int:
    packages, edges, errors = check_dependencies(ROOT)
    print(f"workspace packages checked: {packages}")
    print(f"production dependency edges checked: {edges}")
    if errors:
        print("violations:")
        print("\n".join(f"- {error}" for error in errors))
        return 1
    print("violations: none")
    return 0


if __name__ == "__main__":
    sys.exit(main())
