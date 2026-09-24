# ncl-types

Type specifiers, `typep`, `subtypep`, and the standard type-related symbol
registrations for NCL.

## Public API

```rust
pub enum TypeSpecifier { /* Named, IntegerRange, Or, And, Not, Member, Eql,
                            Satisfies, Array, Vector, Cons, Function, Values,
                            Deftype */ }
pub enum ArrayDimensions { Wild, Ranks(Vec<Option<Word>>), Rank(usize) }
pub enum NamedType { /* T, Nil, Boolean, ..., ValuesType */ }
pub enum TypeError { /* InvalidSpecifier, Object, CannotInvoke,
                       UnexpandedDeftype */ }

pub fn parse_type_specifier(ctx: &mut ThreadContext, spec: Word)
    -> Result<TypeSpecifier, TypeError>;
pub fn typep(ctx: &mut ThreadContext, object: Word, spec: &TypeSpecifier)
    -> Result<bool, TypeError>;
pub fn subtypep(sub: &TypeSpecifier, sup: &TypeSpecifier)
    -> Result<(bool, bool), TypeError>;   // (subtype-p, is-definite)
pub fn register(runtime: &Runtime) -> Result<(), ObjectError>;
```

## Registration

`register` interns the 64 Phase 1 symbols owned by this crate (50 in
`COMMON-LISP`, 14 in `SB-EXT`), registers the standard class names, and binds
the type-related function names. Class objects are placeholders
(`Word::fixnum(1)`) and function objects are placeholders (`Word::UNBOUND`),
matching `ncl_object::register`; real class objects arrive with `ncl-clos`
and real builtin ABI bindings with `ncl-runtime` code objects.

## Known gaps (partial Phase 1 implementation)

- The character family (`character`, `base-char`, `standard-char`,
  `extended-char`) never matches: `ncl-sys` encodes immediate characters with
  a lowtag that overlaps the list lowtag, so `classify` cannot reach its
  `Character` arm. This is an `ncl-sys` defect tracked outside this crate.
- `short-float`, `single-float`, and `long-float` have no object
  representation and never match.
- `base-string`, `simple-string`, and `simple-base-string` share the single
  `STRING` representation.
- `(satisfies predicate)` parses but `typep`/`subtypep` return
  `TypeError::CannotInvoke` (predicate invocation needs the runtime call
  machinery). `deftype` names parse into `TypeSpecifier::Deftype` and return
  `TypeError::UnexpandedDeftype` until the compiler front expands them.
- Array/vector `element-type` is parsed but not enforced; only dimensions,
  rank, and length are checked. `IntegerRange` compares fixnums only.
- `subtypep` is partial: the numeric tower, ranges, the sequence/array
  hierarchy, and `or`/`and` are decided; everything else is `(false, false)`.

## Change history

- Revision 1 (first, frozen): initial public API as listed above.
- Post-review fix before freeze: `register` sets the constant bit on the 11
  owned `constant` symbols (the predecessor interned them without the bit and
  the ownership gate rejected them), and the character family reports `false`
  rather than testing an unreachable `classify` arm. No public API change.
