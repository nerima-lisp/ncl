# Native FASL v1

## Decision

FASL v1 is a relocatable native compilation unit. It stores machine code, typed constants, symbol imports, relocations, stack maps, debug records, and ordered top-level execution records. Loading a FASL validates and links those records; it does not read or recompile source forms.

The file is target-specific. It is accepted only when its architecture, pointer width, byte order, FASL format, compiler identity, runtime ABI, policy fingerprint, and feature fingerprint match the running compiler/runtime. A well-formed but incompatible file is rejected before any code is allocated or published.

## Current implementation

The current implementation does not satisfy this contract:

- `packages/objfile/src/fasl.rs` lines 65–79 writes container version 2; lines 217–223 accepts versions 1 and 2. The version-2 header contains architecture, pointer width, endianness, feature bits, and fixed ranges for code, relocations, constants, symbols, stack maps, and debug data, but no compiler or policy fingerprint.
- `packages/runtime/src/compile.rs` lines 279–313 runs source through the compile-file top-level pipeline, then writes empty code, relocation, constant, symbol, and stack-map sections. Its debug section contains an `NCLRTFASL` payload with UTF-8 source and an FNV-1a hash.
- `packages/runtime/src/load.rs` lines 102–121 decodes that source and sends it back through the normal source-form execution path. This recompiles at LOAD and violates AC-4's requirement that FASL loading not recompile source.
- Parser failures currently use `ObjectError` and runtime-native errors. The required Lisp-facing `invalid-fasl` condition mapping is not present.
- Images are a separate `NCLIMAGE` format. `packages/image/src/format.rs` lines 12–35 defines version 2 with object records, roots, code blobs, and feature strings. `packages/image/README.md` lines 141–145 says image load parses and discards those feature strings. The image version is not the FASL version.
- The recorded package versions disagree: `flake.nix` line 50 declares `0.1.0` while `Cargo.toml` line 85 declares `0.1.1`.

## Versioned wire format

The proposed native format uses semantic FASL version 1 inside `ncl-objfile` container schema version 3. Runtime `LOAD` accepts this native format and rejects legacy container versions 1 and 2 that encode source payloads; ordinary Lisp source remains loadable through the source-file path. Keep the container-schema version distinct from the semantic FASL format version.

The fixed 128-byte little-endian header has these fields:

| Field | Encoding | Contract |
| --- | --- | --- |
| magic | 8 bytes | Exactly `NCLFASL\0`. |
| container version | `u16` | Exactly 3 for the v1 native container. |
| architecture | `u8` | Existing `ncl-objfile` architecture tag: x86-64 or AArch64. |
| pointer width | `u8` | 8 bytes. |
| endianness | `u8` | 1 means little-endian. |
| header size | `u8` | Exactly 128. |
| reserved | `u16` | Zero. |
| semantic FASL version | `u32` | Exactly 1. |
| runtime ABI version | `u32` | Exact runtime code/object ABI revision. |
| compiler identity | 32 bytes | SHA-256 of compiler semantic version, compiler build identity, and code-generation ABI revision. |
| policy fingerprint | 32 bytes | SHA-256 of canonical optimization and safety policy data. |
| feature fingerprint | 32 bytes | SHA-256 of the sorted required-feature identifiers and versions. |
| section count | `u32` | Exactly 8. |
| directory offset | `u32` | Exactly 128. |

The directory occupies bytes 128 through 319 and contains eight 24-byte entries in ascending section-ID order: metadata, symbols, constants, code, relocations, stack maps, debug records, and top-level records. Each entry stores a `u16` section ID, zero `u16` flags, `u32` alignment, `u64` file offset, and `u64` byte length. Alignment is a power of two from 1 through 4096 bytes. Non-empty section ranges start at or after byte 320, are aligned and in bounds, do not overlap each other, and cover the payload in directory order; padding bytes are zero. Empty sections are present with zero length. No host pointer or process address is serialized.

The metadata section contains the exact compiler semantic version string, compiler build identity, code-generation ABI revision, runtime ABI version, canonical policy values, and sorted feature identifiers and versions from which the header fingerprints are recomputed. It also records the source content digest and target triple. A reader rejects an unknown required feature or any fingerprint mismatch.

The code section is a sequence of code-object records. Each record identifies a stable object number, byte range, entry offset, and references to its stack-map and debug records. Entry and code ranges must lie wholly within the code section. Code bytes are stored before relocation and never contain absolute process addresses.

Relocation records identify a code-object number, code-relative write offset, relocation kind, target kind, target index, and signed addend. Targets are limited to another code object, a typed constant, or an imported symbol. The reader validates target indices, relocation width, paired relocations, alignment, and target range before applying any relocation. The architecture encoder defines the meaning and width of each relocation kind.

The constants section stores typed object records with stable file-local IDs. References use those IDs, not process addresses, so constants can be reconstructed in two passes and can represent shared or circular Lisp object graphs. The symbols section stores package and symbol names as length-prefixed UTF-8 plus binding kind; symbol identity is resolved by the runtime only after structural validation. Stack-map entries are sorted by code-object ID and PC offset. Debug records map code-relative PCs to source content digest, normalized relative path, form span, variable names, and frame locations. Top-level records contain an ordered sequence of entry references and load-time execution flags.

`ncl-objfile` serializes and validates bytes only. It does not mmap, publish executable memory, intern symbols, or execute top-level records; those operations stay in the runtime boundary described by the [crate map](crates.md).

## Load and rejection order

FASL v1 has hard limits of 256 MiB per file, 1,048,576 records per section, and 1 MiB per string. Implementations may set lower limits but must not raise these values. Record and string lengths are also bounded by their containing section. The loader completes these stages before any executable mapping, symbol interning, or Lisp evaluation:

1. Read the file under a configured limit no greater than 256 MiB. Reject an oversized file before allocating section buffers.
2. Validate the fixed header length, magic, container version, semantic FASL version, architecture, pointer width, endianness, header size, and reserved-zero fields.
3. Validate the directory count and location, section IDs and order, alignment, checked offset-plus-length arithmetic, file bounds, non-overlap with the header and directory, zero padding, and absence of trailing bytes.
4. Check compiler identity, runtime ABI, policy fingerprint, and required-feature fingerprint against the running runtime. Reject version skew here.
5. Parse metadata, symbol, constant, code, relocation, stack-map, debug, and top-level records in that order. Bound every record count and string length by both the file size and v1 limits. Validate every file-local reference and every code-relative range.
6. Validate relocation kinds, widths, target indices, paired fixups, and destination ranges. Validate stack-map PCs and variable locations against their code objects. Unknown tags, flags, reserved bits, duplicate IDs, and non-canonical record ordering are invalid.
7. Only after the complete input passes, resolve symbol imports, materialize constants, allocate code objects, apply relocations, install metadata, synchronize caches, seal pages, and publish function entries using the [code-space publication order](code-space.md).
8. Execute top-level records in file order after every referenced entry is published. A top-level error propagates as a load error; effects from earlier records remain and are not rolled back.

Every malformed or unsupported wire value, incompatible version, target, policy, or feature set signals Lisp condition `invalid-fasl` with a reason identifying the failed check. File I/O failures remain file errors. Parsing must return a typed error rather than panic, wrap an offset, allocate from an unchecked length, or silently accept a trailing fragment.

This check order is the V6 verification-layer contract for FASL fuzzing. Mutators target each stage, including truncated headers, invalid ranges, arithmetic overflow, duplicate IDs, unknown relocation tags, invalid symbol encodings, oversized constants, and invalid stack-map PCs. A rejected file leaves package tables, function cells, code mappings, and top-level state unchanged.

A structurally valid FASL contains executable native code. Validation prevents malformed input from corrupting the loader; it does not sandbox intentionally malicious machine code. Load only FASLs from trusted producers.

## Compiler, image, and cache compatibility

FASL format version, container schema version, compiler version, runtime ABI version, and image format/runtime versions are independent compatibility dimensions checked by their respective readers.

- The FASL reader accepts semantic format 1 only. An incompatible compiler identity or runtime ABI is rejected; compatibility is exact for v1. A future format requires a new semantic version and an explicit reader policy.
- The compiler semantic version comes from the Cargo package version. Release packaging must derive its version from the same source or assert equality. The present `flake.nix`/`Cargo.toml` mismatch is a release blocker; do not write two independent version literals.
- Image saving and restoring use `NCLIMAGE` and its own format/runtime compatibility versions. FASL v1 does not replace `SAVE-LISP-AND-DIE` options `:executable`, `:toplevel`, and `:save-runtime-options`. Image restore validates architecture, image format, runtime ABI, and required features before rebuilding code. It must not trust feature strings that are parsed and then discarded.
- The FASL cache key is the tuple of source-content SHA-256, compiler identity, runtime ABI version, policy fingerprint, feature fingerprint, and target architecture. Policy or feature changes cannot reuse a prior entry. The cache location follows the XDG cache directory contract.

A version mismatch is a rejection, not a best-effort load. FASL and image readers never infer compatibility from the package version alone.

## Reproducible output

For the same source bytes, compile-time environment and results, compiler identity, target, policy, and features, the writer emits identical bytes. It uses fixed section order, stable object and symbol IDs, sorted imports and relocations, zero-filled reserved and padding bytes, and canonical integer encodings. It excludes timestamps, random IDs, absolute paths, memory addresses, and hash-table iteration order. Source paths in debug information are normalized relative paths.

Compile-time Lisp that intentionally observes time, randomness, or other changing external state can produce different semantics; byte-for-byte reproducibility does not override those semantics. Tests compare complete output bytes from repeated builds with identical recorded inputs, not only parsed structures.

## Ordered implementation slices

L1-3 is the code-space implementation lane. L3-13 is the native FASL and image lane; it begins after the L1-3 publication interface is frozen.

| Order | Commit unit | Primary paths | Completion evidence |
| --- | --- | --- | --- |
| L3-13.1 | Add the v1 metadata, canonical sections, native records, version checks, parser limits, and `invalid-fasl` error mapping in `ncl-objfile`. | `packages/objfile/src/fasl.rs`, `packages/objfile/src/fasl_relocation.rs`, `packages/objfile/src/error.rs` | Golden tests compare exact bytes. Unit tests reject each malformed field and section. V6 fuzzing finds no panic, out-of-bounds access, partial mutation, or unbounded allocation. |
| L3-13.2 | Compile source forms to machine code and ordered top-level records; LOAD validates, links, and publishes without invoking the compiler. | `packages/runtime/src/compile.rs`, `packages/runtime/src/load.rs`, `packages/runtime/src/lib.rs` | FASL round trip executes definitions and top-level effects. A test proves LOAD does not enter source compilation and succeeds when the original source file is absent. |
| L3-13.3 | Add image/runtime version metadata and the requested save options; make restore relink code and validate required features. | `packages/image/src/*`, `packages/runtime/src/*` | Save and restore in a fresh process preserves roots and callable code. Format, runtime ABI, target, and feature mismatch tests reject before publication. |
| L3-13.4 | Complete architecture-aware disassembly and source/debug maps for loaded code. | `packages/disasm/src/lib.rs`, `packages/image/src/*`, `packages/runtime/src/*` | Disassembly, PC-to-form lookup, variable locations, stack traces, and profiler maps refer to the same published code ranges. |

The L3-13 release checks are FASL byte round trip, LOAD without recompilation, malformed-input fuzzing, image round trip, version-skew rejection, and startup/size measurements for the selected targets. The cache key is implemented by the compiler-performance lane; its input contract is fixed above.
