# Crate architecture

## Decision

This document revises the crate map frozen on 2026-09-24. The target map governs Wave 1, Wave 2, and the compiler stages that follow. Crates represent bounded contexts named with Common Lisp HyperSpec (CLHS) terms or compiler stages. Modules inside each crate follow the same feature boundary. The revision is explicit: it adds `ncl-hir` and the HIR dependency edges listed below while retaining the prohibition on `ncl-opt -> ncl-runtime`.

The kernel crate ncl-object owns Word, object tags and layouts, classification required by heap-facing code, heap and rooting APIs, ThreadContext, Runtime's generic registries, number representations and primitives, and the structure layout, registry, class APIs, and heap operations. Lisp-visible hash-table operations and option interpretation belong to ncl-hash-tables. Other feature model crates depend on the kernel. The kernel does not depend on a feature crate.

Registration belongs to the crate that owns the CLHS feature. A feature registration function registers only that feature. ncl-stdlib is the composition root that calls feature registration functions in dependency order. Runtime calls ncl-stdlib rather than owning the order table.

External dependencies remain prohibited. Unsafe code remains confined to ncl-sys. Feature crates own typed error enums; adapters map those errors to Lisp conditions under the mapping policy in docs/src/design/conditions.md.

## Current workspace at the baseline

The baseline is commit 6aa6639d. The root manifest lists 38 workspace members in Cargo.toml:1-41. Running cargo metadata --format-version 1 --no-deps exited 0 and reported 38 packages and zero non-workspace dependency edges. The table records normal edges and test-only dev edges. Manifest references show the declarations; packages with no edges have none in the metadata output.

| Crate | Normal dependencies | Test-only dependencies | Manifest evidence |
| --- | --- | --- | --- |
| ncl | ncl-runtime | ncl-lib-streams | Cargo.toml:111,114 |
| ncl-asdf | ncl-object, ncl-conditions, ncl-clos, ncl-uiop | none | packages/asdf/Cargo.toml:9-12 |
| ncl-asm-aarch64 | none | none | packages/asm/aarch64/Cargo.toml:1-10; cargo metadata (no edges) |
| ncl-asm-x86-64 | none | none | packages/asm/x86-64/Cargo.toml:1-12; cargo metadata (no edges) |
| ncl-clos | ncl-object, ncl-lib-numbers, ncl-lib-streams, ncl-types, ncl-conditions | none | packages/clos/Cargo.toml:9-13 |
| ncl-codegen | ncl-ir, ncl-asm-x86-64, ncl-asm-aarch64, ncl-object, ncl-sys | ncl-disasm, ncl-objfile | packages/codegen/Cargo.toml:9-17 |
| ncl-compiler-front | ncl-ir, ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-clos | ncl-ownership | packages/compiler/front/Cargo.toml:9-17 |
| ncl-opt | ncl-ir | none | packages/compiler/opt/Cargo.toml:10 |
| ncl-conditions | ncl-object, ncl-ownership | ncl-sys | packages/conditions/Cargo.toml:9-13 |
| ncl-conformance | ncl-runtime, ncl-ownership | none | packages/conformance/Cargo.toml:9-10 |
| ncl-coverage | ncl-object, ncl-compiler-front | none | packages/coverage/Cargo.toml:9-10 |
| ncl-debug | ncl-object, ncl-sys, ncl-conditions, ncl-codegen, ncl-lib-streams | none | packages/debug/Cargo.toml:9-13 |
| ncl-disasm | ncl-object | ncl-asm-x86-64, ncl-asm-aarch64 | packages/disasm/Cargo.toml:10,13-14 |
| ncl-ffi | ncl-object, ncl-sys, ncl-conditions, ncl-ownership | none | packages/ffi/Cargo.toml:9-12 |
| ncl-image | ncl-object, ncl-objfile, ncl-sys | ncl-ownership | packages/image/Cargo.toml:9-14 |
| ncl-ir | none | none | packages/ir/Cargo.toml:1-10; cargo metadata (no edges) |
| ncl-lib-format | ncl-object, ncl-types, ncl-conditions, ncl-lib-streams, ncl-lib-strings, ncl-printer | none | packages/lib/format/Cargo.toml:9-14 |
| ncl-lib-hash-arrays | ncl-object, ncl-types, ncl-conditions | ncl-clos | packages/lib/hash-arrays/Cargo.toml:9-14 |
| ncl-lib-macros | ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-lib-sequences | ncl-sys | packages/lib/macros/Cargo.toml:9-16 |
| ncl-lib-numbers | ncl-object, ncl-sys, ncl-types, ncl-conditions | none | packages/lib/numbers/Cargo.toml:9-12 |
| ncl-lib-packages | ncl-object, ncl-types, ncl-conditions | none | packages/lib/packages/Cargo.toml:9-11 |
| ncl-lib-pathnames | ncl-object, ncl-clos, ncl-types, ncl-conditions, ncl-lib-strings, ncl-sys | none | packages/lib/pathnames/Cargo.toml:9-14 |
| ncl-lib-sequences | ncl-object, ncl-lib-numbers, ncl-types, ncl-conditions | none | packages/lib/sequences/Cargo.toml:9-12 |
| ncl-lib-streams | ncl-object, ncl-types, ncl-conditions, ncl-sys | none | packages/lib/streams/Cargo.toml:9-12 |
| ncl-lib-strings | ncl-object, ncl-types, ncl-conditions | none | packages/lib/strings/Cargo.toml:9-11 |
| ncl-object | ncl-sys | ncl-ownership | packages/object/Cargo.toml:9,12 |
| ncl-objfile | none | none | packages/objfile/Cargo.toml:1-10; cargo metadata (no edges) |
| ncl-os | ncl-object, ncl-sys, ncl-conditions, ncl-lib-streams | none | packages/os/Cargo.toml:9-12 |
| ncl-ownership | ncl-object | none | packages/ownership/Cargo.toml:9 |
| ncl-printer | ncl-object, ncl-conditions | ncl-ownership, ncl-lib-streams | packages/printer/Cargo.toml:9-14 |
| ncl-profiler | ncl-object, ncl-sys, ncl-threads, ncl-conditions | none | packages/profiler/Cargo.toml:9-12 |
| ncl-reader | ncl-object, ncl-lib-pathnames | ncl-lib-packages, ncl-ownership | packages/reader/Cargo.toml:9-14 |
| ncl-runtime | ncl-compiler-front, ncl-opt, ncl-ir, ncl-codegen, ncl-conditions, ncl-stdlib, ncl-image, ncl-objfile, ncl-threads, ncl-ffi, ncl-object, ncl-types, ncl-lib-pathnames, ncl-printer, ncl-reader, ncl-sys, ncl-asm-aarch64, ncl-asm-x86-64 | none | packages/runtime/Cargo.toml:9-26 |
| ncl-stdlib | ncl-object, ncl-sys, ncl-compiler-front, ncl-types, ncl-reader, ncl-printer, ncl-conditions, ncl-clos, ncl-lib-numbers, ncl-lib-sequences, ncl-lib-strings, ncl-lib-hash-arrays, ncl-lib-streams, ncl-lib-pathnames, ncl-lib-packages, ncl-lib-format, ncl-lib-macros, ncl-threads, ncl-ffi, ncl-image, ncl-os, ncl-uiop, ncl-asdf, ncl-profiler, ncl-coverage, ncl-debug, ncl-disasm | none | packages/stdlib/Cargo.toml:9-35 |
| ncl-sys | none | none | packages/sys/Cargo.toml:1-12; cargo metadata (no edges) |
| ncl-threads | ncl-object, ncl-sys, ncl-conditions, ncl-ownership | none | packages/threads/Cargo.toml:9-12 |
| ncl-types | ncl-object | ncl-ownership | packages/types/Cargo.toml:9-12 |
| ncl-uiop | ncl-object, ncl-conditions, ncl-lib-streams, ncl-lib-pathnames, ncl-lib-strings, ncl-os | none | packages/uiop/Cargo.toml:9-14 |

The earlier crate-map prose treated `ncl-codegen -> ncl-objfile` as a production edge. The baseline manifest declares it as a dev-only edge (`packages/codegen/Cargo.toml:17`), so the current graph follows the manifest. The frozen contract also barred new workspace members and reverse dependency edges. This revision makes a scoped exception for `ncl-hir`, allowing exactly these HIR edges: `ncl-compiler-front -> ncl-hir`, `ncl-hir -> ncl-ir`, and `ncl-opt -> ncl-hir`. It still prohibits `ncl-opt -> ncl-runtime`, which would reverse the existing `ncl-runtime -> ncl-opt` edge.

## Feature module ownership

The table records the owner for each feature currently represented in ncl-object. CLHS builtins and registration remain in per-feature ncl-lib-* crates, ncl-reader, or ncl-clos. The target splits the current ncl-lib-hash-arrays adapter into ncl-lib-arrays and ncl-lib-hash-tables, each with a registration function for only its feature. This keeps feature registration out of the kernel and keeps Runtime from owning registration order.

| Current module | Target owner | Evidence and current consumers |
| --- | --- | --- |
| array and specialized_array | ncl-arrays | CLHS operations and builtins are in packages/lib/hash-arrays/src/arrays.rs:1-21,238-250,284-323. Callers include packages/lib/sequences/src/domain/copy_seq.rs:3-4,31-49, packages/lib/strings/src/builtins/string_ops.rs:9-31, packages/compiler/front/src/form.rs:310-353, packages/runtime/src/constants.rs:4-6,121-179, and packages/printer/src/array.rs:5,31-39. The kernel keeps representation primitives in packages/object/src/array_primitives.rs, including make_specialized_array for packages/reader/src/dispatch.rs:249 and specialized_array_ref for packages/image/src/save.rs:230. |
| hash-table model and Lisp-facing operations | ncl-hash-tables | The kernel keeps the heap representation, layout, key equality/hash, construction, lookup, insertion, removal, and iteration used by Package's INTERNAL/EXTERNAL slots. ncl-hash-tables owns option interpretation and the feature-facing API over that core. Lisp builtins and registration are in packages/lib/hash-tables/src/hash_tables.rs and packages/lib/hash-tables/src/lib.rs. |
| package | ncl-packages | packages/object/src/lib.rs:27,83 and packages/object/src/registry_extensions.rs:17-60. CLHS functions are in packages/lib/packages/src/introspection/intern_symbol.rs:1-3,25-43. Current callers include packages/reader/src/token.rs:431-442, packages/compiler/front/src/register.rs:31-32, and packages/runtime/src/arith_fast.rs:22-48. |
| stream | ncl-streams | packages/object/src/lib.rs:38,105-109. Stream operations are in packages/lib/streams/src/file/adapters.rs:98,216-229,282-314. |
| structure and structure_registry | ncl-object kernel | StructureLayout and Runtime's structure APIs use private kernel state (packages/object/src/runtime.rs:28-31,92,134-227; packages/object/src/structure.rs:5-57). Keep layout, registry, class APIs, construction, and slot access together in ncl-object. |
| number | ncl-object kernel | Array primitives, hash-table equality/options, GC, and typed numeric access call number representations and operations directly (packages/object/src/array_primitives.rs:57-64; packages/object/src/hash_table/equality.rs:79-115,173-214; packages/object/src/hash_table/options.rs:106-202; packages/object/src/gc.rs:68-78; packages/object/src/typed/numeric.rs:34-59). Keep number representations, allocation/access primitives, and typed wrappers in ncl-object. Lisp numeric builtins remain in ncl-lib-numbers. |
| readtable | ncl-readtables | packages/object/src/lib.rs:30,86-89. Reader behavior is in packages/reader/src/readtable.rs:84-140 and packages/reader/src/copy_readtable.rs:2-4. Runtime load options use the object descriptor at packages/runtime/src/load/read_options.rs:4-7,36. |

The kernel retains the tags, layout descriptors, classification, and representation-level operations needed to inspect or construct heap objects without importing feature crates. Its source concentrates those contracts in packages/object/src/layout.rs, packages/object/src/classify.rs:69-80, packages/object/src/array_primitives.rs, and packages/object/src/gc.rs:76-119,144-152. Runtime keeps generic registries in packages/object/src/runtime.rs:22-40. CLHS domain APIs for Package, HashTable, Stream, Readtable, and Array do not remain as ncl-object public re-exports. The hash-table core is a representation-level exception required by package storage. Array representation primitives such as ArrayElementType, array_dimensions, specialized_array_element_type, make_specialized_array, specialized_array_ref, and specialized_array_set remain in ncl-object; ncl-arrays owns CLHS array operations.

## Target crate map

The allowed dependency column lists normal direct dependencies. A crate may add an edge only by updating this contract and `docs/src/design/crate-dependencies.tsv`. The TSV records the target production graph; test-only edges remain lane-specific until a test move establishes the destination dependencies.

| Crate | Bounded context and owned features | Allowed direct dependencies |
| --- | --- | --- |
| ncl | command-line entry point | ncl-runtime |
| ncl-sys | operating-system ABI, memory, threads, platform calls, and signal-recovery trampoline | none |
| ncl-object | Word, layout and tags, classification, heap API, roots, ThreadContext, Runtime registries, number representations, allocation/access primitives and typed wrappers, structure layout and registry, class APIs, construction and slot access, package hash-table representation and core operations | ncl-sys |
| ncl-arrays | CLHS array and vector model, rank, element type, displacement, adjustment, storage | ncl-object |
| ncl-hash-tables | Lisp-visible hash-table API and option interpretation over the object hash-table core | ncl-object |
| ncl-packages | CLHS package model, symbol membership, export, import, shadowing and use-list | ncl-object, ncl-hash-tables |
| ncl-streams | CLHS stream state, element type, direction, file and character stream model | ncl-object, ncl-sys |
| ncl-readtables | CLHS readtable representation and syntax state | ncl-object |
| ncl-compiler-front | source forms, macro expansion, declarations, and lowering to HIR | ncl-hir, ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-clos, ncl-arrays, ncl-packages |
| ncl-hir | Lisp-semantic high-level IR and compilation-unit index for function definition locations; located at packages/compiler/hir/ | ncl-ir |
| ncl-ir | compiler-independent intermediate representation | none |
| ncl-opt | HIR optimization and inlining that preserve Lisp meaning | ncl-hir |
| ncl-codegen | machine functions, register allocation, frames and safepoints | ncl-ir, ncl-asm-x86-64, ncl-asm-aarch64, ncl-object, ncl-sys |
| ncl-asm-x86-64 | x86-64 instruction model and encoder | none |
| ncl-asm-aarch64 | AArch64 instruction model and encoder | none |
| ncl-objfile | object-file and FASL byte encoding | none |
| ncl-types | type specifiers, predicates and type errors | ncl-object |
| ncl-conditions | Lisp conditions, handlers, restarts and cleanup | ncl-object, ncl-ownership |
| ncl-reader | tokenization, reader, syntax dispatch and input forms | ncl-object, ncl-lib-pathnames, ncl-packages, ncl-readtables |
| ncl-printer | readable and pretty printing | ncl-object, ncl-conditions, ncl-arrays, ncl-packages |
| ncl-clos | classes, slots, generic functions, MOP and defstruct integration | ncl-object, ncl-types, ncl-conditions, ncl-lib-numbers, ncl-lib-streams |
| ncl-lib-numbers | CLHS numeric functions and numeric builtin registration | ncl-object, ncl-types, ncl-conditions, ncl-sys |
| ncl-lib-sequences | CLHS list, sequence, tree and equality functions | ncl-object, ncl-lib-numbers, ncl-types, ncl-conditions, ncl-arrays, ncl-hash-tables |
| ncl-lib-strings | CLHS character and string functions | ncl-object, ncl-types, ncl-conditions, ncl-arrays |
| ncl-lib-arrays | CLHS array and vector functions and registration | ncl-object, ncl-types, ncl-conditions, ncl-arrays |
| ncl-lib-hash-tables | CLHS hash-table functions and registration | ncl-object, ncl-types, ncl-conditions, ncl-hash-tables |
| ncl-lib-packages | CLHS package functions and registration | ncl-object, ncl-types, ncl-conditions, ncl-packages |
| ncl-lib-streams | CLHS stream functions and registration | ncl-object, ncl-types, ncl-conditions, ncl-sys, ncl-streams |
| ncl-lib-pathnames | pathname model and file-system functions | ncl-object, ncl-clos, ncl-types, ncl-conditions, ncl-lib-strings, ncl-sys, ncl-hash-tables, ncl-packages |
| ncl-lib-format | FORMAT parser, directives, stream output and registration | ncl-object, ncl-types, ncl-conditions, ncl-lib-streams, ncl-lib-strings, ncl-printer, ncl-packages |
| ncl-lib-macros | CLHS macro and function expansion functions | ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-lib-sequences |
| ncl-stdlib | composition root for registration order | ncl-object, ncl-sys, ncl-compiler-front, ncl-types, ncl-reader, ncl-printer, ncl-conditions, ncl-clos, ncl-lib-numbers, ncl-lib-sequences, ncl-lib-strings, ncl-lib-arrays, ncl-lib-hash-tables, ncl-lib-streams, ncl-lib-pathnames, ncl-lib-packages, ncl-lib-format, ncl-lib-macros, ncl-environment, ncl-threads, ncl-ffi, ncl-image, ncl-disasm, ncl-os, ncl-uiop, ncl-asdf, ncl-profiler, ncl-coverage, ncl-debug |
| ncl-runtime | evaluation, compilation orchestration, loading, literal materialization, and the type-check failure signaling entry point (API name set by conditions.md) | ncl-compiler-front, ncl-opt, ncl-ir, ncl-codegen, ncl-conditions, ncl-stdlib, ncl-image, ncl-objfile, ncl-threads, ncl-ffi, ncl-object, ncl-types, ncl-lib-pathnames, ncl-printer, ncl-reader, ncl-sys, ncl-asm-aarch64, ncl-asm-x86-64, ncl-arrays, ncl-packages, ncl-readtables |
| ncl-image | image serialization and restoration | ncl-object, ncl-objfile, ncl-sys, ncl-hash-tables, ncl-packages, ncl-streams, ncl-readtables |
| ncl-threads | Lisp thread API | ncl-object, ncl-sys, ncl-conditions, ncl-ownership |
| ncl-ffi | foreign declarations and calls | ncl-object, ncl-sys, ncl-conditions, ncl-ownership, ncl-arrays |
| ncl-ownership | ownership-table parsing and validation, with no Lisp feature API | ncl-object |
| ncl-conformance | conformance runner | ncl-runtime, ncl-ownership; reintroduce with L0-6 when its runner is assigned |
| ncl-disasm | disassembly interfaces | ncl-object |
| ncl-environment | environment variables, command-line arguments and process environment | ncl-object, ncl-sys, ncl-conditions |
| ncl-introspection | inspection of functions, code metadata and runtime state | ncl-object, ncl-codegen, ncl-sys, ncl-conditions |
| ncl-os | OS-facing Lisp functions | ncl-object, ncl-sys, ncl-conditions, ncl-lib-streams; recreate for L2-ch19/20 and extend in L4-3 |
| ncl-uiop | UIOP pathname, stream and process functions | ncl-object, ncl-conditions, ncl-lib-streams, ncl-lib-pathnames, ncl-lib-strings, ncl-os; recreate for L4-4 |
| ncl-asdf | ASDF systems, components and operations | ncl-object, ncl-conditions, ncl-clos, ncl-uiop; recreate for L4-4 |
| ncl-debug | debugger and SLIME/SLY integration | ncl-object, ncl-sys, ncl-conditions, ncl-codegen, ncl-lib-streams, ncl-introspection; recreate for L4-5 |
| ncl-profiler | runtime profiling | ncl-object, ncl-sys, ncl-threads, ncl-conditions; recreate for L4-6 |
| ncl-coverage | coverage collection and reporting | ncl-object, ncl-compiler-front; recreate for L4-6 |

The HIR compilation-unit index maps each function to its definition location and belongs in `ncl-hir`, where `ncl-opt` can read it. `ncl-runtime` already depends on `ncl-opt`; placing the index in runtime would require `ncl-opt -> ncl-runtime` and create a cycle with the existing `ncl-runtime -> ncl-opt` edge.

The HIR design gives `ncl-hir` a compiler-only type lattice, separate from `ncl-types`, which owns runtime type specifiers and `TYPEP`/`SUBTYPEP`. It directs the compiler front end to map known `ncl-types::TypeSpecifier` constructors into HIR nodes and preserve unknown or implementation-defined specifiers as opaque nodes.

Feature model crates own typed error enums and do not depend on Lisp conditions. Lisp-facing adapters such as ncl-lib-* and ncl-clos depend on both the model and ncl-conditions and map model errors at that boundary. Other consumers propagate their own typed errors for mapping at the outer Lisp API boundary. ncl-conditions does not depend on feature model crates. Condition classes and mapping rules belong in docs/src/design/conditions.md.

Baseline dev edges are listed separately in the current-workspace table. They do not automatically become target test edges; each feature lane records only the dev dependencies still required after its test move.

## Allowed dependency data and checker

Store the target direct-edge allowlist in `docs/src/design/crate-dependencies.tsv`. Use columns `from`, `to`, `kind`, and `target`. `kind` is `normal`, `dev`, or `build`. `target` is empty for an unconditional edge and otherwise holds Cargo's target expression. One row represents one permitted direct edge. Do not infer permission from transitive reachability. The file describes the target graph, including `ncl-compiler-front -> ncl-hir`, `ncl-hir -> ncl-ir`, and `ncl-opt -> ncl-hir`. The HIR design forbids `ncl-hir -> ncl-types`; `ncl-types -> ncl-hir` is not listed in the target crate map.

The checker `scripts/check_crate_dependencies.py` reads workspace and member `Cargo.toml` files, the target allowlist, and the exception table. It compares normal dependencies from `[dependencies]` and `[target.*.dependencies]` with the allowlist and exceptions, skips dependency packages that are not workspace members, and rejects stale exceptions. It does not inspect `dev-dependencies` or `build-dependencies`; those kinds are outside this check.

The checker does not build or traverse a dependency graph, so it does not check for cycles in either the target graph or the current normal/build dependency graph.

## Differences from the prior freeze

This target contract adds five feature model crates: `ncl-arrays`, `ncl-hash-tables`, `ncl-packages`, `ncl-streams`, and `ncl-readtables`. It splits the combined array and hash-table registration adapter, keeps `ncl-object` as a kernel, and adds `ncl-hir` for Lisp-semantic HIR and its compilation-unit index. It also assigns the signal-recovery trampoline to `ncl-sys` and a type-check-failure signaling entry point to `ncl-runtime`; `docs/src/design/conditions.md` chooses that entry point's API name.
D-07 removes placeholder crates from the workspace until an owning lane needs them. Keep their crate names when they return: ncl-asdf, ncl-uiop, ncl-os, ncl-debug, ncl-profiler, ncl-coverage, and ncl-conformance. Their target rows above assign names and expected edges. L0-6 restores ncl-conformance; L2-ch19/20 restores ncl-os, which L4-3 extends; L4-4 restores ncl-uiop and ncl-asdf; L4-5 restores ncl-debug; L4-6 restores ncl-profiler and ncl-coverage. ncl-environment is the new crate for L2-ch24/25. ncl-introspection is the new crate for L4-5.

## Migration order

The migration rows follow section 7 of the NCL run specification. A lane is an independently owned work unit with crate and path assignments and prerequisites. Wave 1 covers blockers and enabling work; Wave 2 covers CLHS chapter work. L0-2 freezes contracts before these waves. Wave 1 lanes may run in parallel where their prerequisites allow. L1-4 waits until L1-10 has moved the object features and established the kernel boundary; L1-10 uses move-only commits for that split. `L*` and `L2-ch*` are lane identifiers from the run plan.

| Order | Lane | Crate ownership and migration | Prerequisite |
| --- | --- | --- | --- |
| Wave 1 | L1-1 | runtime, conditions, codegen trap path | L0-2 |
| Wave 1 | L1-2 | sys and threads | L0-2 |
| Wave 1 | L1-3 | sys and runtime publication | L0-2 |
| Wave 1 | L1-5 | lib/format, which owns ncl-lib-format | L1-1 |
| Wave 1 | L1-6 | reader and readtable behavior | L1-1 |
| Wave 1 | L1-7 | lib/numbers and numeric reader/printer paths | L1-1 |
| Wave 1 | L1-8 | printer | L1-5 |
| Wave 1 | L1-9 | lib/streams, which owns ncl-lib-streams | L1-1 |
| Wave 1 | L1-10 | ncl-object feature split and destination crates | L0-2 |
| Wave 1 | L1-4 | sys and object heap, collector, barrier, weak, rehash and verify work | L0-2 and L1-10 |
| Wave 1 | L1-11 | workspace lint and quality rules; root manifest stays with FR-011 owner | L0-2 |
| Wave 2 | L2-ch3 | compiler-front and runtime | Wave 1 |
| Wave 2 | L2-ch4 | types | Wave 1 |
| Wave 2 | L2-ch5/14/17 | macros and sequences | Wave 1 |
| Wave 2 | L2-ch6 | macro loop implementation | L2-ch5 |
| Wave 2 | L2-ch7 | CLOS | Wave 1 |
| Wave 2 | L2-ch8 | defstruct within CLOS | L2-ch7 |
| Wave 2 | L2-ch9 | conditions | L1-1 |
| Wave 2 | L2-ch10/11 | packages | Wave 1 |
| Wave 2 | L2-ch12 | remaining numbers work | L1-7 |
| Wave 2 | L2-ch13/16 | strings | Wave 1 |
| Wave 2 | L2-ch15/18 | split lib/arrays and lib/hash-tables from lib/hash-arrays | L1-4 rehash |
| Wave 2 | L2-ch19/20 | pathnames and OS | Wave 1 |
| Wave 2 | L2-ch24/25 | runtime environment and ncl-environment | Wave 1 |

Give each crate and shared path one lane owner at a time. Keep root Cargo.toml with its existing FR-011 owner. Before closing a lane, record its functional-requirement evidence (FR), verification-layer results (V), conformance scoreboard diff, Rust quality checks (J), and confirmation that it added no `#[allow]` attributes. Run no more than 16 lanes concurrently. The lane rows above follow the section 7 plan.

Decision D-08 requires each feature lane to start with one behavior-neutral, move-only commit. Record the selected test names and count before the move; run the same selection across source and destination crates after the move. The selection must be non-empty when the source had tests, names and count must match, and all selected tests must pass. Put behavior changes in a later commit. L1-10 applies this procedure to each moved object feature.

