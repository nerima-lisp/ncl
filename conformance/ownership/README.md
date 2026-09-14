# Symbol ownership

## `symbols.tsv`

Each row assigns one source surface symbol to one implementation crate. `kind` is the `+`-joined set of positive SBCL extraction flags; `other` covers contrib/internal entries without an ANSI kind. `phase` is 1 for lane implementation, 2 for runtime integration, and 3 for contrib/internal compatibility. `direct-expansion` is `yes` only for the primitive set in `docs/src/design/compiler-pipeline.md`.

The checker compares `(package, symbol)` against every symbol row in `conformance/sbcl/symbols/`, including contrib tables and `internal-referenced.tsv`; `contribs.tsv` is metadata and is not itself a symbol surface.

## Assignment rules

- Core special operators, declarations, and front-end syntax go to `ncl-compiler-front`.
- Macro expansion, standard macros, and SETF expansion go to `ncl-lib-macros`; LOOP and FORMAT have dedicated crates.
- Numeric, sequence/list, string/character, array/hash, stream, pathname, package/symbol, conditions, CLOS, and type surfaces follow the lane rules in the launch contract.
- Evaluation, compilation, loading, feature and implementation-environment functions go to Phase 2 `ncl-runtime`.
- `sb-*` packages use their dedicated crate mapping; unclassified `sb-ext` and `sb-sys` entries retain an explicit note.
- Contrib and internal references use Phase 3 reservation crates named `ncl-contrib-<name>`.

## Counts by crate and phase

| crate | phase | count |
| --- | ---: | ---: |
| `ncl-clos` | 1 | 136 |
| `ncl-compiler-front` | 1 | 42 |
| `ncl-conditions` | 1 | 65 |
| `ncl-contrib-alexandria` | 3 | 2 |
| `ncl-contrib-asdf` | 3 | 230 |
| `ncl-contrib-babel` | 3 | 1 |
| `ncl-contrib-cffi` | 3 | 7 |
| `ncl-contrib-sb-aclrepl` | 3 | 7 |
| `ncl-contrib-sb-bsd-sockets` | 3 | 79 |
| `ncl-contrib-sb-capstone` | 3 | 121 |
| `ncl-contrib-sb-cltl2` | 3 | 9 |
| `ncl-contrib-sb-concurrency` | 3 | 37 |
| `ncl-contrib-sb-cover` | 3 | 13 |
| `ncl-contrib-sb-executable` | 3 | 2 |
| `ncl-contrib-sb-gmp` | 3 | 39 |
| `ncl-contrib-sb-grovel` | 3 | 1 |
| `ncl-contrib-sb-introspect` | 3 | 26 |
| `ncl-contrib-sb-md5` | 3 | 16 |
| `ncl-contrib-sb-mpfr` | 3 | 130 |
| `ncl-contrib-sb-posix` | 3 | 547 |
| `ncl-contrib-sb-queue` | 3 | 9 |
| `ncl-contrib-sb-rotate-byte` | 3 | 1 |
| `ncl-contrib-sb-rt` | 3 | 10 |
| `ncl-contrib-sb-simple-streams` | 3 | 106 |
| `ncl-contrib-sb-sprof` | 3 | 16 |
| `ncl-contrib-uiop` | 3 | 463 |
| `ncl-contrib-usocket` | 3 | 12 |
| `ncl-ffi` | 1 | 100 |
| `ncl-image` | 1 | 4 |
| `ncl-lib-format` | 1 | 1 |
| `ncl-lib-hash-arrays` | 1 | 44 |
| `ncl-lib-macros` | 1 | 81 |
| `ncl-lib-numbers` | 1 | 146 |
| `ncl-lib-packages` | 1 | 42 |
| `ncl-lib-pathnames` | 1 | 39 |
| `ncl-lib-sequences` | 1 | 90 |
| `ncl-lib-streams` | 1 | 93 |
| `ncl-lib-strings` | 1 | 101 |
| `ncl-object` | 1 | 14 |
| `ncl-runtime` | 2 | 575 |
| `ncl-threads` | 1 | 91 |
| `ncl-types` | 1 | 12 |

## Counts by kind

| kind | count |
| --- | ---: |
| `class` | 186 |
| `class+function` | 19 |
| `condition+class` | 123 |
| `condition+class+function` | 10 |
| `constant` | 502 |
| `constant+class` | 2 |
| `function` | 2041 |
| `macro` | 194 |
| `macro+class` | 1 |
| `macro+special-operator` | 1 |
| `other` | 217 |
| `special-operator` | 27 |
| `special-operator+class` | 3 |
| `type` | 20 |
| `variable` | 209 |
| `variable+function` | 5 |

## Judgement notes

3295 rows carry a reason in `notes`. Representative cases:
- `ASDF/INTERFACE:*ASDF-VERBOSE*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*CENTRAL-REGISTRY*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*COMPILE-FILE-FAILURE-BEHAVIOUR*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*COMPILE-FILE-WARNINGS-BEHAVIOUR*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*DEFAULT-ENCODING*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*DEFAULT-SOURCE-REGISTRIES*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*ENCODING-DETECTION-HOOK*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*ENCODING-EXTERNAL-FORMAT-HOOK*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*OUTPUT-TRANSLATIONS-PARAMETER*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*RESOLVE-SYMLINKS*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*SOURCE-REGISTRY-PARAMETER*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*SYSTEM-DEFINITION-SEARCH-FUNCTIONS*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*USER-CACHE*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*UTF-8-EXTERNAL-FORMAT*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*VERBOSE-OUT*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:*WARNINGS-FILE-TYPE*` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:ACCEPT` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:ACTION-DESCRIPTION` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:ADDITIONAL-INPUT-FILES` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:ALREADY-LOADED-SYSTEMS` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:APPLY-OUTPUT-TRANSLATIONS` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:ASDF-MESSAGE` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:ASDF-VERSION` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface
- `ASDF/INTERFACE:BAD-SYSTEM-NAME` -> `ncl-contrib-asdf`, Phase 3: Phase 3 contrib surface

The largest judgment bucket is unclassified COMMON-LISP surface assigned to Phase 2 `ncl-runtime`; it is intentionally visible in `notes` so later implementation lanes can refine it without changing coverage.
