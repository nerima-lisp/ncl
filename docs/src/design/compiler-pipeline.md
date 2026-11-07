# Compiler pipeline

This page records the current NCL path and the contract for replacing direct
front-end lowering with HIR. HIR retains Common Lisp meaning and type facts;
MIR is the existing low-level SSA form consumed by code generation. HIR's data
model and verifier are specified in [HIR](hir.md).

## Current implementation

The current path is:

```text
reader forms
  -> front-end macro expansion and AST
  -> direct lowering to ncl-ir SSA
  -> ncl-opt pass manager
  -> ncl-codegen MachineFunction and target encoder
  -> native code, safepoint metadata, and runtime publication
```

`read_forms` reads source into forms (`packages/runtime/src/compile.rs:407-414`).
The front-end module describes the AST and lowering boundary
(`packages/compiler/front/src/lib.rs:1-6`). `Runtime::compile_form_with_values`
creates a fresh compiler-macro registry, expands the form, lowers it to
`ncl-ir`, runs optimization, then compiles each function
(`packages/runtime/src/lib.rs:189-209`). The current IR has low-level value
types `Word`, `I64`, `F64`, `Address`, `Bool`, and `Unit`
(`packages/ir/src/types.rs:55-64`). Its `Builtin` operation carries a string
name, while calls are represented by `Call`, `CallIndirect`, or `CallClosure`
(`packages/ir/src/types.rs:307-337`). Type declarations are reduced to scalar
hints during lowering (`packages/compiler/front/src/lower/v2.rs:304-320`).
`compile_native` selects a target compiler, publishes machine code, encodes
safepoint maps, and registers code metadata
(`packages/runtime/src/lib.rs:252-294`). Code generation exposes a
`MachineFunction` (`packages/codegen/src/machine.rs:81`) and target-specific
compile functions (`packages/codegen/src/target_x86_64.rs:101`,
`packages/codegen/src/target_aarch64.rs:31`).

The default optimizer order is SCCP, TypeEffect, InlineDirectCalls, global
value numbering, and dead-code elimination
(`packages/compiler/opt/src/manager.rs:37-44`). The manager repeats the pass
sequence until unchanged or until its configured iteration limit, and verifies
after each pass by default (`packages/compiler/opt/src/manager.rs:55-127`,
`packages/compiler/opt/src/lib.rs:216-222`). This is the current pass list.

The current inliner resolves only `OpKind::Call` whose callee is a constant
`FunctionEntry` (`packages/compiler/opt/src/inline.rs:167-193`). Ordinary
named calls lower to `CallClosure` (`packages/compiler/front/src/lower/expr.rs:255-299`),
and direct builtins lower to `Builtin`
(`packages/compiler/front/src/lower/expr.rs:235-253`). The inliner therefore
runs but does not match the ordinary named-call shape.
Presence in the pass list is not evidence that a call was inlined.

The compiler-macro registry contains Rust callback entries
(`packages/compiler/front/src/compiler_macro.rs:61-85`). The runtime path
passes a newly empty registry to the form expander
(`packages/runtime/src/lib.rs:189-194`). The ordinary-function probe ran under
the project development environment:

```sh
cargo run --quiet -- --eval '(progn (defun ncl-compiler-macro-probe (x) 99) (ncl-compiler-macro-probe 1))'
```

It exited 0 and printed `99`. The compiler-macro probe was:

```sh
cargo run --quiet -- --eval '(progn (defun ncl-compiler-macro-probe (x) 99) (define-compiler-macro ncl-compiler-macro-probe (x) 7) (ncl-compiler-macro-probe 1))'
```

It exited 1 with a `MacroExpansion` `TypeError` while expanding
`DEFINE-COMPILER-MACRO`, before reaching the call.
This runtime compilation path has no working user-defined compiler-macro
application: its registry starts empty, and the definition probe fails first.

## HIR and MIR boundary

HIR is the semantic optimization layer required by D-02, which separates
Lisp-semantic HIR from machine-oriented MIR, between macro expansion and the
existing low-level `ncl-ir` MIR. It belongs to the new `ncl-hir` crate at
`packages/compiler/hir/`. The l02a-crates lane must revise the [crate map](crates.md)
before L3-1 starts, adding the member and the
`ncl-compiler-front -> ncl-hir -> ncl-ir` and
`ncl-opt -> ncl-hir` edges. HIR keeps package-qualified identities for
functions and variables, lexical and dynamic scope, full CL type facts,
declarations, source origins, and explicit condition behavior. HIR operations
must not be reduced to name-only builtins before known-function transforms,
type inference, call analysis, and policy checks have run.

HIR owns the compilation-unit index across per-form arenas. The inliner reads
that index through `ncl-opt -> ncl-hir`; runtime owns the compilation entry
point but not the index. `ncl-types` remains the runtime type-specifier model,
while HIR owns the compiler-only abstract lattice and front end maps known
runtime specifiers into it. See the [HIR type contract](hir.md#common-lisp-type-lattice).
MIR carries control flow, machine-oriented value classes, explicit call and
condition edges, and the GC facts needed by code generation. The crate edges
listed above are a required addition to the crate map, not a change to its
other ownership rules.

The HIR-to-MIR boundary must:

- retain the source form and debug origin for each emitted operation;
- lower explicit HIR checks to MIR branches with the required Lisp condition
  behavior;
- select internal specialized entries only when the call target and argument
  facts are proven, while public/global calls preserve redefinition and error
  semantics;
- keep proven fixnum, signed/unsigned integer, single/double float, complex,
  SAP, and character values unboxed where the target ABI permits, and box at
  generic calls, escaping storage, and public boundaries that require a
  general Lisp value;
- mark unboxed values as non-pointers in stack maps and GC maps, and retain
  roots for tagged values live at calls, allocations, and backedges;
- preserve multiple values, dynamic bindings, cleanup behavior, and
  non-local exits.

Representation selection uses HIR type facts but chooses physical register,
stack, and boxed/unboxed forms during MIR lowering and code generation. A type
fact is not itself a register assignment.

## Ordered pipeline contract

| Stage | Representation | Work and dependency |
|---|---|---|
| 1. Read and expand | Forms, then front-end AST | Read source, macroexpand ordinary macros, apply compiler macros and source transforms in the correct environment, and retain source expansion origins. A compiler macro obeys `NOTINLINE`. |
| 2. Build HIR | `ncl-hir`: per-form arenas plus its compilation-unit index | Resolve lexical bindings and package-qualified symbols; attach declarations, policy, source locations, and known-function facts. Verify the completed form. |
| 3. Normalize local functions | HIR | Perform local-call analysis, assignment conversion, contification, capture analysis, and tail-position marking. Preserve `FLET` and `LABELS` scope rules. |
| 4. Infer and optimize | HIR | Run type/effect/constraint propagation, known-function and sequence transforms, inlining, SCCP, GVN/CSE, copy propagation, jump threading, and loop transforms. Re-run dependent analyses after a rewrite changes calls or control flow. |
| 5. Generate required checks | HIR | `H130 CheckGeneration` inserts policy-required type, argument-count, keyword, and array-bound checks as explicit condition edges after inference. Lower type-check failures through `ncl_signal_type_error(datum, expected_type, source_origin)`; pass a full type-specifier constant and code-origin ID. Apply the [FR-C08 safety matrix](hir.md#declarations-and-optimization-policy), then run post-check simplification. |
| 6. Select allocation lifetime | HIR facts | Run escape and dynamic-extent analysis after HIR transforms and required checks have exposed all relevant control flow. Decide which values may use stack lifetime; defer layout, barriers, TLAB code, and physical representation to MIR/codegen. Omit a write barrier only for a raw non-managed slot, as specified by the [GC contract](gc-interface.md); every managed reference store writes the slot and then calls the barrier. |
| 7. Lower and optimize | MIR | Lower semantic operations and selected representations; run MIR cleanup, instruction selection, address/immediate folding, compare/branch fusion, and peephole optimization. Preserve explicit GC maps and debug origins. |
| 8. Allocate and emit | Machine IR and code | Run register allocation and its checker, emit target code and relocation data, and publish specialized/public entries with the runtime metadata. |

The HIR verifier runs after construction and after every HIR pass. The MIR
verifier runs after each MIR pass. The machine verifier and register allocator
checker run before encoding. A pass that changes a fact invalidates dependent
type, effect, escape, dominance, and liveness information before the next
consumer uses it.

### Stable pass IDs and order

These IDs name the HIR optimization schedule and the later MIR passes. IDs are
stable across releases and are never reassigned. A new pass receives a new ID
and an explicit order before any implementation lane starts.

| Phase | ID | Pass |
|---|---|---|
| HIR fixed point before checks | H010 | Local-call analysis and assignment conversion |
| HIR fixed point before checks | H020 | Control normalization, tail-position marking, and capture analysis |
| HIR fixed point before checks | H030 | Type, effect, and constraint propagation |
| HIR fixed point before checks | H040 | Known-function and guarded source transforms |
| HIR fixed point before checks | H050 | Inlining and dependent-analysis invalidation |
| HIR fixed point before checks | H060 | Arithmetic and sequence transforms |
| HIR fixed point before checks | H070 | SCCP |
| HIR fixed point before checks | H080 | GVN/CSE |
| HIR fixed point before checks | H090 | Copy propagation |
| HIR fixed point before checks | H100 | Jump threading |
| HIR fixed point before checks | H110 | Loop transforms |
| Check insertion | H130 | Policy-driven check generation |
| HIR fixed point after checks | H140 | Check-aware simplification and dead-code elimination |
| Final HIR analysis | H150 | Escape and allocation-lifetime analysis |
| MIR | M010 | Representation selection and semantic lowering |
| MIR | M020 | MIR value and control-flow cleanup |
| MIR | M030 | Target instruction selection |
| MIR | M040 | Address and immediate folding |
| MIR | M050 | Compare/branch fusion |
| MIR | M060 | Target peephole cleanup |
| Machine | M070 | Register allocation and its checker |

HIR passes H010-H110 run in ID order to a fixed point. H130 runs once after
that fixed point; H140 then runs to a fixed point, and H150 recomputes escape
and lifetime facts from the final checked CFG. MIR and machine passes run in ID
order after lowering. Each pass reports which fact domains it invalidates.

### Fixed points and opt-bisect

Constraint propagation processes strongly connected control-flow regions to
a fixed point. The HIR optimizer repeats dependent passes until no pass changes
the module. Every loop is bounded by explicit optimizer fuel and an iteration
limit. If fuel expires, the compiler reports that optimization stopped; it
must not report convergence or use unverified facts. The last verified HIR is
the rollback point for a failed pass.

Pass invocation order is the flattened deterministic trace of the ordered pass
schedule across fixed-point iterations. `opt-bisect N` runs the first N pass
invocations in that trace, not the first N distinct pass IDs; a repeated pass
in a later iteration is another invocation. It verifies the boundary after
the Nth invocation, then binary-searches the smallest invocation prefix that
reproduces a failure. `N = 0` runs no optimization pass invocation. The report
records the pass ID, invocation iteration, source form, and policy. Pass dumps
are available at each boundary. A verifier
failure or fuel exhaustion cannot be hidden by bisect mode.

Per D-10, CI and tests always verify HIR. Release builds keep the verifier
enabled by default until V2 and V3 pass; afterward, release builds may disable
it by default but retain an explicit enable option. This keeps verification on
while type and behavior soundness lack differential evidence, then permits the
compile-time cost to be removed from ordinary release builds.

V2 is the fixed-seed ANSI test suite's random tester (`RTP.*`), including
`loop-random-int-forms`; V3 compares generated UB-free programs with SBCL under
optimization-off, default, and `(SPEED 3)` policies. Both require zero
mismatches.

## FR-C optimization placement

| Requirement | Primary layer | Required order or boundary |
|---|---|---|
| C01 local calls and assignment conversion | HIR | Before closure layout and type-driven rewrites. Preserve lexical scope and assignment semantics. |
| C02 tail calls | HIR analysis, then MIR/codegen | Mark all tail positions after control-flow normalization; lower self, mutual, and general tail transfers without adding a frame when policy permits. |
| C03 closure representation | HIR escape/capture analysis, then MIR/codegen | Determine captures and escape before selecting flat/shared layout or lambda lifting. |
| C04 known-function database | HIR | Load facts before source transforms and calls are specialized. Failed constant folds keep the original call and emit a note. |
| C05 compiler macros | Expansion before HIR call lowering | Consult the lexical/global compiler-macro environment; obey `NOTINLINE`; preserve compile-time ordering. |
| C06 inlining | HIR | Require eligible function body, policy/cost approval, and non-`NOTINLINE`; rerun type/effect and CFG analyses after expansion. |
| C07 type inference | HIR | Run before checks and representation selection. Propagate typep, equality, comparison, ftype, range, and loop facts to a fixed point. |
| C08 safety and checks | HIR checks, then MIR lowering | Insert checks after inference, then rerun HIR simplification; omit only when proven redundant or explicitly trusted under the selected policy. SBCL's `checkgen` flow also reoptimizes after check insertion (`src/compiler/main.lisp:546-564`). |
| C09 representation selection | HIR facts, MIR physical form | Use HIR type facts to select unboxed candidates; make actual machine representations and boxing conversions at lowering. |
| C10 arithmetic | HIR transforms, then MIR | Specialize generic arithmetic by proven types and arity; preserve overflow and bignum fallback. Resolve operators by symbol identity, not printed name. |
| C11 sequences and arrays | HIR transforms, then MIR | Specialize only with proven sequence/element types; resolve constant keyword arguments; let safety policy control bounds checks. |
| C12 control and multiple values | HIR control, then MIR/codegen | Preserve multiple-value and NLX semantics; select jump tables only when type range, density, policy, and target favor them. |
| C13 allocation | HIR escape/lifetime analysis, then MIR/codegen | Honor `DYNAMIC-EXTENT`; stack-allocate only proven non-escaping values; lower TLAB allocation. Omit a write barrier only for a raw non-managed slot, as specified by the [GC contract](gc-interface.md); every managed reference store writes the slot and then calls the barrier. |
| C14 loops | HIR analysis and transforms, then MIR | Recognize loops and induction variables before LICM, strength reduction, and bounds-check removal. Unswitching and unrolling remain optional. |
| C15 global SSA optimization | HIR and MIR | HIR owns semantic SCCP/GVN/copy propagation/jump threading; MIR owns target-aware cleanup and peepholes. Preserve effects and error paths. |
| C16 CLOS and structures | HIR transforms plus runtime | Specialize proven slots, accessors, constructors, and dispatch sites. Runtime owns mutable generic-function and effective-method caches. |
| C17 calling convention | HIR call analysis, then MIR/codegen | Separate public/fdefn entry from statically known internal entry; avoid rest-list construction for supported optional/key calls. |
| C18 block compilation | Compilation-unit HIR | Index definitions across the `WITH-COMPILATION-UNIT` or `:BLOCK-COMPILE` extent while retaining per-form arenas and source origins. |
| C19 SIMD API (optional) | HIR API nodes, then target MIR/codegen | Preserve target-independent operation semantics; select x86-64 AVX2 or AArch64 NEON only when supported. |
| C20 PGO (optional) | Runtime profile data and HIR/MIR | Collect versioned profile data separately from compilation; use it only when source, compiler, policy, and target keys match. |

MIR may refine a machine operation only from facts that HIR proved. When HIR inserts a check, later MIR passes may
remove or fold its machine sequence only when they preserve the same condition
and control-flow behavior.

## Backend requirements FR-D01 through FR-D03

| Requirement | Contract |
|---|---|
| FR-D01 register allocation | Use linear scan with live-range splitting as the default. At `(SPEED 3)`, use graph coloring. Reuse spill slots, coalesce moves, and assign call-crossing values to callee-saved registers only when the GC map remains correct. The register-allocation checker is the test oracle. |
| FR-D02 instruction selection | Fold legal addresses and immediates, fuse comparisons with branches where the ISA permits, then run a target-aware peephole pass. These are MIR/backend transformations and cannot change HIR error behavior. |
| FR-D03 GC maps | Every safepoint map distinguishes tagged pointers from unboxed scalars. An unboxed value is never marked as a heap pointer. A potentially live tagged value remains visible to the collector across allocation and calls. |

Decision D-14 keeps linear scan with splitting as the default and graph
coloring at `(SPEED 3)`. This is an NCL decision, not a description of SBCL's
exact allocator. SBCL 2.6.10 uses adaptive allocation: its default is greedy,
and a `(SPEED 3)` block selects iterative Chaitin-Briggs allocation
(`src/compiler/pack.lisp:1729-1779`,
`src/compiler/pack-iterative.lisp:1-45`). Before changing D-14, compare the
backtracking allocator such as regalloc2 and measure:

- compile time and allocator memory on the fixed compile corpus;
- spills, spill-slot reuse, move count, and generated code size;
- runtime of the same micro suites on x86-64 and AArch64;
- the distribution of live-range lengths and call-crossing values.

Record the CPU, OS, compiler revision, and corpus with the measurements. Run
both allocators on the same host and interleave at least five A/B runs. Treat a
regression as confirmed only when both the candidate median and minimum are at
least 10% slower.

## SBCL 2.6.10 source comparison

The comparison below comes from the `sbcl-2.6.10` release tag source under
`src/compiler/` and related compiler files. SBCL was not executed. The source
shows concrete implementation choices; it does not require NCL to copy their
algorithms or file organization.

| FR-C | SBCL source read | NCL contract |
|---|---|---|
| C00 | `src/compiler/node.lisp`, `src/compiler/ir1tran.lisp`, `src/compiler/ir2tran.lisp` | Separate semantic HIR and machine-oriented MIR. |
| C01 | `src/compiler/locall.lisp`, `src/compiler/ir1tran.lisp` | HIR local-call, assignment-conversion, and contification analyses. |
| C02 | `src/compiler/ir1opt.lisp`, `src/compiler/ir2tran.lisp` | Mark tail positions in HIR; select tail transfers in MIR/codegen. |
| C03 | `src/compiler/locall.lisp`, `src/compiler/ir1tran.lisp`, `src/compiler/envanal.lisp` | Analyze captures and escape before choosing closure representation. |
| C04 | `src/compiler/knownfun.lisp:124-188,196-245`, `src/compiler/fndb.lisp:225-249`, `src/compiler/macros.lisp:260-370` | Keep function facts and guarded transforms in HIR; preserve calls when folding is invalid. |
| C05 | `src/compiler/ir1tran.lisp:760-831`, `src/compiler/main.lisp:1070-1089` | Apply compiler macros at expansion with lexical environment and NOTINLINE handling. |
| C06 | `src/compiler/ir1tran.lisp:1115-1150`, `src/compiler/ir1opt.lisp:1681-1735`, `src/compiler/ir1util.lisp:3310-3326` | HIR inlining with policy, cost, and expansion limits. |
| C07 | `src/compiler/ctype.lisp`, `src/compiler/constraint.lisp:48-60,804,1744,1877`, `src/compiler/constraint-back.lisp`, `src/compiler/equality-constraints.lisp` | Use a CL type lattice with uncertain subtype answers and fixed-point branch propagation. |
| C08 | `src/compiler/checkgen.lisp:656-705`, `src/compiler/policies.lisp:20-122`, `src/compiler/main.lisp:546-564` | Generate HIR checks after inference, then re-optimize; keep type-check and array-bound policy decisions explicit. |
| C09 | `src/compiler/represent.lisp:231-311,1022-1086`, `src/compiler/ir2tran.lisp`, `src/compiler/ir2opt.lisp` | Keep CL type reasoning in HIR; select physical forms at lowering. |
| C10 | `src/compiler/srctran.lisp`, `src/compiler/integer-tran.lisp`, `src/compiler/modarith.lisp:399-429` | HIR arithmetic transforms plus MIR target selection and overflow fallback. |
| C11 | `src/compiler/seqtran.lisp:140-168,325-419,935-1060,2118-2229,2624-2647,3399-3435,4403-4420`, `src/compiler/array-tran.lisp:2582-2629` | Split sequence-level transforms from array layout and element access lowering. |
| C12 | `src/compiler/srctran.lisp:10516-10670`, `src/compiler/ir2tran.lisp:711-761` | Select jump tables using type interval, density, policy, and target. |
| C13 | `src/compiler/envanal.lisp:352-475`, `src/compiler/ltn.lisp:608-619`, `src/compiler/stack.lisp:121-188`, `src/compiler/ir2opt.lisp:1677-1709`, `NEWS:153-155` | Stack-allocate only when escape analysis proves the lifetime. SBCL 2.6.9 NEWS records automatic stack allocation for nonescaping self- or mutually-recursive closures. |
| C14 | `src/compiler/loop.lisp:63-106`, `src/compiler/main.lisp:732-744`, `src/compiler/control.lisp:66-122`, `src/compiler/constraint.lisp:1377-1414` | Treat requested LICM, induction-variable, strength-reduction, and bounds-check passes as NCL requirements; the inspected files do not establish all as standalone SBCL passes. |
| C15 | `src/compiler/ir1opt.lisp`, `src/compiler/copyprop.lisp`, `src/compiler/constraint.lisp`, `src/compiler/ir2opt.lisp` | Use HIR semantic optimizations and separate MIR target cleanup. |
| C16 | `src/pcl/dfun.lisp`, `src/pcl/cache.lisp`, `src/pcl/ctor.lisp`, `src/pcl/compiler-support.lisp` | Specialize proven slot and constructor sites in HIR; keep mutable dispatch caches in the CLOS runtime. |
| C17 | `src/compiler/callable-args.lisp`, `src/compiler/ir1tran.lisp`, `src/compiler/ir2tran.lisp` | Separate checked public entries from known internal calling paths. |
| C18 | `src/compiler/main.lisp:73-90`, `src/compiler/ir1tran.lisp` | Add a compilation-unit HIR index over per-form arenas. |
| C19 | `src/compiler/x86-64/simd-pack*.lisp`, `src/compiler/arm64/simd-pack.lisp` | Keep SIMD optional and target-specific below the semantic API. |
| C20 | No `PGO`, profile-guided, or profile-feedback match was found in the inspected SBCL `src/compiler/` tree (`rg` exit 1). | Treat NCL PGO as an optional profile collection and feedback workflow, not a verified SBCL compiler subsystem. |

## Verification contract

Every candidate runs V4: HIR/MIR verifier checks, the register-allocation
checker, and an opt-bisect test that can isolate a deliberately introduced
bad pass. Every optimization lane also supplies V7 evidence: the relevant
micro suite plus generated-code inspection for instructions and allocation
byte counts. V7 satisfies AC-3: cl-bench's geometric mean is no slower than
SBCL, no individual benchmark exceeds 1.5x SBCL, and the optimization-specific
micro suites and generated-code assertions pass. The micro suites cover float
and fixnum loops, CLOS dispatch, structures, arrays, consing, tail recursion,
closures, and `&KEY` calls.

| Evidence | Required result |
|---|---|
| HIR/MIR verifier | Zero invariant violations after each pass. |
| Register-allocation checker | Zero invalid assignments or GC-map mismatches; it is the oracle for FR-D01. |
| Opt-bisect | A controlled bad pass is isolated by invocation-prefix search over the stable IDs and fixed-point trace. |
| Generated code | The expected specialization, check, branch, allocation, or unboxed instruction is present or absent as asserted by the requirement. |
| Allocation measurement | Allocation-byte assertions cover zero-allocation requirements and stack/TLAB allocation requirements. |
| Runtime comparison | Compare V7 micro suites and cl-bench with the same-day SBCL baseline under AC-3. |
| Compiler performance | V8 measures fixed-corpus compile time, per-pass time, peak RSS, startup time, and binary size. |

## L3 implementation slices

L3 names the Wave 3 optimization lanes in the
[wave plan](../project/wave-plan.md). Lane IDs such as L0-3, L1-4, and L2-ch7
refer to dependencies in that plan. L3-1 lands first and freezes the HIR
contract before the other optimization lanes start. L3-1a through L3-1e are
sequential commits in the listed order; each depends on the preceding slice.
L3-2 through L3-15 start after L3-1e completes. Paths name bounded contexts; final crate
names follow the [crate map](crates.md).

| Order | Owned paths | Completion condition and focused evidence |
|---|---|---|
| L3-1a: arena and IDs | `packages/compiler/hir/src/arena.rs`, `packages/compiler/hir/src/ids.rs`, `packages/compiler/hir/tests/arena.rs` | One arena per top-level form; typed indices reject invalid bounds. Test arena ownership, index reuse rules, and overflow handling. |
| L3-1b: semantic nodes and bindings | `packages/compiler/hir/src/node.rs`, `packages/compiler/hir/src/binding.rs`, `packages/compiler/hir/src/control.rs`, `packages/compiler/hir/tests/bindings.rs` | Node kinds preserve calls, assignments, local function scope, multiple values, cleanup, and non-local exits. Use the specified structured-node plus CFG-block representation. Test `FLET`/`LABELS` visibility and binding identity. |
| L3-1c: CL type model | `packages/compiler/hir/src/cl_type.rs`, `packages/compiler/hir/src/cl_type/numeric.rs`, `packages/compiler/hir/src/cl_type/compound.rs`, `packages/compiler/hir/src/cl_type/relation.rs`, `packages/compiler/hir/tests/cl_type.rs` | Keep `cl_type.rs` as a facade below 500 lines. Map known `ncl-types::TypeSpecifier` forms, retain unresolved forms opaquely, and test shared type meanings plus yes/no/unknown subtype results. |
| L3-1d: policy, locations, and verifier | `packages/compiler/hir/src/policy.rs`, `packages/compiler/hir/src/source.rs`, `packages/compiler/hir/src/verify.rs`, `packages/compiler/hir/tests/verifier.rs` | Verifier checks indices, scope, CFG, source origins, fact epochs, and check edges after every pass. Type-fact soundness is verified by V3. Release behavior matches D-10. |
| L3-1e: front-end builder and MIR bridge | `packages/compiler/hir/src/lib.rs`, `packages/compiler/front/src/hir_builder.rs`, `packages/compiler/front/src/hir_builder/bindings.rs`, `packages/compiler/front/src/hir_builder/control.rs`, `packages/compiler/front/src/hir_builder/calls.rs`, `packages/compiler/front/src/hir_builder/types.rs`, `packages/compiler/front/src/hir_builder/declarations.rs`, `packages/compiler/hir/src/lower.rs`, `packages/compiler/front/tests/hir_builder.rs`, `packages/compiler/front/tests/fixtures/hir-bridge/` | Keep `hir_builder.rs` as a facade below 500 lines. L3-1e solely owns `ncl-hir`'s `lib.rs` and exports the L3-1 modules. The checked-in fixture corpus covers every emitted HIR node family, including bindings, local/global/indirect calls, declarations, multiple values, cleanup, and non-local exits. Compare results and condition outcomes with baseline outputs; run V4 verification through HIR and MIR. |

The remaining L3 lanes own non-overlapping modules. L3-2 solely owns
`packages/compiler/opt/src/hir.rs`, the pass registry root and the ordered
schedule. Other lanes add implementations and pass constructors only in their
owned modules; they do not edit the registry root. L3-2 performs the registry
integration after the pass modules are ready. `packages/compiler/hir/src/lib.rs`
is solely owned by L3-1e, which integrates module exports after the earlier
L3-1 modules land. Integration tests use lane-prefixed paths such as
`packages/compiler/opt/tests/l3_2_*.rs`; private invariant tests stay beside
their module. Prefixes are unique within each crate. Split large
`type_inference.rs` implementations into its facade and named files under
`type_inference/`. No lane uses `mod.rs`. All rows use V4 as a candidate
requirement. Each optimization row also uses V7 generated-code inspection and
its requirement-specific suite.

| Lane | Requirement | Owned paths | Depends on | Additional V4/V7 evidence |
|---|---|---|---|---|
| L3-2 | C01-C03 | `packages/compiler/opt/src/hir.rs`, `packages/compiler/opt/src/hir/local_calls.rs`, `packages/compiler/opt/src/hir/tail_calls.rs`, `packages/compiler/opt/src/hir/closures.rs`, `packages/codegen/src/tail_call.rs` | L3-1 | Sole owner of pass registry root and schedule; integrate lane pass constructors in stable-ID order. Ten-million-step tail recursion, mutual-tail calls, nonescaping `FLET` allocation, closure micro suite. |
| L3-3 | C04, C05, C10, C11 | `packages/compiler/opt/src/hir/known_functions.rs`, `packages/compiler/opt/src/hir/source_transforms.rs`, `packages/compiler/opt/src/hir/arithmetic.rs`, `packages/compiler/opt/src/hir/sequences.rs`, `packages/compiler/front/src/compiler_macro_lisp.rs`, `packages/runtime/src/lib.rs` | L3-1 | Own the runtime compile entry's compiler-macro registry initialization (`packages/runtime/src/lib.rs:190`). `(+ 1 2)` folds; failing folds retain calls and notes; compiler-macro expansion and `NOTINLINE`; arithmetic/sequence code inspection. |
| L3-4 | C07-C08 | `packages/compiler/opt/src/hir/type_inference.rs`, `packages/compiler/opt/src/hir/type_inference/transfer.rs`, `packages/compiler/opt/src/hir/type_inference/refine.rs`, `packages/compiler/opt/src/hir/type_inference/constraints.rs`, `packages/compiler/opt/src/hir/type_inference/worklist.rs`, `packages/compiler/opt/src/hir/constraints.rs`, `packages/compiler/opt/src/hir/checkgen.rs`, `packages/compiler/opt/tests/l3_4_type_inference.rs` | L3-1 | Keep `type_inference.rs` as a facade below 500 lines. Own stage H130 check generation. Run type-soundness cases and V2 RTP before the next dependent optimizer; inspect safety-policy guards. |
| L3-5 | C06, C18 | `packages/compiler/opt/src/hir/inlining.rs`, `packages/compiler/hir/src/compilation_unit.rs` | L3-1, L3-3 | The compilation-unit index lives in `ncl-hir`, not runtime. Test same-unit cross-form inlining; declared inline call disappears from generated code; `NOTINLINE` remains a call. |
| L3-6 | C09, FR-D03 | `packages/ir/src/types.rs`, `packages/compiler/opt/src/hir/representation.rs`, `packages/codegen/src/lowering.rs`, `packages/codegen/src/safepoint.rs`, `packages/codegen/src/target_x86_64_lowering.rs`, `packages/codegen/src/target_aarch64_lowering.rs`, `packages/codegen/src/target_x86_64_lowering/representation.rs`, `packages/codegen/src/target_aarch64_lowering/representation.rs` | L3-4 | Sole owner of new scalar MIR types, shared representation/safepoint lowering, and target-lowerer dispatch roots. L3-7 and L3-10 add only feature-specific target modules and submit dispatch integration to L3-6. Double loop allocates zero bytes; generated scalar operations are unboxed; GC maps classify scalar values as non-pointers. |
| L3-7 | C13 | `packages/compiler/opt/src/hir/escape.rs`, `packages/codegen/src/allocation.rs`, `packages/codegen/src/target_x86_64_lowering/allocation.rs`, `packages/codegen/src/target_aarch64_lowering/allocation.rs` | L3-2, L3-6, L1-4 | Add allocation-only target lowerers; do not edit `lowering.rs` or `safepoint.rs`. Stack/TLAB allocation byte counts, escape cases that remain heap allocated, and GC stress. |
| L3-8 | C14-C15 | `packages/compiler/opt/src/hir/loops.rs`, `packages/compiler/opt/src/hir/global_value_numbering.rs`, `packages/codegen/src/mir_opt.rs` | L3-4 | Loop and SSA micro suites; inspect eliminated bounds checks, repeated loads, and dead branches. |
| L3-9 | C16 | `packages/clos/**`, `packages/compiler/opt/src/hir/clos.rs` | L2-ch7, L3-3 | Dispatch microbench is within 1.5x SBCL; inspect specialized slot, accessor, and constructor code. |
| L3-10 | C12, C17 | `packages/compiler/front/src/hir_calls.rs`, `packages/codegen/src/call_abi.rs`, `packages/codegen/src/control.rs`, `packages/codegen/src/target_x86_64_lowering/calls.rs`, `packages/codegen/src/target_aarch64_lowering/calls.rs`, `packages/codegen/src/target_x86_64_lowering/control.rs`, `packages/codegen/src/target_aarch64_lowering/control.rs` | L3-1, L3-6 | Add call/control-only target modules; do not edit shared lowering, safepoint, or target-dispatch roots. `&KEY` calls allocate zero bytes; inspect fixed-arity entries, multiple values, jump tables, and NLX paths. |
| L3-11 | FR-D01, FR-D02 | `packages/codegen/src/regalloc.rs`, `packages/codegen/src/instruction_select.rs`, `packages/codegen/tests/regalloc.rs` | L3-6 | Register checker is the oracle; inspect folded addresses/immediates, compare-branch fusion, and peephole output. |
| L3-12 | Diagnostics (E01-E04) | `packages/compiler/front/src/compiler_diagnostics.rs`, `packages/runtime/src/compile_diagnostics.rs` | L3-4 | Warning and failure flags match the diagnostic tests; inspect file, form, and line locations. |
| L3-13 | FASL and images (D05-D08) | `packages/objfile/src/**`, `packages/image/src/**`, `packages/runtime/src/image.rs`, `packages/disasm/src/**` | L1-3 | FASL/image round trip, disassembly, relocation, and target fingerprint tests. |
| L3-14 | Compiler metrics and cache (I01-I03) | `scripts/compiler-metrics/**`, `packages/compiler/opt/src/metrics.rs`, `packages/runtime/src/compile_cache.rs` | L0-3 | V8 fixed-corpus time, pass time, peak RSS, and cache-key invalidation evidence. |
| L3-15 | C19-C20 (optional) | `packages/compiler/opt/src/hir/profile_guided.rs`, `packages/runtime/src/profile.rs`, `packages/codegen/src/simd.rs`, `packages/codegen/src/simd_x86_64.rs`, `packages/codegen/src/simd_aarch64.rs` | L3-6 | Compare optional SIMD/PGO output with an unoptimized baseline; inspect target instruction selection and profile-key rejection. |

The paths in this table are ownership boundaries for the parallel lanes. A
lane must coordinate a contract change before editing another lane's path.
