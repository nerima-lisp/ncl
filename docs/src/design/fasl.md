# Native FASL v1

## Decision

FASL v1 is a relocatable native compilation unit. It stores machine code, typed constants, symbol imports, relocations, stack maps, debug records, and ordered top-level execution records. Loading a FASL validates and links those records; it does not read or recompile source forms.

The file is target-specific. It is accepted only when its architecture, pointer width, byte order, FASL format, compiler identity, runtime ABI, policy fingerprint, and feature fingerprint match the running compiler/runtime. A well-formed but incompatible file is rejected before any code is allocated or published.

## Current implementation

The current implementation does not satisfy this contract:

- `packages/objfile/src/fasl.rs` lines 65–79 writes container version 2; lines 217–223 accepts versions 1 and 2. The version-2 header contains architecture, pointer width, endianness, feature bits, and fixed ranges for code, relocations, constants, symbols, stack maps, and debug data, but no compiler or policy fingerprint.
- `packages/runtime/src/compile.rs` lines 279–313 runs source through the compile-file top-level pipeline, then writes empty code, relocation, constant, symbol, and stack-map sections. Its debug section contains an `NCLRTFASL` payload with UTF-8 source and an FNV-1a hash.
- `packages/runtime/src/load.rs` lines 102–121 decodes that source and sends it back through the normal source-form execution path. This recompiles at LOAD and violates AC-4's requirement that FASL loading not recompile source.
- `packages/objfile/src/fasl.rs` lines 139–140 exposes `FaslReader::read` as `Result<Fasl, ObjectError>`, and `packages/runtime/src/load.rs` lines 61–63 converts `ObjectError` into `FileLoadError`; neither provides typed FASL rejection reasons or maps them to a Lisp condition. The `NCL-EXT:INVALID-FASL` package, superclass, and `REASON` slot contract is now specified in [conditions.md](conditions.md) lines 42 and 49, but the current reader/runtime still lack typed rejection reasons and condition mapping.
- Images are a separate `NCLIMAGE` format. `packages/image/src/format.rs` lines 12–35 defines version 2 with object records, roots, code blobs, and feature strings. `packages/image/README.md` lines 141–145 says image load parses and discards those feature strings. The image version is not the FASL version.
- The recorded package versions disagree: `flake.nix` line 50 declares `0.1.0` while `Cargo.toml` line 85 declares `0.1.1`.

## Versioned wire format

The proposed native format uses semantic FASL version 1 inside `ncl-objfile` container schema version 3. Runtime `LOAD` accepts this native format and rejects legacy container versions 1 and 2 that encode source payloads; ordinary Lisp source remains loadable through the source-file path. Keep the container-schema version distinct from the semantic FASL format version.

The fixed 128-byte little-endian header has these fields:
| Offset | Field | Encoding | Contract |
| ---: | --- | --- | --- |
| 0 | magic | 8 bytes | Exactly `NCLFASL\0`. |
| 8 | container version | `u16` | Exactly 3 for the v1 native container. |
| 10 | architecture | `u8` | Existing `ncl-objfile` architecture tag: x86-64 or AArch64. |
| 11 | pointer width | `u8` | 8 bytes. |
| 12 | endianness | `u8` | 1 means little-endian. |
| 13 | header size | `u8` | Exactly 128. |
| 14 | reserved | `u16` | Zero. |
| 16 | semantic FASL version | `u32` | Exactly 1. |
| 20 | runtime ABI version | `u32` | Exact runtime code/object ABI revision. |
| 24 | compiler identity | 32 bytes | SHA-256 of the semantic version, reproducible build identity, and code-generation ABI revision. |
| 56 | policy fingerprint | 32 bytes | SHA-256 of the canonical policy records in metadata. |
| 88 | feature fingerprint | 32 bytes | SHA-256 of the canonical required-feature records in metadata. |
| 120 | section count | `u32` | Exactly 8. |
| 124 | directory offset | `u32` | Exactly 128. |

The directory occupies bytes 128 through 319 and contains eight 24-byte entries in ascending section-ID order: metadata, symbols, constants, code, relocations, stack maps, debug records, and top-level records. Each entry stores a `u16` section ID, zero `u16` flags, `u32` alignment, `u64` file offset, and `u64` byte length. Alignment is fixed by section ID: metadata 4, symbols 4, constants 8, code 16, relocations 4, stack maps 4, debug records 4, and top-level records 4 bytes. Starting with cursor 320, each entry's offset is exactly `align_up(cursor, alignment)`; the cursor then becomes `offset + byte_length`, including for zero-length sections. Padding between the cursor and aligned offset is zero, and the file ends exactly at the final cursor, including any alignment padding required by the final entry, with no other trailing bytes. This fixes empty-section offsets as well as non-empty offsets. No host pointer or process address is serialized.
Each 24-byte directory entry has this exact layout:

| Entry offset | Field | Encoding | Contract |
| ---: | --- | --- | --- |
| 0 | section ID | `u16` | 1 metadata, 2 symbols, 3 constants, 4 code, 5 relocations, 6 stack maps, 7 debug records, 8 top-level records. |
| 2 | flags | `u16` | Zero in v1. |
| 4 | alignment | `u32` | Power of two from 1 through 4096. |
| 8 | file offset | `u64` | Start of this section, relative to file byte 0. |
| 16 | byte length | `u64` | Exact serialized section length; padding follows the section. |

Every section begins at an offset satisfying its directory alignment. Directory entries are in ascending section-ID order.

The metadata section uses this exact field order: `metadata_schema:u32=1`; `semantic_version_length:u32` and that many UTF-8 bytes; `compiler_build_identity:[u8;32]`; `codegen_abi_revision:u32`; `target_triple_length:u32` and that many UTF-8 bytes; `source_sha256:[u8;32]`; `policy_count:u32`; policy records; `feature_count:u32`; feature records. It ends immediately after the last feature record.

Policy records are four bytes: `policy_id:u16`, `value:u8`, `reserved:u8=0`. IDs are sorted and unique: 1 is safety, 2 is speed, and 3 is debug; each value is 0 through 3. Feature records are `name_length:u32`, UTF-8 feature name bytes, `version_length:u32`, UTF-8 version bytes. Feature records are sorted and unique by the byte sequence `name || 0x00 || version`. Names and versions are nonempty, contain no NUL, and use canonical UTF-8.

The reader recomputes each fingerprint from metadata before comparing it with the header or current runtime. Compiler identity is SHA-256 over the domain bytes `NCL-FASL-COMPILER-V1 || 0x00`, the length-prefixed semantic version, the 32-byte build identity, and the little-endian code-generation ABI revision. Policy fingerprint is SHA-256 over `NCL-FASL-POLICY-V1 || 0x00`, the policy count, and the ordered policy records. Feature fingerprint is SHA-256 over `NCL-FASL-FEATURES-V1 || 0x00`, the feature count, and the ordered length-prefixed feature records. The `ncl-objfile` crate owns the dependency-free SHA-256 implementation and digest API; the runtime reuses it for content hashes, fingerprints, and cache keys.

The compiler build identity is the SHA-256 digest of a canonical build manifest. Its byte stream is `NCL-FASL-BUILD-V1 || 0x00`, `file_count:u32`, sorted unique file records, length-prefixed rustc release, rustc commit, target triple, and Cargo release, `feature_count:u32` and sorted crate-qualified feature names, then `codegen_setting_count:u32` and sorted code-generation setting records, in that order. Each file record is a UTF-8 repository-relative path prefixed by its little-endian `u32` byte length, followed by the file's 32-byte SHA-256 digest. The file set is `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `flake.nix`, `flake.lock`, all tracked files under `.cargo/`, `packages/`, and `src/`, and the root `build.rs` if present, including per-crate manifests and build scripts, sorted by path bytes using `/` separators. Every string is encoded as a little-endian `u32` byte length followed by UTF-8 bytes. Feature names use `crate-name/feature-name`, are sorted by byte sequence, and are unique. Each setting record is a length-prefixed key/value pair; setting keys are sorted by byte sequence and unique. Settings include every effective rustc `--cfg` and `-C` option that can affect emitted code or FASL metadata, with its exact value or an explicit `unset` value; omit only linker-only arguments that cannot affect emitted code. The repository's `[profile.release]` settings are `opt-level=3`, `lto=thin`, `codegen-units=1`, `panic=abort`, and `strip=symbols` (`Cargo.toml` lines 119–124), and their effective values appear in the setting records. Link-only arguments, including absolute Nix store paths passed through `-C link-arg` or `NIX_LDFLAGS`, are excluded because they do not change generated FASL code. The manifest excludes timestamps, absolute paths, branch names, `.git/`, `.jj/`, and generated build output. Rustc release and commit come from `rustc -Vv`; Cargo release comes from `cargo -V`. Thus a tracked source/configuration, toolchain, enabled-feature, or code-generation-option change changes the build identity, while repeated clean builds from the same recorded inputs produce the same identity.
## Canonical section and record layouts

Every `u16`, `u32`, and `u64` is little-endian. Signed integers use two's-complement. Strings are a `u32` byte length followed by canonical UTF-8 bytes. Records have no implicit Rust padding; the padding specified below is present and must be zero.

### Symbols

The section starts with `symbol_count:u32`, followed by that many records. Each record is `symbol_id:u32`, `binding_kind:u32`, `flags:u32=0`, `package_length:u32`, package UTF-8 bytes, `name_length:u32`, name UTF-8 bytes, then zero padding to a four-byte boundary. IDs are dense from zero and records are sorted by `(binding_kind, package bytes, name bytes)`. Binding kind 1 denotes a Lisp symbol; kind 2 denotes an imported native function. Unknown kinds, duplicate names for one kind, nonzero flags, NUL bytes, and non-canonical UTF-8 are rejected. These records contain names and IDs, never symbol or function pointers.

### Constants and code objects

The constants section starts with `code_object_count:u32`. It contains one group per code object, in ascending code-object ID order. Each group is `code_object_id:u32`, `constant_count:u32`, then one record for every zero-based constant-vector index. A constant record is `kind:u16`, `flags:u16=0`, `payload_length:u32`, payload bytes, and zero padding to a four-byte boundary. The kind tags and payloads are fixed:

| Tag | Constant kind | Payload |
| ---: | --- | --- |
| 0 | NIL | Empty. |
| 1 | T | Empty. |
| 2 | fixnum | One signed `i64`, within the target runtime's fixnum range. |
| 3 | character | One Unicode scalar as `u32`. |
| 4 | single-float | IEEE-754 binary32 bits as `u32`. |
| 5 | double-float | IEEE-754 binary64 bits as `u64`. |
| 6 | string | UTF-8 bytes. |
| 7 | symbol reference | One `u32` symbol ID. |
| 8 | cons | Two `u32` constant indices: car, then cdr. |
| 9 | simple vector | `element_count:u32`, then that many `u32` constant indices. |
| 10 | bignum | `sign:u8` (0 positive, 1 negative), three zero bytes, `limb_count:u32`, then little-endian `u32` magnitude limbs least-significant first. |
| 11 | ratio | Two `u32` constant indices: numerator, then denominator. |
| 12 | complex | Two `u32` constant indices: real part, then imaginary part. |

Constant indices are zero-based within the owning code object's vector. Compound constant references may form shared or circular graphs and must stay in that vector. Bignums have no leading zero limb and no negative zero. A reader rejects an out-of-range index, an unknown tag, malformed payload length, nonzero padding, or a fixnum outside the runtime range.

The code section starts with `code_object_count:u32`, followed by code-object records in ascending dense ID order. IDs are assigned in top-level source order, with nested compiled objects in lexical depth-first order. Each fixed 28-byte record has this field order: `code_object_id:u32`, `entry_offset:u32`, `code_length:u32`, `frame_words:u16`, `flags:u16=0`, `constant_count:u32`, `stack_map_count:u32`, `debug_record_count:u32`. It is followed by exactly `code_length` machine-code bytes and zero padding to an eight-byte boundary. Code length is nonzero, entry offset is less than code length, and the three counts match the corresponding sections. The compiler assigns constant-vector indices in deterministic first-emission order; the writer preserves those indices and does not sort constants by value.

Serialized code contains no absolute process address, Lisp `Word`, or heap-object address. A function object's `CODE` handle owns the GC-traced constant vector in its `CONSTANTS` slot; machine code obtains that vector through the runtime's code-object ABI and uses its zero-based `u32` index. The collector may update a vector slot without rewriting executable bytes. Relocations may write code addresses or native import addresses before publication, but never Lisp heap addresses.

### Relocations

The relocation section starts with `relocation_count:u32`, then fixed 32-byte records sorted by `(code_object_id, write_offset)`: `code_object_id:u32`, `write_offset:u32`, `kind:u32`, `target_kind:u32`, `target_index:u32`, `addend:i64`, `pair_id:u32`. Target kinds are 1 for a code object, 2 for an imported native function, and 3 for a constant-vector slot in the owning code object. Kind tags and patch widths are frozen:

| Tag | Kind | Width | Target and rule |
| ---: | --- | ---: | --- |
| 0 | `Abs64` | 8 | Absolute code-object or native-function address, resolved at load. Never a Lisp heap address. |
| 1 | `PcRel32` | 4 | x86-64 signed PC-relative displacement to a code object. |
| 2 | `Plt32` | 4 | x86-64 signed PLT displacement to an imported native function. |
| 3 | `Branch26` | 4 | AArch64 scaled branch displacement to a code object or native function; the site is four-byte aligned and the signed 26-bit word displacement must fit. |
| 4 | `Adrp21` | 4 | First half of an AArch64 page-address pair; its signed 21-bit page displacement must fit. |
| 5 | `Add12` | 4 | Second half of the corresponding AArch64 page-address pair; patches the low 12 target-address bits. |
| 6 | `CondBranch19` | 4 | AArch64 scaled conditional-branch displacement to a code object; the site is four-byte aligned and the signed 19-bit word displacement must fit. |
| 7 | `CodeEntry` | 8 | Absolute entry address of the referenced code object. |
| 8 | `ExternalSymbol` | 8 | Absolute address of the referenced imported native function. |
| 9 | `ConstantIndex32` | 4 | Unsigned zero-based index into this code object's constant vector; target kind must be 3 and the index must be less than `constant_count`. |

Tag values 0 through 8 match the existing `ncl-objfile` relocation tags. `ConstantIndex32` at tag 9 is the sole FASL v1 addition to `RelocKind`; it adds a non-pointer constant-vector index. It writes `target_index` as a little-endian `u32`, requires `addend=0`, and is the only relocation allowed to target kind 3. Unknown tags and target-kind combinations are invalid. A relocation site and its full patch width must be inside its code object, sites must not overlap, and code-relative targets must be within their target object.

`A` denotes the record's signed `i64` addend. For target kind 1, `target_index` selects a code object. `Abs64` uses that object's base address as `S`; `CodeEntry` and the branch/address-pair kinds use its entry address. The resulting code-relative target must remain inside that object. For target kind 2, `target_index` selects an imported native function and `S` is its resolved address. Target kind 3 is valid only for `ConstantIndex32`, whose value is the constant-vector index rather than an address and whose addend is zero. `Abs64`, `CodeEntry`, and `ExternalSymbol` write the little-endian `u64` value `S + A`; they write a code-object base address, a code object's entry address, or an imported native function address, never a Lisp heap address. `PcRel32` and `Plt32` write the signed little-endian `i32` value `S + A - P`, where `P` is the address of the first byte of the four-byte patch field. `Branch26` writes `((S + A - P) >> 2)` into instruction bits 25:0; `CondBranch19` writes the same scaled displacement into bits 23:5. Both require four-byte aligned sites and an exactly representable signed displacement. `Adrp21` writes `page(S + A) - page(P)` in 4 KiB pages, with `immlo` in bits 30:29 and `immhi` in bits 23:5. Its paired `Add12` writes `(S + A) & 0xfff` in bits 21:10 with the shift bit clear. An unpaired relocation has `pair_id=0`. Every nonzero `pair_id` is unique within the relocation section and occurs exactly twice in the same code object: `Adrp21` at offset N followed by `Add12` at N+4. Both records have the same target kind, target index, and addend. Both immediate fields are zero in the serialized instruction words; all other opcode bits are preserved. A pair ID used once or more than twice, reversed or non-adjacent halves, mismatched targets, and out-of-range page deltas reject the FASL. The loader applies every relocation while the code remains writable and unpublished.

### Stack maps

The section starts with `code_object_count:u32`. Each group is `code_object_id:u32`, `entry_count:u32`, then that many existing `SafepointMap` entries. An entry reuses the current 16-byte little-endian header exactly: `pc_offset:u32`, `frame_words:u16`, `slot_words:u16`, `word_slot_count:u16`, `register_mask:u16`, `map_flags:u32`. It is followed by `ceil(slot_words/8)` bitmap bytes and one `u16` register ID for each set bit of `register_mask`, in increasing bit order. This is the representation consumed by `ncl-sys::SafepointMap::decode(bytes, count)`; FASL does not define a second safepoint encoding. Groups are sorted by code-object ID and entries by strictly increasing PC offset. The reader checks bitmap length, unused high bits, register IDs, frame and slot counts, and that every PC and variable location is inside its code object and frame.

### Debug and top-level records

The debug section starts with `file_count:u32`, then file records `file_id:u32`, `path_length:u32`, normalized relative-path UTF-8 bytes, and zero padding to four bytes. File IDs are dense from zero and files are sorted by path bytes. It then has `record_count:u32`, followed by records sorted by `(code_object_id, pc_start)`. Each debug record has nine `u32` fields: code-object ID, PC start, PC end, file ID, one-based line, one-based column, source-form byte start, source-form byte end, and variable count. Each variable has `name_length:u32`, UTF-8 name bytes, `location_kind:u32` (0 stack slot, 1 saved register), and `location_index:u32`, then zero padding to four bytes. Variables are sorted by name bytes with no duplicate names per record. Paths cannot be absolute or contain a parent-directory component. PC ranges are nonempty and within the code object; source spans, file IDs, frame slots, and register IDs must validate.

The top-level section starts with `record_count:u32`, followed by fixed 16-byte records: `ordinal:u32`, `code_object_id:u32`, `entry_offset:u32`, `flags:u32`. Ordinals are dense from zero and preserve source order. Flag bit 0 means execute this entry during LOAD; bit 1 means the entry installs a function definition and requires bit 0. All other bits are zero. Each entry offset must match a validated code-object entry. LOAD runs flagged entries only after validation and publication, in ordinal order.

Golden-byte tests use fixed metadata, target, code, imports, and policy/feature fingerprints, and compare the complete serialized file including zero padding. Fixtures cover x86-64 and AArch64, every relocation tag, both halves of the AArch64 pair, and the 16-byte safepoint header. The writer must produce identical bytes for identical recorded inputs; comparing parsed structures alone is insufficient.

`ncl-objfile` serializes and validates bytes only. It does not mmap, publish executable memory, intern symbols, or execute top-level records; those operations stay in the runtime boundary described by the [crate map](crates.md).

## Load and rejection order

FASL v1 has hard limits of 256 MiB per file, 1,048,576 records per section, and 1 MiB per string. Implementations may set lower limits but must not raise these values. Record and string lengths are also bounded by their containing section. The loader completes these stages before any executable mapping, symbol interning, or Lisp evaluation:
1. Read the file under a configured limit no greater than 256 MiB. Reject an oversized file before allocating section buffers.
2. Validate the fixed header length, magic, container version, semantic FASL version, architecture, pointer width, endianness, header size, reserved-zero fields, section count, and directory offset.
3. Validate the directory's IDs, order, alignments, checked offset-plus-length arithmetic, file bounds, non-overlap with the header/directory/other sections, zero padding, and absence of trailing bytes.
4. Parse only the bounded metadata section. Validate its schema version, canonical strings, policy IDs and values, sorted feature records, and exact end position.
5. Recompute compiler identity, policy fingerprint, and feature fingerprint from the canonical metadata records. Compare each recomputed digest with the header, then compare compiler identity, runtime ABI, target, policy, and required features with the running runtime. A mismatch is rejected before any runtime object or mapping is created.
6. Parse all remaining sections in ID order. Validate exact record lengths, padding, tags, dense and unique IDs, references, code ranges, relocation sites and pairs, safepoint entries, debug locations, and top-level order. Decode each stack-map group with `ncl-sys::SafepointMap::decode` after checking the canonical increasing-PC wire order.
7. Prepare the complete load off-table. Resolve imports by read-only lookup. Build new symbol objects and replacement package-table snapshots off-table. Construct and root the constant graph in staging storage. Allocate writable, unpublished code; apply every relocation; install metadata; seal the code; then synchronize caches after the final protection transition. Perform every fallible allocation and validation before changing a visible package-table root. If preparation fails, discard unpublished code and staging roots; visible package and function bindings remain unchanged.
8. Commit with only non-failing publication operations. Release-publish the prepared code handles, acquire writer locks for affected packages in canonical package-name order, install staged function-cell values, and swap the prebuilt package-table roots before releasing the locks. Package readers use the same locks and cannot observe an intermediate set of roots. New symbol objects already contain their prepared function entries when the new tables become visible. A rejected file or preparation allocation failure occurs before this commit and leaves the prior roots and visible bindings intact.
9. Execute flagged top-level records in file order after commit. A Lisp condition raised by a valid top-level form is an ordinary LOAD failure; effects of earlier forms remain and are not rolled back. The loader does not report such an execution failure as `invalid-fasl`.

`ncl-objfile` returns a typed `FaslError` carrying a `FaslRejectReason` enum and optional byte offset. The enum has stable variants for `BadMagic`, `UnsupportedVersion`, `TargetMismatch`, `RuntimeAbiMismatch`, `CompilerIdentityMismatch`, `PolicyFingerprintMismatch`, `FeatureFingerprintMismatch`, `MalformedDirectory`, `ResourceLimitExceeded`, `MalformedMetadata`, `InvalidReference`, `InvalidCodeObject`, `InvalidRelocation`, `InvalidSafepointMap`, `InvalidDebugRecord`, and `InvalidTopLevelRecord`. It does not classify failures from display strings. The runtime maps each variant to the corresponding stable keyword in the table below and signals `NCL-EXT:INVALID-FASL`, whose superclass is `COMMON-LISP:ERROR`. Its required `REASON` slot has initarg `:reason`, reader `invalid-fasl-reason`, type `KEYWORD`, and contains the mapped keyword, as specified in [conditions.md](conditions.md) lines 42 and 49. The condition is already registered at `packages/conditions/src/symbols/ncl_ext.rs` lines 72–76 with the declared superclass at `packages/conditions/src/hierarchy.rs` line 60; l02d's condition contract is complete, while the runtime slot initialization and typed rejection mapping remain to be implemented.

| `FaslRejectReason` | `NCL-EXT:INVALID-FASL` `REASON` |
| --- | --- |
| `BadMagic` | `:BAD-MAGIC` |
| `UnsupportedVersion` | `:UNSUPPORTED-VERSION` |
| `TargetMismatch` | `:TARGET-MISMATCH` |
| `RuntimeAbiMismatch` | `:RUNTIME-ABI-MISMATCH` |
| `CompilerIdentityMismatch` | `:COMPILER-MISMATCH` |
| `PolicyFingerprintMismatch` | `:POLICY-MISMATCH` |
| `FeatureFingerprintMismatch` | `:FEATURE-MISMATCH` |
| `MalformedDirectory` | `:INVALID-SECTION` |
| `ResourceLimitExceeded` | `:RESOURCE-LIMIT` |
| `MalformedMetadata` | `:INVALID-METADATA` |
| `InvalidReference` | `:INVALID-REFERENCE` |
| `InvalidCodeObject` | `:INVALID-CODE-OBJECT` |
| `InvalidRelocation` | `:INVALID-RELOCATION` |
| `InvalidSafepointMap` | `:INVALID-STACK-MAP` |
| `InvalidDebugRecord` | `:INVALID-DEBUG-RECORD` |
| `InvalidTopLevelRecord` | `:INVALID-TOPLEVEL-RECORD` |

File I/O failures remain `FILE-ERROR`. A malformed or incompatible input never panics, wraps an offset, allocates from an unchecked length, mutates a package table, or silently accepts trailing bytes. The V6 fuzz contract targets each rejection stage, including truncated headers, invalid ranges, arithmetic overflow, bad fingerprints, duplicate IDs, unknown relocation tags, invalid symbol encodings, oversized constants, and malformed stack maps.
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

This is the canonical L3-13 ownership table. The primary path sets are disjoint; paths marked `(new)` are planned files. The current `packages/objfile/src/fasl.rs` is 383 lines, so L3-13.1 keeps it as a facade and splits section codecs and validation into new modules to stay below the 500-line file limit.

| Order | Commit unit | Primary paths | Completion evidence |
| --- | --- | --- | --- |
| L3-13.1 | Add v1 metadata, records, version checks, parser limits, and typed rejection mapping in `ncl-objfile`. Keep `fasl.rs` as a facade below 500 lines; put each section codec in a focused module. | `packages/objfile/src/fasl.rs`, `packages/objfile/src/fasl_header.rs` (new), `packages/objfile/src/fasl_directory.rs` (new), `packages/objfile/src/fasl_metadata.rs` (new), `packages/objfile/src/fasl_records.rs` (new), `packages/objfile/src/fasl_symbols.rs` (new), `packages/objfile/src/fasl_constants.rs` (new), `packages/objfile/src/fasl_code.rs` (new), `packages/objfile/src/fasl_stack_map.rs` (new), `packages/objfile/src/fasl_debug.rs` (new), `packages/objfile/src/fasl_top_level.rs` (new), `packages/objfile/src/fasl_validate.rs` (new), `packages/objfile/src/fasl_relocation.rs`, `packages/objfile/src/error.rs` | Golden tests compare complete bytes, including padding. Unit tests reject malformed fields and sections. V6 fuzzing finds no panic, out-of-bounds access, partial mutation, or unbounded allocation. |
| L3-13.2 | Compile source forms to machine code and ordered top-level records; LOAD validates, stages, and publishes without invoking the compiler. | `packages/runtime/src/compile.rs`, `packages/runtime/src/load.rs`, `packages/runtime/src/lib.rs`, `packages/runtime/src/fasl_publish.rs` (new) | FASL round trip executes definitions and top-level effects. A test proves LOAD does not enter source compilation and succeeds when the original source file is absent. Rejected or allocation-failed preparation preserves visible package bindings. |
| L3-13.3 | Add image/runtime version metadata and save options; make restore relink code and validate required features. | `packages/image/src/code.rs`, `packages/image/src/error.rs`, `packages/image/src/format.rs`, `packages/image/src/lib.rs`, `packages/image/src/record.rs`, `packages/image/src/record_types.rs`, `packages/image/src/register.rs`, `packages/image/src/restore.rs`, `packages/image/src/restore_fill.rs`, `packages/image/src/save.rs`, `packages/image/src/tests_format.rs`, `packages/image/src/tests_restore_fill.rs`, `packages/image/src/tests_save.rs`, `packages/runtime/src/image.rs` (new) | Save and restore in a fresh process preserves roots and callable code. Format, runtime ABI, target, and feature mismatch tests reject before publication. |
| L3-13.4 | Complete architecture-aware disassembly and source/debug maps for loaded code. | `packages/disasm/src/aarch64.inc`, `packages/disasm/src/aarch64.rs`, `packages/disasm/src/lib.rs`, `packages/disasm/src/x86_64.inc`, `packages/disasm/src/x86_64.rs`, `packages/profiler/src/lib.rs`, `packages/runtime/src/fasl_debug.rs` (new) | Disassembly, PC-to-form lookup, variable locations, stack traces, and profiler maps refer to the same published code ranges. |

The L3-13 release checks are FASL byte round trip, LOAD without recompilation, malformed-input fuzzing, image round trip, version-skew rejection, and startup/size measurements for the selected targets. The cache key is implemented by the compiler-performance lane; its input contract is fixed above.
