# ncl-compiler-front

`ncl-compiler-front` is the front end of NCL. It turns reader output
(`ncl-object` values) into an internal AST, tracks lexical and declaration
environments, expands macros through `MacroCaller`, and lowers the AST into
`ncl-ir` in the `lower` module.

## Frozen AST contract

The following modules are the frozen boundary that the lowering lane consumes.
Adding a field or a variant is a contract change.

| module | contents |
| --- | --- |
| `ast` | `Expr` (one variant per special operator plus `Constant`, `Variable`, `Call`, `Function`, `Lambda`), `Operator`, `LambdaExpr`, `LetBinding`, `LocalFunction`, `LocalMacro`, `SymbolMacro`, `TagbodyItem`, `EvalSituation`, `FunctionDesignator` |
| `literal` | `Literal` and `NumberLiteral`: structurally representable readable data |
| `symbols` | `SymbolRef`: package name, symbol name, and an expansion-local identity for uninterned symbols |
| `types` | `TypeSpecifier`: a placeholder until `ncl-types` freezes its API |
| `lambda_list` | `LambdaList`, `ParamName`, `OptionalParam`, `KeyParam`, `AuxParam` |
| `declaration` | `Declaration`, `OptimizeQuality`, `Quality`, `parse_declare_form` |
| `error` | `FrontError` |
| `form` | readers from `Word` to `Literal` and `SymbolRef` |
| `macro_caller` | `MacroCaller` |
| `compiler_macro` | `MacroRegistry`, `CompilerMacro`, `ArityPattern`, `MacroExpander` |

The tree stores no `ncl-object` `Word`. `quote` and self-evaluating atoms both
parse to `Expr::Constant`; `nil` is `Literal::Nil`. Declarations appear only in
the declaration-bearing forms CLHS names: `lambda`, `let`, `let*`, `locally`,
`flet`, `labels`, `macrolet`, and `symbol-macrolet`.

`MacroCaller` is the seam the runtime implements (Wave 3). `call_macro`
expands a global macro; `call_local_macro` expands a `macrolet` definition and
has a default implementation that fails loudly, because a local macro body must
be evaluated at expansion time and the front end cannot do that alone.

## Expansion

`FormExpander` walks a form, resolves macros, and produces the AST. The
resolution order is: special operator, local macro, global macro (the symbol's
macro bit), compiler macro, then a function call. `expand/parameters` parses
ordinary and macro lambda lists and enforces the section order of CLHS 3.4.1.
`special/*` parses the 25 Common Lisp special operators plus `sb-ext:truly-the`
and `sb-sys:nlx-protect`.

## Registration and ownership

`register(&Runtime)` interns the 152 Phase 1 symbols the ownership table assigns
to this crate, creates `SB-EXT` and `SB-SYS`, and sets the `macro`, `constant`,
and `special` flag bits. It is idempotent and is what `ncl-stdlib` calls in
dependency order.

The 35 `function`-kind rows and the `FUNCTION` class row are registered as
`Word::UNBOUND` placeholders, because the `ncl-ownership` gate looks those up by
name (`tests/coverage.rs`). `ncl-runtime` (L23) replaces the function
placeholders with real function objects and `ncl-clos` (L18) the `FUNCTION`
class. `owned_symbols.rs` is generated from
`conformance/ownership/symbols.tsv`; regenerate it with:

```text
python3 -c 'import pathlib; src = pathlib.Path("conformance/ownership/symbols.tsv").read_text(); km = {"class":"Class","condition":"Condition","constant":"Constant","function":"Function","macro":"Macro","other":"Other","special-operator":"SpecialOperator","type":"Type","variable":"Variable"}; [print(f"    OwnedSymbol {{ package: \"{f[0]}\", name: \"{f[1]}\", kinds: &[{", ".join("SymbolKind::" + km[x] for x in f[2].split("+"))}] }},") for f in (l.split(chr(9)) for l in src.splitlines()[1:]) if len(f) == 7 and f[3] == "ncl-compiler-front"]'
```

## Lowering

`lower::lower_toplevel(&Expr) -> Result<Lowered, LowerError>` lowers one
expanded form into an entry `ncl_ir::Function` and one function per lambda the
form contains:

```rust
pub struct Lowered {
    pub entry: Function,
    pub nested: Vec<Function>,
}
```

Every generated function passes `ncl_ir::verify`. `ncl-runtime` (L23) calls
`lower_toplevel` on each expanded form, registers `entry` and every `nested`
function, and links them; `LowerError` names any form with no representation.

A generated function takes a leading `argc` parameter of type `Word` followed by
its captured variables and its declared parameters, and returns one `Word`; the
entry function takes no parameters. `argc` counts the actual arguments the
caller supplied, excluding the captures and the `argc` slot, so an `&optional`
parameter compares it against its position. The lowering implements the front-end
rules of `compiler-pipeline.md`:

- a lexical variable a lambda captures and some form assigns is boxed in a
  one-word cell (`Alloc { words: 1 }`) that the closures capture by reference; a
  read-only capture is passed by value;
- a `block`/`tagbody` whose `return-from`/`go` stay inside one function becomes
  `Jump` between basic blocks;
- `multiple-value-prog1` records its first form's value with
  `SetMultipleValues`, and `multiple-value-call` goes through the variadic
  adapter builtin;
- an `&optional` parameter lowers to `LoadArg`, an `argc` comparison, and a
  default block.

`tests/lower.rs` holds one test per rule and asserts the generated IR.

## Compiler macros

`MacroRegistry` is wired into expansion but has no entries. The direct-expansion
primitives it is meant to hold (`car`, `cdr`, fixnum arithmetic, `eq`, and so
on) are not yet distinguishable in the frozen AST, so lowering needs no entry
and the table stays empty until those forms gain a representation.

## Dependencies that had not landed

`ncl-types` and `ncl-reader` were empty skeletons when this contract was frozen,
and `ncl-types` is still an empty skeleton at origin/main. The `TypeSpecifier`
placeholder therefore stays; replace it and convert at the `form` boundary when
`ncl-types` lands. The tests build reader input directly from `ncl-object`
values.

## Known gaps

- `ncl-ir` has no constant or op naming a function entry, so a lambda callee is a
  `Constant::Symbol` placeholder and its captured values are passed as leading
  arguments rather than through a closure object. `catch`, `unwind-protect`, and
  `progv` lower to `LowerError::Unsupported` pending the runtime lane's
  handler-region and builtin lowering.
- `&rest` and `&key` lower to `LowerError::UnsupportedLambdaList`, because the
  code generator owns their prologue.
- The cell-boxing analysis is name-based, so a shadowed variable can be boxed
  conservatively.
- `sb-sys:%primitive` reports `FrontError::MalformedForm`: the frozen AST has no
  node that distinguishes a runtime builtin call from an ordinary call.
- A compound function name such as `(setf f)` cannot be a `SymbolRef`.
- `sb-sys:nlx-protect` is approximated as `unwind-protect`; the distinction
  between a cleanup that runs on every exit and one that runs only on a
  non-local exit is not represented.
- A specialized array (a bit vector) cannot be converted to a `Literal`: the
  object layer exposes no length accessor for it.
- `ncl_object::classify_object` reads a widetag for every word, so it reports a
  garbage widetag for conses and immediates. `form::classify_form` consults the
  lowtag first and is used instead.
- A registered `ThreadContext` must be dropped before its `Runtime`; the test
  fixture declares its context first for that reason.

## Verification

```text
nix develop --command cargo fmt -p ncl-compiler-front
nix develop --command cargo test --locked -p ncl-compiler-front
nix develop --command cargo clippy --locked -p ncl-compiler-front --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" nix develop --command cargo doc --locked -p ncl-compiler-front --all-features --no-deps
python3 scripts/check_standards.py
python3 scripts/reachability.py
```
