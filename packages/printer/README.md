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
pprint_dispatch(&mut ThreadContext, Word, Word) -> Result<Word, ObjectError>
set_pprint_dispatch(&mut ThreadContext, &Runtime, Word, Word, Word) -> Result<Word, ObjectError>
copy_pprint_dispatch(&mut ThreadContext, &Runtime, Word) -> Result<Word, ObjectError>
```

| item | role |
| --- | --- |
| `CharSink` | output trait: `write_char`, `write_str`. `ncl-lib-streams` adapts streams to it. |
| `StringSink` | in-crate `CharSink` that accumulates a `String`. |
| `PrintOptions` | opaque, validated value object for the `*print-*` variables, with `new`, typed builders, and `from_specials`. |
| `PrintBase` | validated radix newtype restricted to 2 through 36. |
| `NonNegative` | validated non-negative limit value used by length and level options. |
| `PrintCase` | `:upcase`, `:downcase`, `:capitalize`. |
| `PrintError` | `Object`, `Sink`, `NotReadable`, `Circularity`. |

`PrintOptions::from_specials` reads the ambient variables when they are interned
and special, and keeps the default otherwise. The dispatch functions operate on
the `*print-pprint-dispatch*` table, a list of `(type-specifier . function)`
entries whose `T` entry is the default.

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
| simple, specialized, and non-simple array | `#(element ...)` or `#nA(element ...)`; `*print-array*` false prints an opaque form |
| hash table, structure, instance, function, package, stream, readtable, code | `#<LABEL 0xADDRESS>`, or `PrintError::NotReadable` when `*print-readably*` is true |

`*print-level*` replaces objects below the depth limit with `#`. With
`*print-circle*`, shared objects get `#n=` at first use and `#n#` after, and a
cycle without `*print-circle*` returns `PrintError::Circularity` instead of
looping. `NCL-EXT:*PRINT-CIRCLE-NOT-SHARED*` restricts labels to cyclic objects.
With `*print-pretty*`, sequence separators become a newline and an indent once
the line reaches the margin.

## Owned symbols

`register` installs the 24 Phase 1 symbols the ownership table assigns to
`ncl-printer`: the functions `PRIN1`, `PRINC`, `PRINT`, `PRIN1-TO-STRING`,
`PRINC-TO-STRING`, `WRITE-TO-STRING`, `PRINT-OBJECT`,
`PRINT-NOT-READABLE-OBJECT`, `PPRINT`, `PPRINT-FILL`, `PPRINT-LINEAR`,
`PPRINT-TAB`, `PPRINT-TABULAR`, `PPRINT-INDENT`, `PPRINT-NEWLINE`,
`PPRINT-DISPATCH`, `SET-PPRINT-DISPATCH`, `COPY-PPRINT-DISPATCH`, and the
`NCL-EXT` functions `PRINT-SYMBOL-WITH-PREFIX`, `PRINT-UNREADABLY`; and the
special variables `*PRINT-PPRINT-DISPATCH*`, `*PRINT-READABLY*`,
`NCL-EXT:*PRINT-CIRCLE-NOT-SHARED*`, `NCL-EXT:*PRINT-VECTOR-LENGTH*`.
`*PRINT-PPRINT-DISPATCH*` starts as an empty dispatch table, the rest as `NIL`.

## Known gaps

- **`classify_object` is unusable for headerless conses and characters**:
  it reads a widetag from the first payload word, which a cons does not have,
  and `Word::character` encodes `(scalar << 4) | 1`, whose `lowtag()` reads as
  `List`, so `Word::is_character` never matches and `classify` reports a
  character as a cons. The printer detects conses with `Word::is_cons` and
  characters with the scalar bound in `print::character_code`. Needed fixes in
  `ncl-sys`/`ncl-object`: make `character` encode the `Character` lowtag, or
  make `classify_object` consult the lowtag before the widetag.
- **Specialized and non-simple array length**: `ncl-object` exposes no length
  or rank accessor, so the printer probes `specialized_array_ref` for the
  length and uses `array_dimensions` for the rank. Needed additions:
  `specialized_array_length` and non-simple array rank/fill-pointer accessors.
- **Reader round trip**: `ncl-reader` (L2) is not on `main` yet, so the round
  trip test is deferred. Readable output is checked against fixed expected
  strings until the reader lands.
- **Dynamic bindings**: `ThreadContext` exposes no binding lookup, so
  `PrintOptions::from_specials` reads value cells, not dynamic bindings.
- **Builtin bodies**: `register` installs function names with an unbound
  placeholder; callable function objects and the `pprint` stream arguments need
  the runtime and stream layers. The `pprint-*` functions have no Rust-side
  entry points yet; `write` with `*print-pretty*` is the working path.
- **Type-specifier dispatch**: `set_pprint_dispatch` stores entries, but
  matching a non-`T` specifier needs `ncl-types`, so lookup compares it with
  `eq`.

## Verification

```sh
nix develop --command cargo test --locked -p ncl-printer
nix develop --command cargo clippy --locked -p ncl-printer --all-targets --all-features -- -D warnings
python3 scripts/check_standards.py
python3 scripts/reachability.py
```
