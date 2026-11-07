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

HIR is a compiler-stage bounded context. Its final crate assignment follows
the [crate map](crates.md). HIR must not depend on runtime objects, machine
registers, or target encoders. MIR lowering is the only boundary where CL
types become machine value classes and generic calls become target call
sequences.

## Storage and identity (FR-I03)

Each top-level form owns an arena. Nodes, types, declarations, and source
records are referred to by small typed indices, not pointers into growable
vectors. A reference is valid only in its owning form arena unless it is an
explicit compilation-unit reference.

The compilation unit owns the cross-form index for definitions, proclamations,
and inline templates. Cross-form references use stable definition identities,
not borrowed node references. An inliner that copies a body into another form
must allocate nodes in the caller's arena and retain the callee's source
origin. This allows `WITH-COMPILATION-UNIT` and `:BLOCK-COMPILE` without
turning every form into one global arena.

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

The implementation may choose structured control nodes, explicit basic
blocks, or a combination. If it uses blocks, each block has one terminator and
all successor references resolve within the owning function. The selected
representation must support dominance, reachability, loop, and data-flow
analysis without erasing lexical scope or dynamic control.

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

Keep declared, asserted, and inferred type facts distinct. A declaration is
not evidence that a value has the declared type. Under the configured safety
policy, the compiler either emits the required check or treats the declaration
as an optimization assumption according to the policy contract. Type
intersections that become empty may report a compile-time contradiction, but
must not change a runtime error into an unrelated compile-time failure when
the policy requires the original check.

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
Common Lisp quality value. At minimum, `SAFETY` controls whether declared
types and call contracts require runtime checks; it must not be inferred from
`SPEED`. Array bounds, argument count, keyword, and general type checks have
separate policy decisions where their contracts differ.

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
5. Every type reference is valid, and every attached inferred fact is a
   conservative over-approximation of possible runtime values. Unknown facts
   cannot authorize an unchecked operation.
6. Every explicit check has a failure edge with the correct Lisp condition
   behavior for its operation and active safety policy.
7. Every source origin resolves, and transform-origin links do not form a
   cycle.
8. Any SSA subgraph verifies its definitions, uses, dominance, and block
   arguments independently of storage order.
9. A pass reports which cached facts it invalidated; no later pass consumes a
   stale type, effect, escape, or CFG result.

CI and test builds always run the verifier. Per decision D-10, release builds
keep verification enabled by default until V2 and V3 pass. After both gates
pass, release builds may disable verification by default, but must retain an
explicit option to enable it. The gate definitions are in the
[compiler pipeline](compiler-pipeline.md#verification-contract). A release
optimization result is never accepted as verified merely because the verifier
was compiled out.

## Compiler performance

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

