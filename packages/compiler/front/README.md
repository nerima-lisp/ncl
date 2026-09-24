# ncl-compiler-front

`ncl-compiler-front` is the front end of NCL. It turns reader output
(`ncl-object` values) into an internal AST, tracks lexical and declaration
environments, expands macros through `MacroCaller`, and, in the `lower` module
owned by the back-half lane, lowers the AST into `ncl-ir`.

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

`register(&Runtime)` interns the 151 Phase 1 symbols the ownership table assigns
to this crate, creates `SB-EXT` and `SB-SYS`, and sets the `macro`, `constant`,
and `special` flag bits. It is idempotent and is what `ncl-stdlib` calls in
dependency order.

The 35 `function`-kind rows and the `FUNCTION` class row are **not** registered
here, because they need the actual functions and the class. The back-half lane
(L12b) supplies them, and the `ncl-ownership` coverage test for this crate
therefore completes at L12b. `owned_symbols.rs` is generated from
`conformance/ownership/symbols.tsv`; regenerate it with:

```text
python3 -c 'import pathlib; src = pathlib.Path("conformance/ownership/symbols.tsv").read_text(); km = {"class":"Class","condition":"Condition","constant":"Constant","function":"Function","macro":"Macro","other":"Other","special-operator":"SpecialOperator","type":"Type","variable":"Variable"}; [print(f"    OwnedSymbol {{ package: \"{f[0]}\", name: \"{f[1]}\", kinds: &[{", ".join("SymbolKind::" + km[x] for x in f[2].split("+"))}] }},") for f in (l.split(chr(9)) for l in src.splitlines()[1:]) if len(f) == 7 and f[3] == "ncl-compiler-front"]'
```

## What L12b inherits

- The `lower` module: AST to `ncl-ir`, one test per D4 rule (cell boxing for a
  variable captured mutably by two closures, a non-escaping `block`/`tagbody` as
  `Jump`, `multiple-value-call`/`multiple-value-prog1` through the variadic
  adapter ABI, and `&optional` default blocks), with every generated function
  passing `ncl_ir::verify`.
- The `function`-kind and `FUNCTION` class rows of the ownership table, which
  make this crate's coverage test pass.
- The compiler-macro table: `MacroRegistry` is wired into expansion but has no
  entries yet.

## Dependencies that had not landed

`ncl-types` and `ncl-reader` were empty skeletons when this contract was
frozen. `TypeSpecifier` is therefore a local placeholder carrying the source
form as a `Literal`, and the tests build reader input directly from
`ncl-object` values. Replace the placeholder when `ncl-types` lands.

## Known gaps

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
