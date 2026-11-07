# SBCL surface tables

Regenerate the six-column SBCL surface tables from the repository root with:

```sh
nix develop --command sbcl --script conformance/sbcl/extract-surface.lisp
```

The extractor requires SBCL 2.6.0. The flat tables are the macOS arm64 baseline. Linux x86-64 tables that differ from that baseline, plus tables present only on Linux, are stored in `linux-x86_64/`; byte-identical tables use the flat baseline. `conformance/ownership/check.py` selects a Linux override when one exists and otherwise uses the baseline.

The symbol tables begin with `package`, `symbol`, `present`, `external`, `kind`, and `lambda-list`. `present` means the symbol is interned in the package; `external` means the package exports it. Boolean fields use `yes` or `no`. Compound kinds use `+` in this order: `special-operator`, `macro`, `function`, `variable`, `constant`, `class`, `condition`, `type`, `other`. Function and macro lambda lists use lowercase Common Lisp printer output; `()` denotes an empty list, `unknown` denotes unavailable introspection, and `-` is used for other symbols. The internal reference table retains its three provenance columns after these fields.

## COMMON-LISP implementation matrix

`common-lisp.tsv` is the 978-symbol COMMON-LISP matrix as well as the SBCL surface table. The SBCL extractor writes its six base columns. Run the matrix generator after extraction to append `clhs_kinds`, `clhs_chapters`, `implementation_status`, `status_notes`, `test_refs`, and `ncl_kinds`; running the extractor again replaces the enriched table with the six-column surface data.

Build the release binary, generate the matrix, then check it against a fresh runtime probe:

```sh
cargo build --release --bin ncl
ncl=${CARGO_TARGET_DIR:-target}/release/ncl
python3 scripts/check_cl_matrix.py --ncl "$ncl" --write
python3 scripts/check_cl_matrix.py --ncl "$ncl"
```

The generator joins the six-column surface table to `conformance/sbcl/common-lisp-metadata.tsv` by package and symbol. Metadata kinds use `+` separators, CLHS chapter numbers and test references use `;` separators, and `-` means no reference. `ANSI:<name>` references must exist in `conformance/baselines/ansi-test-sbcl-2.6.0-macos-arm64.tsv`; these names were matched by exact symbol/test-name stems, and the baseline contains no test bodies to verify the match. Other references name repository test files.

`implementation_status` reports runtime binding availability for the metadata kinds. `implemented` requires the observable kind bindings and at least one test reference. `partial` records a missing or unavailable binding, a kind the probe cannot inspect, or a missing test reference. `missing` records a non-external symbol or a row with no available expected binding. `status_notes` identifies the reason, and `ncl_kinds` records the kinds observed in NCL. These checks establish symbol and namespace availability; they do not establish full ANSI behavior or prove that every referenced test exercises every listed kind.

The checker prints interned COMMON-LISP function and macro names whose `fboundp` is false. A generic call sweep is unsafe because calls can have side effects, stop, or terminate NCL. Quote `--probe-call 'SYMBOL=(...)'` to pass a direct call expression for an explicitly selected symbol; run it only when the supplied arguments are safe. The checker reports that other fbound functions were not call-probed, so a callback that fails with an unimplemented error may remain undetected.

All counts below exclude the header row.

| Package table | macOS arm64 rows | Linux x86-64 rows |
| --- | ---: | ---: |
| COMMON-LISP | 978 | 978 |
| SB-EXT | 237 | 269 |
| SB-THREAD | 83 | 83 |
| SB-ALIEN | 62 | 62 |
| SB-SYS | 111 | 111 |
| SB-MOP | 102 | 102 |
| SB-GRAY | 32 | 32 |
| SB-UNICODE | 47 | 47 |
| SB-DEBUG | 24 | 25 |
| **Public SB package total** | **698** | **731** |

| SBCL contrib | macOS arm64 status | macOS rows | Linux x86-64 status | Linux rows |
| --- | --- | ---: | --- | ---: |
| sb-posix | loaded | 547 | loaded | 549 |
| sb-bsd-sockets | loaded | 79 | loaded | 79 |
| sb-concurrency | loaded | 37 | loaded | 37 |
| sb-queue | loaded | 9 | loaded | 9 |
| sb-md5 | loaded | 16 | loaded | 16 |
| sb-rotate-byte | loaded | 1 | loaded | 1 |
| sb-cltl2 | loaded | 9 | loaded | 9 |
| sb-gmp | loaded | 39 | loaded | 39 |
| sb-mpfr | loaded | 130 | loaded | 130 |
| sb-simd | out-of-scope | 0 | loaded | 312 |
| sb-capstone | loaded | 121 | loaded | 121 |
| sb-aclrepl | loaded | 7 | loaded | 7 |
| sb-executable | loaded | 2 | loaded | 2 |
| sb-rt | loaded | 10 | loaded | 10 |
| sb-grovel | loaded | 1 | loaded | 1 |
| sb-perf | out-of-scope | 0 | loaded | 2 |
| sb-sprof | loaded | 16 | loaded | 16 |
| sb-cover | loaded | 13 | loaded | 13 |
| sb-introspect | loaded | 26 | loaded | 26 |
| sb-simple-streams | loaded | 106 | loaded | 106 |

`loaded` means SBCL's `require` returned successfully and the expected package was available for symbol extraction. The Linux run printed `WARNING: Capstone not loaded.` while `require` succeeded and SB-CAPSTONE's table was extracted with 121 external symbols. This extraction did not test whether NCL can load the native `libcapstone` at runtime.

| Additional system | macOS arm64 status | macOS rows | Linux x86-64 status | Linux rows |
| --- | --- | ---: | --- | ---: |
| ASDF | loaded | 230 | loaded | 230 |
| UIOP | loaded | 427 | loaded | 427 |

`internal-referenced.tsv` contains 58 references collected from CFFI, Alexandria, Babel, UIOP, and usocket sources. The source summary is CFFI 7, Alexandria 2, Babel 1, UIOP 36, and usocket 12. Its provenance columns identify the library, example source location, and whether the symbol was exported in SBCL 2.6.0. Of 58 referenced names, 57 occur in the SBCL 2.6.0 surface table: 29 are external and 28 are internal. `SB-C::SIMPLE-COMPILER-NOTE` does not occur in that table.
