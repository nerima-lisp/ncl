# SBCL surface tables

Regenerate the tables from the repository root with:

```sh
nix develop --command sbcl --script conformance/sbcl/extract-surface.lisp
```

The symbol tables begin with `package`, `symbol`, `present`, `external`, `kind`, and `lambda-list`. Boolean fields use `yes` or `no`. Compound kinds use `+` in this order: `special-operator`, `macro`, `function`, `variable`, `constant`, `class`, `condition`, `type`, `other`. Function and macro lambda lists use lowercase Common Lisp printer output; `()` denotes an empty list, `unknown` denotes unavailable introspection, and `-` is used for other symbols. The internal reference table retains its three provenance columns after these fields.

All counts below exclude the header row.

| Package table | Rows |
| --- | ---: |
| COMMON-LISP | 978 |
| SB-EXT | 237 |
| SB-THREAD | 83 |
| SB-ALIEN | 62 |
| SB-SYS | 111 |
| SB-MOP | 102 |
| SB-GRAY | 32 |
| SB-UNICODE | 47 |
| SB-DEBUG | 24 |

The eight SB public-package tables contain 698 rows combined, eight more than the 690-row D-13 baseline.

The 20 SBCL contrib entries are separate from the additional ASDF and UIOP entries. D-13 states that 20 contribs are in scope on macOS arm64; this extraction loaded 18 and marked `sb-simd` and `sb-perf` out of scope. Those excluded entries do not reconcile the 20-in-scope requirement, and the historical inventory has no two additional names to close that gap. D-13 schedules `sb-simd` and `sb-perf` for x86-64 Linux, but extraction on that platform has not run. No in-scope contrib failed. The `external-count` in `contribs.tsv` matches each loaded table's data row count.

| SBCL contrib | Status | Rows |
| --- | --- | ---: |
| sb-posix | loaded | 547 |
| sb-bsd-sockets | loaded | 79 |
| sb-concurrency | loaded | 37 |
| sb-queue | loaded | 9 |
| sb-md5 | loaded | 16 |
| sb-rotate-byte | loaded | 1 |
| sb-cltl2 | loaded | 9 |
| sb-gmp | loaded | 39 |
| sb-mpfr | loaded | 130 |
| sb-simd | out-of-scope | 0 |
| sb-capstone | loaded | 121 |
| sb-aclrepl | loaded | 7 |
| sb-executable | loaded | 2 |
| sb-rt | loaded | 10 |
| sb-grovel | loaded | 1 |
| sb-perf | out-of-scope | 0 |
| sb-sprof | loaded | 16 |
| sb-cover | loaded | 13 |
| sb-introspect | loaded | 26 |
| sb-simple-streams | loaded | 106 |

| Additional system | Status | Rows |
| --- | --- | ---: |
| ASDF | loaded | 230 |
| UIOP | loaded | 427 |

`internal-referenced.tsv` contains 58 references collected from CFFI, Alexandria, Babel, UIOP, and usocket sources. The source summary is CFFI 7, Alexandria 2, Babel 1, UIOP 36, and usocket 12; the other 10 scanned library sources contributed none. Its provenance columns identify the library, example source location, and whether the symbol was exported in SBCL 2.6.0. Of 58 references, 57 were present: 29 external and 28 internal. `SB-C::SIMPLE-COMPILER-NOTE` was absent.
