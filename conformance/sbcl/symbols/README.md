# SBCL surface tables

Regenerate the tables from the repository root with:

```sh
nix develop --command sbcl --script conformance/sbcl/extract-surface.lisp
```

The extractor requires SBCL 2.6.0. The flat tables are the macOS arm64 baseline. Linux x86-64 tables that differ from that baseline, plus tables present only on Linux, are stored in `linux-x86_64/`; byte-identical tables use the flat baseline. The ownership checker selects a Linux override when one exists and otherwise uses the baseline.

The symbol tables begin with `package`, `symbol`, `present`, `external`, `kind`, and `lambda-list`. Boolean fields use `yes` or `no`. Compound kinds use `+` in this order: `special-operator`, `macro`, `function`, `variable`, `constant`, `class`, `condition`, `type`, `other`. Function and macro lambda lists use lowercase Common Lisp printer output; `()` denotes an empty list, `unknown` denotes unavailable introspection, and `-` is used for other symbols. The internal reference table retains its three provenance columns after these fields.

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

The Linux public-package increase is 32 symbols in SB-EXT and one in SB-DEBUG; the other six SB package counts match macOS arm64. At revision `57cc06ab`, this README reported 236, 82, 61, 110, 101, 31, 46, and 23 rows for the eight SB public packages (690 total). The current macOS TSVs and the TSVs under `conformance/sbcl/symbols/` at that revision each contain 237, 83, 62, 111, 102, 32, 47, and 24 rows (698 total), one more row per package. Their symbol-name sets are identical, and the repository contains no 690-symbol membership list. The 690 figure is a stale README count; the evidence identifies no eight missing symbols.

The 20 SBCL contrib entries are separate from ASDF and UIOP. D-13 states 20 contribs for macOS arm64; 18 loaded there, while `sb-simd` and `sb-perf` were out of scope. The extractor's 20-entry `*sbcl-contribs*` list in `conformance/sbcl/extract-surface.lisp` contains no additional contrib entries to account for the difference. On x86-64 Linux, all 20 contribs loaded, including `sb-simd` and `sb-perf`. No in-scope contrib failed. The platform-specific `external-count` in `contribs.tsv` matches the resolved table's data row count.

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

`internal-referenced.tsv` contains 58 references collected from CFFI, Alexandria, Babel, UIOP, and usocket sources. The source summary is CFFI 7, Alexandria 2, Babel 1, UIOP 36, and usocket 12; the other 10 scanned library sources contributed none. Its provenance columns identify the library, example source location, and whether the symbol was exported in SBCL 2.6.0. Of 58 references, 57 were present: 29 external and 28 internal. `SB-C::SIMPLE-COMPILER-NOTE` was absent.
