# ncl-printer

`ncl-printer` renders `ncl-object` values as Lisp text. It owns the `*print-*`
variables, the `prin1`/`princ`/`pprint` family, and `write-to-string`; the
reader round trip is the acceptance target for the full lane.

## Public API

```text
write(&mut ThreadContext, &Runtime, Word, &mut dyn CharSink, &PrintOptions)
    -> Result<(), PrintError>
write_to_string(&mut ThreadContext, &Runtime, Word, &PrintOptions)
    -> Result<Word, PrintError>          // returns a Lisp string
register(&mut ThreadContext, &Runtime) -> Result<(), ObjectError>
```

| item | role |
| --- | --- |
| `CharSink` | output trait: `write_char`, `write_str`. `ncl-lib-streams` adapts streams to it. |
| `StringSink` | in-crate `CharSink` that accumulates a `String`. |
| `PrintOptions` | one-to-one mirror of the `*print-*` variables, with `new`, `with_*` builders, and `from_specials`. |
| `PrintCase` | `:upcase`, `:downcase`, `:capitalize`. |
| `PrintError` | `Object`, `Sink`, `NotReadable`, `Circularity`. |

`PrintOptions::from_specials` reads the ambient variables when they are interned
and special, and keeps the default otherwise.

## Printed forms

| object kind | form |
| --- | --- |
| fixnum, bignum | digits in `*print-base*`, with `#b`/`#o`/`#x`/`#nR` when `*print-radix*` |
| ratio | `numerator/denominator` |
| double float | shortest round-tripping decimal |
| complex | `#C(real imaginary)` |
| character | `#\Name` or the bare character when `*print-escape*` is false |
| string | `"..."` with `\"` and `\\` when escaping, otherwise raw |
| symbol | package prefix (`:`, `PKG:`, bare for `COMMON-LISP`), `#:` for uninterned, `\|...\|` when escaping is required, `*print-case*` conversion |
| cons | list, dotted pair, and the `'` / `#'` abbreviations; `*print-length*` prints `...` |
| simple vector | `#(element ...)`; `*print-array*` false prints an opaque form |
| hash table, structure, instance, function, package, stream, readtable, code | `#<LABEL 0xADDRESS>`, or `PrintError::NotReadable` when `*print-readably*` is true |

`*print-level*` replaces objects below the depth limit with `#`.

## Owned symbols

`register` installs the 24 Phase 1 symbols the ownership table assigns to
`ncl-printer`: the functions `PRIN1`, `PRINC`, `PRINT`, `PRIN1-TO-STRING`,
`PRINC-TO-STRING`, `WRITE-TO-STRING`, `PRINT-OBJECT`,
`PRINT-NOT-READABLE-OBJECT`, `PPRINT`, `PPRINT-FILL`, `PPRINT-LINEAR`,
`PPRINT-TAB`, `PPRINT-TABULAR`, `PPRINT-INDENT`, `PPRINT-NEWLINE`,
`PPRINT-DISPATCH`, `SET-PPRINT-DISPATCH`, `COPY-PPRINT-DISPATCH`, and the
`SB-EXT` functions `PRINT-SYMBOL-WITH-PREFIX`, `PRINT-UNREADABLY`; and the
special variables `*PRINT-PPRINT-DISPATCH*`, `*PRINT-READABLY*`,
`SB-EXT:*PRINT-CIRCLE-NOT-SHARED*`, `SB-EXT:*PRINT-VECTOR-LENGTH*`.

## Known gaps

- **Reader round trip**: `ncl-reader` (L2) is not on `main` yet, so the round
  trip test is deferred. Readable output is checked against fixed expected
  strings until the reader lands.
- **Specialized and non-simple arrays**: `ncl-object` exposes no length or rank
  accessor for them, so they print as `#<ARRAY ...>`. Needed additions:
  `specialized_array_length` and non-simple array rank/fill-pointer accessors.
- **Dynamic bindings**: `ThreadContext` exposes no binding lookup, so
  `PrintOptions::from_specials` reads value cells, not dynamic bindings.
- **Builtin bodies**: `register` installs function names with an unbound
  placeholder; callable function objects need the runtime and stream layers.

## Verification

```sh
nix develop --command cargo test --locked -p ncl-printer
nix develop --command cargo clippy --locked -p ncl-printer --all-targets --all-features -- -D warnings
python3 scripts/check_standards.py
python3 scripts/reachability.py
```
