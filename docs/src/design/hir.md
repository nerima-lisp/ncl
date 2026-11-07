# High-level intermediate representation

This page defines the semantic intermediate representation (HIR) used by the
Common Lisp compiler optimization pipeline. HIR preserves Lisp-level meaning
long enough for type inference, known-function transforms, control-flow
optimization, and allocation analysis. The existing low-level SSA IR remains
the machine-oriented representation, called MIR in the pipeline contract.

The design fixes observable semantics, pass boundaries, and verification
requirements. Arena layout details, node encoding, and individual algorithms
remain implementation choices when they satisfy this contract.

## Boundary and ownership

HIR construction begins after source macro expansion and resolves lexical
bindings. It preserves package-qualified symbol identity, full Common Lisp type
specifiers, dynamic bindings, declarations, policy, and source origins. The
current expansion and lowering path is documented in the
[compiler pipeline](compiler-pipeline.md). The replacement path must make
compiler-macro definitions and compilation-unit state visible where they are
semantically in scope.

HIR belongs to the new `ncl-hir` crate at `packages/compiler/hir/`. The
l02a-crates lane must revise the [crate map](crates.md) before L3-1 starts,
adding that workspace member and these edges: `ncl-compiler-front -> ncl-hir`,
`ncl-hir -> ncl-ir`, and `ncl-opt -> ncl-hir`. The current crate map at
`crates.md` lines 58-59 does not yet include this member. `ncl-hir` must not
depend on runtime objects, machine registers, or target encoders. MIR lowering
is the only boundary where CL types become machine value classes and generic
calls become target call sequences.

## Storage and identity (FR-I03)

FR-I03 sets the compiler-performance direction: use per-form arenas and index
references, intern names, memoize type operations, and avoid repeated vector
reconstruction.

Each top-level form owns an arena. Nodes, types, declarations, and source
records are referred to by small typed indices, not pointers into growable
vectors. A reference is valid only in its owning form arena unless it is an
explicit compilation-unit reference.

`ncl-hir` owns the compilation-unit index across form arenas. It indexes
definitions, proclamations, and inline templates. Cross-form references use
stable definition identities, not borrowed node references. Runtime owns the
compilation entry point and the lifetime of the compilation, not this index.
An inliner that copies a body into another form must allocate nodes in the
caller's arena and retain the callee's source origin. This allows
`WITH-COMPILATION-UNIT` and `:BLOCK-COMPILE` without turning every form into one
global arena or making `ncl-opt` depend on runtime.

Intern package-qualified symbol names once and use a `SymbolId` for global
function and variable identity. Lexical bindings use distinct `BindingId`
values. Equal printed names do not make two bindings or two symbols identical.
Uninterned symbols keep their identity separate from their spelling.

The implementation chooses the exact integer width and arena container. It
must detect index overflow instead of wrapping an index into a different
node.

## Semantic nodes

HIR nodes represent Lisp operations before machine representation selection.
The node set must distinguish at least:

- literals, lexical references, special-variable references, global-variable
  references, and assignments;
- `LET`, `LET*`, `FLET`, `LABELS`, lambda expressions, closure references,
  direct local calls, named global calls, and calls through function values;
- `IF`, `BLOCK`/`RETURN-FROM`, `TAGBODY`/`GO`, `CATCH`/`THROW`,
  `UNWIND-PROTECT`, and non-local exits;
- multiple-value production, binding, return, and calls;
- type assertions, known-function operations, generic sequence and array
  operations, and explicit runtime checks;
- top-level definitions and compile-time forms needed by the compilation
  unit.

The operator identity remains explicit until a transform proves a more
specific operation. In particular, a call to a Common Lisp function is not
lowered to an opaque name-only builtin during HIR construction. Preserve the
distinction between a stable local function, a re-definable global function,
a known compiler intrinsic, and an arbitrary function value. Global calls must
retain the indirection required by function redefinition semantics.

Macro expansion may replace source forms, but every generated node retains a
link to the originating source form and expansion chain. Unsupported forms
must produce a front-end diagnostic; they must not silently become an
unoptimizable placeholder node.

## Variables, functions, and control flow

A variable reference identifies its binding, not only its symbol. Its binding
kind is lexical, special, or global. HIR records assignments and read/write
uses so assignment conversion, capture analysis, and escape analysis do not
need to reconstruct scope from names.

Local functions have identities independent of global function symbols.
`FLET` and `LABELS` preserve their distinct recursive scope rules. HIR records
whether a function value escapes, but the escape result is an analysis fact,
not a source-level semantic property. Closure layout is chosen only after
analysis.

Control-flow nodes preserve dynamic binding and non-local exit semantics.
Ordinary local branches may become CFG edges. A `RETURN-FROM`, `GO`, or
`THROW` that crosses a function boundary remains an explicit non-local exit
until its target and cleanup behavior are proven. `UNWIND-PROTECT` cleanup
must run on each applicable exit. Multiple values retain their count and
ordering until a call boundary requires a convention conversion.

HIR uses structured nodes for lexical and dynamic control constructs, cleanup,
and non-local exits, with explicit basic blocks for local branches and loops.
Each block has one terminator and all successor references resolve within the
owning function. The CFG view must support dominance, reachability, loop, and
data-flow analysis without erasing the structured scope or cleanup semantics.
L3-1b implements this hybrid representation.

## Common Lisp type lattice

HIR types describe Common Lisp values, not machine registers. The type model
must cover:

- the universal and empty types and named type specifiers;
- numeric domains with lower and upper bounds, independently inclusive or
  exclusive, including integer ranges and supported floating-point ranges;
- `MEMBER` values with Common Lisp equality semantics;
- canonicalizable unions and intersections corresponding to `OR` and `AND`;
- complement types, cons types, array element type and dimensions, function
  types, and multiple-value types;
- unresolved or implementation-defined specifiers retained as opaque source
  specifiers when their semantics cannot be decided statically.

An unresolved type relation is not a negative answer. Type relation queries
return `yes`, `no`, or `unknown`, matching the two-value certainty behavior of
`SUBTYPEP`. Only a proven `yes` may remove a type check or select an operation
whose preconditions depend on that subtype. An opaque type may be preserved,
but it cannot authorize an unchecked representation.

`ncl-types` remains the runtime model: it parses and stores Lisp type
specifiers and implements `TYPEP` and `SUBTYPEP`. `ncl-hir` owns a separate,
compiler-only abstract lattice for inference and optimization; it does not
store runtime `Word` values or depend on `ncl-types`. The front end maps known
`ncl-types::TypeSpecifier` constructors into HIR type nodes. For an unresolved
or implementation-defined specifier, it retains an opaque HIR node with the
source `SymbolId` and operands. Runtime type checks continue to use
`ncl-types`; HIR facts may remove a check only when their subtype result is
proven `yes`. Shared known specifiers must have mapping tests so the compiler
and runtime models preserve the same CL meaning.

Keep declared, asserted, and inferred type facts distinct. A declaration is
not evidence that a value has the declared type.

FR-C08 defines the policy for type, arity, keyword, and array-bound checks.
For these checks a call is safe when the active `SAFETY` value is at least 1.
`SAFETY 0` trusts declared types and call contracts; it does not make an
invalid operation safe for Rust or the machine backend.

| Check | `SAFETY 0` | `SAFETY 1` | `SAFETY 3` |
|---|---|---|---|
| Declared or asserted type | Treat declarations as optimization assumptions and omit policy-controlled type checks. | Check a declared or asserted type unless HIR proves it or an explicit runtime guard already enforces it. | Keep the same required check set as level 1; do not weaken a check because a declaration enabled specialization. |
| Argument count | Calls are unsafe; a dynamic arity check may be omitted. A statically known invalid call may be diagnosed, but runtime `PROGRAM-ERROR` behavior is not promised. | Calls are safe. Check dynamic calls unless the known entry and argument count prove the call valid; diagnose a statically known mismatch. | Same safe-call rule as level 1; do not omit a dynamic check without proof. |
| Keyword arguments | Calls are unsafe; a dynamic keyword check may be omitted. | Calls are safe. Validate dynamic keys unless the known entry or compile-time arguments prove them valid; diagnose a statically known mismatch. | Same safe-call rule as level 1; do not omit a dynamic check without proof. |
| Array bounds | Omit the Lisp bounds check. The backend must still avoid Rust undefined behavior and invalid machine memory access. | Check each access unless range analysis proves the index is in bounds. | Check each access unless range analysis proves the index is in bounds; safety-driven specialization must retain its guard. |

For these four check classes, levels 1 and 3 have the same required checks;
level 3 forbids weakening or removing a required check without proof. The
compiler may remove a check at any level only after proving its condition or
reusing an equivalent guard. A safe-call arity failure signals `PROGRAM-ERROR`
as specified by the shared condition contract. An unsafe call has undefined
consequences.

At `SAFETY` 1 or 3, an empty type intersection derived from known facts may be
reported as a compile-time contradiction. At `SAFETY` 0, do not turn inferred
facts alone into a compile-time error; retain the runtime behavior specified by
the check matrix.

The implementation may intern normalized type nodes and memoize subtype,
union, intersection, and range operations. Cache keys include the full type
identity and any environment that affects user-defined type expansion.
Unknown results must not be cached as definite yes or no.

## Declarations and optimization policy

HIR retains declarations at their lexical scope and records the effective
policy at each function and optimization region. Preserve source declarations
even after deriving facts from them, so diagnostics and `NOTINLINE` decisions
can refer to the original contract.

The policy includes `SPEED`, `SAFETY`, `DEBUG`, and `SPACE`, each with its
Common Lisp quality value. `SAFETY` uses the explicit FR-C08 matrix above and
is never inferred from `SPEED`. `DEBUG`, `SPEED`, and `SPACE` remain separate
quality settings.

Debug policy controls variable visibility, retained source paths, and frame
state. It may change which metadata reaches machine code, but it must not
change the source meaning of the HIR. `NOTINLINE` suppresses both HIR inlining
and compiler-macro expansion where required by the language contract.

## Source locations

Every source-derived node has a source location containing a file identity,
form identity, and source span. Line and column are available for diagnostics.
Generated nodes point to an expansion-origin record that names the parent
location and transform. This preserves both the user form to report and the
expanded form to inspect. The implementation chooses byte or character span
encoding, but it must use one encoding consistently and validate its bounds.

Source locations are arena-indexed and do not embed heap pointers. An
optimization that merges or clones nodes retains an origin for each resulting
operation; it must not invent a source location from a neighboring node.

## Verifier contract

The HIR verifier runs after construction and after every HIR pass. It checks at
least:

1. Every node, type, binding, symbol, block, constant, and source-location
   index is in bounds and belongs to the expected form or compilation unit.
2. Each lexical reference resolves to exactly one visible binding; special
   and global references are not mistaken for lexical references.
3. Local function references obey `FLET`/`LABELS` scope. Cross-form function
   references resolve through the compilation-unit index or remain explicitly
   unresolved for normal compile-time diagnostics.
4. Each control-flow successor, non-local exit target, cleanup, and multiple-
   value edge satisfies its arity and scope contract.
5. Every type and fact reference is valid. Each cached fact has a valid source
   epoch and provenance. An `unknown` fact cannot authorize an unchecked
   operation.
6. Every explicit check has a success edge and a failure edge whose condition
   matches its operation and active safety policy. A type check is represented
   as `CheckType { value: ValueId, accepted_type: TypeId,
   expected_type: TypeSpecConstId, source_origin: SourceOriginId }`; the
   failing edge lowers to one runtime signal entry named
   `ncl_signal_type_error(datum, expected_type, source_origin)`. `expected_type`
   is a GC-rooted constant-pool `Word` containing the complete Lisp type
   specifier, not `ObjectType`, a class name, or a string. The runtime entry
   must not route this through `LispError::TypeError { expected: ObjectType }`,
   which cannot carry the full specifier. It constructs `TYPE-ERROR` with both
   standard slots and routes it through the condition boundary and active
   handler search. Source origin is a code-object origin ID resolved by runtime
   source metadata. The `conditions.md` contract from l02d-cond must use this
   exact entry name and expected-type encoding.
7. Every source origin resolves, and transform-origin links do not form a
   cycle.
8. Any SSA subgraph verifies its definitions, uses, dominance, and block
   arguments independently of storage order.
9. Every cached type, effect, escape, dominance, and CFG fact carries the
   epoch of its source IR domain. A mutation increments the affected domain
   epoch; a consumer rejects facts with an older epoch. Each pass declares the
   domains it invalidates, and the verifier checks that no stale fact is
   consumed.

The verifier checks fact references and epochs; it does not prove that an
inferred type is a sound over-approximation. Type-fact soundness is a V3
obligation: differential tests compare generated UB-free programs with SBCL
under optimization-off, default, and `(SPEED 3)` policies. Add targeted
type-boundary cases for `SAFETY 0`, `1`, and `3` to that corpus. An `unknown`
fact must never authorize check removal, which the verifier checks
structurally.

CI and test builds always run the verifier. Per decision D-10, release builds
keep verification enabled by default until V2 and V3 pass. After both gates
pass, release builds may disable verification by default, but must retain an
explicit option to enable it. The gate definitions are in the
[compiler pipeline](compiler-pipeline.md#verification-contract). A release
optimization result is never accepted as verified merely because the verifier
was compiled out.

## Compiler performance

Keep the module facades `cl_type.rs`, `hir_builder.rs`, and `type_inference.rs`
below the 500-line file limit. Put implementation in named child modules under
`cl_type/`, `hir_builder/`, and `type_inference/`; do not use `mod.rs`.

HIR uses per-form arenas and indices to avoid a heap allocation per edge.
Intern names and memoize type operations. Reuse pass worklists and buffers
where ownership permits; do not rebuild vectors for unchanged nodes. Deduplicate
macro expansion only when the source form and expansion environment match.
These are design constraints, not performance claims; measure compile time
and peak memory on the fixed corpus before claiming an improvement.

File-level parallel compilation depends on a thread-safe compilation-unit
environment. It must not be enabled while shared definitions or macro state
can race. A future FASL cache key includes source-content hash, compiler
version, policy, and target features; a cache hit does not bypass HIR/MIR
verification when the selected build policy requires it.

