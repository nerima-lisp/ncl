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

The low-level layout API is exposed by `PrettyPrinter`. It provides
`start_logical_block`/`end_logical_block`, `newline` with `Linear`, `Fill`,
`Miser`, and `Mandatory` policies, block/current `indent`, relative/absolute
`tab`, and `column`/`line_count` inspection. `PrintOptions` supplies the
`with_right_margin`, `with_miser_width`, and `with_print_lines` settings used
by printer integrations. Conditional breaks remain pending until the next
text is emitted, so a caller can construct a block without pre-measuring each
item.

`PrettyPrinter` also implements `CharSink`. A FORMAT or logical-block
adapter owns one instance for the output operation, passes it to nested
`ncl_printer::write` calls, and calls `finish()` before returning:

```text
let mut layout = PrettyPrinter::with_options(&mut sink, right_margin, miser_width);
layout.start_logical_block(prefix, per_line_prefix)?;
ncl_printer::write(ctx, runtime, object, &mut layout, &options)?;
layout.newline(NewlineKind::Linear)?;
layout.indent(IndentMode::Block, amount);
layout.tab(TabKind::Relative, column, increment)?;
layout.end_logical_block(suffix)?;
layout.finish()?;
```

`finish()` flushes a trailing conditional break. `end_logical_block()` also
flushes before writing its suffix.

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
entries whose `T` entry is the default. Priority-aware entries use an internal
`(type-specifier function . priority)` representation and are ordered from
highest to lowest priority. Basic `CONS`, `LIST`, `ATOM`, `SYMBOL`, and
`INTEGER` specifiers are recognized.

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
special variables `*PRINT-PPRINT-DISPATCH*`, `*PRINT-PRETTY*`,
`*PRINT-RIGHT-MARGIN*`, `*PRINT-MISER-WIDTH*`, `*PRINT-LINES*`,
`*PRINT-CIRCLE*`, `*PRINT-READABLY*`,
`NCL-EXT:*PRINT-CIRCLE-NOT-SHARED*`, `NCL-EXT:*PRINT-VECTOR-LENGTH*`.
`*PRINT-PPRINT-DISPATCH*` starts as an empty dispatch table, the rest as `NIL`.

## Known gaps

- **`classify_object` is unusable for headerless conses**: it reads a widetag
  from the first payload word, which a cons does not have. The printer detects
  conses with `Word::is_cons` and characters with the sys word API before the
  widetag path.
- **Specialized and non-simple array length**: `ncl-object` exposes no length
  or rank accessor, so the printer probes `specialized_array_ref` for the
  length and uses `array_dimensions` for the rank. Needed additions:
  `specialized_array_length` and non-simple array rank/fill-pointer accessors.
- **Reader round trip**: `ncl-reader` (L2) is not on `main` yet, so the round
  trip test is deferred. Readable output is checked against fixed expected
  strings until the reader lands.
- **Dynamic bindings**: `progv`/special binding machinery updates the symbol
  value cell for the dynamic extent, which `PrintOptions::from_specials` reads.
  A direct binding-stack lookup is still unnecessary for the current runtime
  path but may be needed if value-cell mutation is changed.
- **Builtin bodies**: `PPRINT`, `PPRINT-DISPATCH`, `SET-PPRINT-DISPATCH`, and
  `COPY-PPRINT-DISPATCH` are callable and use the registered output stream or
  ambient dispatch table. The remaining layout primitives still need a
  stream-backed `PrettyPrinter` state adapter.
- **CL pretty-printer connection**: `PrettyPrinter` is the Rust boundary for
  FORMAT and stream integrations; it is now also a nested `CharSink`.
- **Standard dispatch**: operator-specific entries for `QUOTE`, `LET`, and
  `DEFUN` and full Common Lisp type-specifier dispatch remain to be added.

## Verification

```sh
nix develop --command cargo test --locked -p ncl-printer
nix develop --command cargo clippy --locked -p ncl-printer --all-targets --all-features -- -D warnings
python3 scripts/check_standards.py
python3 scripts/reachability.py
```
