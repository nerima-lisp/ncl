# Conditions and typed runtime errors

This contract defines condition mapping and restarts (FR-A01), guarded stacks (FR-A02), unhandled reports (FR-A04), and OS signal handling (FR-A08). L1-1 owns typed error conversion, signaling, and reports. L1-2 owns signal delivery and guarded stacks. Later Common Lisp condition-system work extends the shared signaling boundary. The run's V5 verification layer includes the signal-torture cases in `docs/src/design/signals.md`.

A bounded context owns its error enum and payload. Its adapter preserves typed data through the runtime signal entry, which builds the standard condition slots and invokes condition handler search. A report may add NCL-specific diagnostics, but must not replace or reinterpret standard slots. Crate ownership follows `docs/src/design/crates.md`.

## Baseline on `6aa6639d`

The release binary was built from main revision `6aa6639d5e9bd104bf9ee54e08e4cd88441563a7` with `cargo build --release --bin ncl` (exit 0). These commands ran against that binary:

| Probe | Exit | Observed output |
| --- | ---: | --- |
| `./target/release/ncl --eval '(car 1)'` | 1 | `ncl: native failure TypeMismatch { operation: Car, operand: 0, value: Word(0x2) }: Lisp(TypeError { datum: Word(0x2), expected: Cons })` |
| `./target/release/ncl --eval "(labels ((recur (n) (if (zerop n) n (recur (1- n))))) (recur 10000000))"` | 134 | `fatal runtime error: stack overflow, aborting`; the shell reported `Abort trap: 6`. |
| `./target/release/ncl --eval '(let ((chunks (make-array 9))) (dotimes (i 9) (setf (aref chunks i) (make-array 1000000))) chunks)'` | 1 | `ncl: object error: Storage(CapacityExceeded)` |

The heap probe is bounded by the 64 MiB `HeapConfig` default (`packages/sys/src/heap.rs:42-48`); allocation rejects a request beyond capacity after minor and full collection retries (`packages/sys/src/heap/allocation.rs:162-178, 235-247`). `src/main.rs:19-25, 102-108` exposes `--eval` and `--optimization`, but no `--dynamic-space-size` or `--control-stack-size` option.

For `(car 1)`, the front end lowers `(car x)` to `OpKind::Builtin` (`packages/compiler/front/src/lower/expr.rs:235-253`). Checked-call lowering is in the x86-64 and AArch64 backends (`packages/codegen/src/target_x86_64_lowering/checked_ops.rs:253-276`, `packages/codegen/src/target_aarch64_lowering/ops.rs:374-393`). `native_failure` can construct a typed `LispError::TypeError` (`packages/runtime/src/native_error.rs:18-52`), but `invoke_compiled` returns pending native errors as `RuntimeError::NativeFailure` before condition conversion and handler search (`packages/runtime/src/lib.rs:396-435`). The CLI prints the runtime error and exits 1 (`src/main.rs:43-45`). Direct `Prim::Car` and `Prim::Cdr` lowering is another path and currently emits unchecked loads (`packages/codegen/src/target_x86_64_lowering/primitives.rs:186-193`, `packages/codegen/src/target_aarch64_lowering/primitives.rs:262-281`). Neither path reaches a Lisp `TYPE-ERROR` handler.

The recursive probe overflowed on the Rust main-thread stack and aborted before NCL could signal a condition. Rust std's Unix stack-overflow handler recognizes its own guard and aborts; for other faults it restores the default disposition and returns so the fault is delivered again (`library/std/src/sys/pal/unix/stack_overflow.rs:78-90, 107-135`, source revision `52d0866`). NCL has no signal-recovery API today: `packages/sys/src/os.rs:9-72` only declares memory mapping, pthread identity, stack discovery, and loader APIs. Lisp-created threads use `std::thread::Builder` without configuring the host stack size (`packages/threads/src/thread.rs:133-150`). Their pthread bounds are currently published for GC scanning, not exhaustion recovery (`packages/sys/src/thread.rs:179-181, 219-230`).

The heap probe reaches `ObjectError::Storage`, but `pending_object_error` only consumes pending conditions for `UndefinedFunction` and `TypeError`, then returns other object errors unchanged (`packages/runtime/src/native_error.rs:108-149`). The separate `LispError::Object(Storage)` converter arm exists (`packages/conditions/src/conversion.rs:174-177`), but the probe does not reach it; the CLI prints `RuntimeError::Object` and exits 1 (`packages/runtime/src/error.rs:47-56`, `src/main.rs:43-45`). A full heap also cannot be assumed to have room to allocate the `STORAGE-CONDITION` object. L1-4 owns a preallocated condition object and fixed diagnostic reserve for this path.

Other failures are also erased before reporting: `packages/runtime/src/function_call.rs:148` maps a native conversion failure to `ObjectError::Layout`, and `packages/runtime/src/native_error.rs:48` maps `NativeError::Invalid` to `ObjectError::Layout`. Preserve each originating typed error through conversion instead of using the generic layout category.

## Error-to-condition mapping

The table names standard condition data. `PROGRAM-ERROR` and `STORAGE-CONDITION` have no standard slots. Additional NCL diagnostics stay in a separate typed report payload.

| Bounded-context failure | Condition and standard data | Restart contract |
| --- | --- | --- |
| A value does not satisfy an argument or declared type | `TYPE-ERROR`; preserve `:datum` and the complete `:expected-type` type specifier. A range violation is a `TYPE-ERROR` whose expected type expresses the range; CLHS has no `RANGE-ERROR` class. | `TYPE-ERROR` alone does not require a restart. |
| A safe call has too few or too many arguments, or invalid keyword/program arguments | `PROGRAM-ERROR`; retain function designator, actual count, and expected bounds in typed report data. CLHS defines no standard arity slots. | No named restart is required. |
| A variable or function cell is missing | `UNBOUND-VARIABLE` or `UNDEFINED-FUNCTION`; preserve `:name` through `CELL-ERROR`. | The class alone does not require `USE-VALUE` or `STORE-VALUE`. Offer a restart only at an operation with a valid continuation. |
| Arithmetic operation fails | `DIVISION-BY-ZERO` where applicable; otherwise `ARITHMETIC-ERROR` or a specific floating-point subclass. Preserve `:operation` and `:operands`. | No named restart is required. Floating-point trap behavior is implementation-dependent. |
| Input ends before a read completes | `END-OF-FILE` with the affected `:stream`, inherited from `STREAM-ERROR`. | No named restart is specified. |
| Reader syntax or token parsing fails | `READER-ERROR` with the affected `:stream`; this inherits from `PARSE-ERROR` and `STREAM-ERROR`. | No named restart is specified. |
| A stream operation fails | `STREAM-ERROR` with the affected `:stream`. | No named restart is specified. |
| Package lookup, mutation, or conflict fails | `PACKAGE-ERROR` with the affected `:package`. An `IMPORT` name conflict must be correctable, but CLHS prescribes no restart name. | Attach the operation's correcting restart; do not invent a standard restart name. |
| File operation fails | `FILE-ERROR` with the affected `:pathname`. | No named restart is specified. |
| A FASL is malformed or incompatible | `NCL-EXT:INVALID-FASL` (package `NCL-EXT`), directly below `COMMON-LISP:ERROR`; its required `REASON` slot (`:reason`, type `KEYWORD`) contains the stable keyword paired with the typed `FaslRejectReason` variant in `docs/src/design/fasl.md:144-163`. Preserve the optional byte offset and structured parser diagnostic separately. File I/O remains `FILE-ERROR`. | No named restart is specified. |
| The process receives `SIGINT` | `NCL-EXT::INTERACTIVE-INTERRUPT`, directly below `SERIOUS-CONDITION`, with no standard slots. | No named restart is required. |
| Heap or managed control/binding-stack capacity is exhausted, or an alien stack overflows at a registered abort-safe FFI boundary | `STORAGE-CONDITION`. It has no standard slots and is a `SERIOUS-CONDITION`, not a subtype of `ERROR`. Keep resource kind, requested size, and configured limit in NCL's typed diagnostic record. | No named restart is specified. |
| FORMAT detects an unsupported or malformed operation | CLHS defines no `FORMAT-ERROR` class and prescribes no general condition or restart for a malformed or unsupported FORMAT operation. Preserve `FormatError`; NCL may report a detected implementation error as `SIMPLE-ERROR` with format control and arguments, marked as NCL policy. | No CLHS restart is specified for these FORMAT cases. |

These mappings follow the CLHS condition type pages and the condition/error sections. For arity and keyword checks, use the safety matrix in `docs/src/design/hir.md:152-169`: effective `SAFETY 1` or `SAFETY 3` makes the call safe; effective `SAFETY 0` makes it unsafe. A safe arity or keyword violation signals `PROGRAM-ERROR`; an unsafe call has undefined consequences, though NCL may still check it. The HIR policy separately controls declared-type, argument-count, keyword, and array-bound checks.

`NCL-EXT:INVALID-FASL` is assigned to `ncl-conditions` (`packages/conditions/ownership.tsv:72`) and registered in `NCL-EXT` (`packages/conditions/src/symbols/ncl_ext.rs:74-77`). Its direct superclass is `COMMON-LISP:ERROR` (`packages/conditions/src/hierarchy.rs:60`). The required `REASON` slot is initialized by `:reason`, has type `KEYWORD`, and contains the stable keyword paired with the typed `FaslRejectReason` variant in `docs/src/design/fasl.md:144-163`; do not derive it from a display string. Keep the optional byte offset and other parser details in a separate typed diagnostic payload.

### Common type-error signal entry

All type-check failures, including compiled `(car 1)` and future HIR checkgen failures, use one runtime entry:

```text
ncl_signal_type_error(datum, expected_type, source_origin)
```

- The runtime resolves the current `ThreadContext` from the generated-code ABI.
- `datum` is the offending Lisp `Word`; the entry roots it before any allocation.
- `expected_type` is a GC-rooted constant-pool `Word` denoting the complete Common Lisp type specifier. A named type is its symbol; a compound or range type is its complete type-specifier form. It is not an `ObjectType`, HIR lattice tag, class name, or string. The entry roots it while constructing the condition.
- `source_origin` identifies source metadata such as file, form, source span, and expansion origin. It is not a heap pointer; this lane's representation is specified below.

For L1-1.2, codegen passes the existing `DebugLocationId` as `source_origin` and adds no `ncl-ir` type. The current runtime cannot resolve this handle to a source location, so it signals the condition without one. L1-1.5 owns registering source-origin IDs in code metadata.

The entry replaces `LispError::TypeError { datum, expected: ObjectType }` with a typed payload that retains `datum`, the complete expected-type `Word`, and `source_origin`. The current variant is too narrow (`packages/object/src/typed/error.rs:84-87`). The condition converter places the exact values in `TYPE-ERROR`'s `:datum` and `:expected-type` slots. Native checked builtins translate their low-level mismatch into this entry with the canonical expected-type object; codegen guards and HIR checkgen call the same `ncl_signal_type_error` entry. Source metadata is currently initialized with an empty vector (`packages/runtime/src/lib.rs:276-287`); L1-1.5 registers origin IDs there so source spans resolve in compiled-origin reports, with code offsets available for origins lacking a source record.

The entry constructs the typed condition and routes it through the existing condition signal path. If condition allocation fails because the heap is exhausted, it selects the preallocated `STORAGE-CONDITION` reserved by L1-4.

## Signaling, non-local exits, and restarts

The error payload remains typed through these steps:

1. A compiled check or builtin returns to its runtime boundary with its typed error. Generated code does not stringify the error or jump across live Rust frames.
2. The context adapter preserves the payload. `ncl_signal_type_error` is the shared entry for type failures; other typed errors enter the same condition constructor and signaling path.
3. `ncl_conditions::error` searches the dynamic handler chain (`packages/conditions/src/handler.rs:82-145`). A matching handler runs while its dynamic extent is active. `HANDLER-CASE` expands to `HANDLER-BIND` and a block; the clause transfers with `RETURN-FROM` through the existing non-local exit and cleanup path (`packages/lib/macros/src/control/handler_case.rs:114-173`).
4. If no handler matches, retain the condition and typed report payload until the top-level boundary renders it. Restart lookup is separate; signaling never fabricates or invokes a restart.

The pending condition is a Lisp condition object stored on `ThreadContext`. `RuntimeError::PendingCondition` (`packages/runtime/src/error.rs:27-33`) specifically preserves a `SIMPLE-ERROR` across a nested runtime invocation; it is not a general pending-error record. A pending exit is the non-local control-transfer state carried through generated code. The builtin trampoline performs handler search before generated cleanup dispatch (`packages/runtime/src/builtin_trampoline_dispatch.rs:115-155`). Generated dispatch handles `CATCH`, `UNWIND-PROTECT`, and `PROGV` (`packages/ir/src/types.rs:395-414`); it does not search handlers or restarts. Both backends propagate pending exits through cleanup, with target-specific lowering (`packages/codegen/src/target_x86_64_lowering/dispatch.rs:276-345`, `packages/codegen/src/target_aarch64_lowering/dispatch.rs:335-441`).

| Operation | Required restart behavior |
| --- | --- |
| `CERROR` | Establish `CONTINUE`; invoking it resumes the `CERROR` call. Current `cerror` pushes and removes this restart (`packages/conditions/src/handler.rs:174-220`). |
| `CHECK-TYPE`, `CCASE`, `CTYPECASE` | Establish `STORE-VALUE` with the place to update. `ECASE` and `ETYPECASE` are non-correctable. |
| `USE-VALUE` | CLHS does not require a particular operation to offer it. NCL may add it where a supplied value resumes the operation safely. |
| `UNBOUND-VARIABLE`, `UNDEFINED-FUNCTION` | Neither class alone requires a restart; add one only at a read/call site with a defined continuation. |
| `IMPORT` name conflict | The conflict is correctable, but the restart name is unspecified. |

Restart obligations belong to the operation with the corrected value, place, and continuation. The converter only constructs condition data. Current `USE-VALUE`, `STORE-VALUE`, and `CONTINUE` builtins invoke an active restart; they do not create one (`packages/conditions/src/restart_builtins.rs:182-231`).

## Unhandled-condition reports

An unhandled report shows the condition class, its CLHS report, and a backtrace with Lisp and native frames. Keep all data structured until rendering:

- Capture Lisp frames and native return addresses when handler search returns `Unhandled`, before generated frames unwind. The current code-object metadata stores names and safepoint maps but initializes `source_locations` empty (`packages/runtime/src/lib.rs:276-287`). L1-1.3 defines the report data; L1-1.5 populates its runtime metadata.
- Store frames in a typed `BacktraceFrame` payload with frame kind, program counter or code offset, function identity, and source location when available. Keep native unwind and symbol lookup behind `ncl-sys`.
- Render at the top-level runtime boundary after control reaches a stable stack. Report an unavailable source line as a code offset; report capture failure explicitly rather than silently dropping frames.

## Error-string gate

Production error boundaries carry typed errors and diagnostic payloads. `Display` and `format!` may render a report after condition data is intact; they must not replace the error passed through `Result`, a pending slot, or a condition conversion.

L1-1.6 owns a source gate and fixtures after L1-1.1 through L1-1.5 establish the typed paths. The scan includes the full production error surface, not just L1-1.1's files: in particular `RuntimeError::Native(String)` creation in `packages/runtime/src/lib.rs:267-292`, and error-enum string payloads such as `LowerError::Ir { detail: String }` (`packages/compiler/front/src/lower/error.rs:29-32`, constructed in `packages/compiler/front/src/lower/expr.rs:264-267`). Detect message-shaped `String` payload variants, `Result<T, String>` error APIs, `.map_err(|e| e.to_string())`, and `format!` or `.to_owned()` conversions that replace typed errors. Include fixtures for each forbidden shape and a positive fixture for formatting an intact condition report. Domain data such as a pathname and report text is not an error-message payload; each exception must preserve the original typed error and name its path and reason in the allowlist.

## Wave 1 adoption and implementation slices

The paths below are file-level write ownership, not crate-level ownership. No path appears in two rows. L1-1's typed-error, signaling, report, and producer slices may run alongside L1-2's OS, stack, and recovery slices. The runtime module integration waits for both lanes' feature slices; the error-string gate runs last and scans read-only across the production tree. A scan does not grant write ownership. L1-1.1 exclusively owns `packages/object/src/typed/error.rs`; L1-4 waits for that slice and owns the preallocated `STORAGE-CONDITION` object and fixed diagnostic reserve in its GC/heap slice. L1-4's implementation paths are assigned by its own contract and are outside this table.

| Lane / slice | Owned paths | Depends on | Completion evidence |
| --- | --- | --- | --- |
| L1-1.1: typed context errors and slots | `packages/object/src/typed/error.rs`; `packages/conditions/src/conversion.rs`; `packages/conditions/src/builtin_slot_specs.rs`; `packages/compiler/front/src/lower/error.rs`; `packages/reader/src/error.rs`; `packages/lib/format/src/executor.rs`; `packages/lib/format/src/lib.rs`; new `packages/lib/numbers/src/error.rs`; new `packages/lib/streams/src/error.rs`; new `packages/lib/pathnames/src/error.rs` | L0-2 | Mapping tests assert the class and every standard slot; wrong-arity counts remain report-only; FORMAT retains `FormatError` and offending directive/stream data. |
| L1-1.2: compiled signaling and restarts | `packages/compiler/front/src/lower/expr.rs`; `packages/codegen/src/target_x86_64_lowering/checked_ops.rs`; `packages/codegen/src/target_x86_64_lowering/primitives.rs`; `packages/codegen/src/target_aarch64_lowering/ops.rs`; `packages/codegen/src/target_aarch64_lowering/primitives.rs`; new `packages/runtime/src/signal.rs`; `packages/runtime/src/native_error.rs`; `packages/runtime/src/function_call.rs`; `packages/runtime/src/builtin_trampoline_dispatch.rs`; `packages/conditions/src/handler.rs` | L1-1.1 | `(handler-case (car 1) (type-error (c) (type-error-datum c)))` returns `1`; compiled and HIR type checks use `ncl_signal_type_error`; safe-call arity and restart tests pass on both backends. |
| L1-1.3: unhandled reports and frames | `packages/runtime/src/error.rs`; new `packages/runtime/src/report.rs`; `packages/codegen/src/frames.rs`; new `packages/sys/src/unwind.rs` | L1-1.1 | Unhandled report prints class, datum, and Lisp/native frames; source locations resolve; no payload is reduced to text. |
| L1-1.4: legacy typed error producers | `packages/runtime/src/arith_fast.rs`; `packages/runtime/src/builtin_trampoline.rs`; `packages/runtime/src/builtin_trampoline_keywords.rs`; `packages/runtime/src/compile.rs`; `packages/runtime/src/load.rs`; `packages/runtime/src/load/support.rs`; `packages/runtime/src/load/top_level.rs`; `packages/runtime/src/optimization.rs`; `packages/runtime/src/support.rs` | L1-1.1 | Each producer preserves its bounded-context enum through `Result` and the runtime boundary; load and compile failures retain their typed category. |
| L1-1.5: runtime signal/report integration | `packages/runtime/src/lib.rs` | L1-1.2, L1-1.3, L1-1.4, L1-2.3 | Module and source-origin registration is wired once; `RuntimeError::Native(String)` construction and empty source-location metadata are removed. |
| L1-1.6: error-string gate | `scripts/check_error_strings.py`; new `scripts/tests/fixtures/error_strings/native_message.rs`; new `scripts/tests/fixtures/error_strings/string_error_result.rs`; new `scripts/tests/fixtures/error_strings/map_err_string.rs`; new `scripts/tests/fixtures/error_strings/typed_report.rs` | L1-1.1 through L1-1.5 | Fixtures reject message-shaped `String` payloads, `Result<T, String>`, `.map_err(|e| e.to_string())`, and string replacement; the positive fixture formats an intact typed report. The production scan passes with a reasoned allowlist. |
| L1-2.1: OS signal boundary | `packages/sys/src/os.rs`; new `packages/threads/src/signal_control.rs` | L0-2 | Signal dispositions, masks, sigwait event queue, and alternate-stack registration work on Linux and macOS for x86-64 and AArch64. |
| L1-2.2: mapped stacks and GC roots | `packages/sys/src/thread/stack.rs`; new `packages/sys/src/thread/stack_region.rs`; `packages/sys/src/thread.rs`; `packages/sys/src/heap/collect.rs`; `packages/threads/src/thread.rs`; `src/main.rs` | L0-2 | Every Lisp-running thread, including main, switches to a runtime-mapped control stack configured by `--control-stack-size`; GC walks Lisp maps from saved SP to mapped base without reading guard pages, and registered roots cover native/alien stacks; both mapping edges are guarded. |
| L1-2.3: guard recovery and signal delivery | new `packages/sys/src/signal.rs`; new `packages/sys/src/signal/fault.rs`; new `packages/sys/src/signal/context_linux_x86_64.rs`; new `packages/sys/src/signal/context_linux_aarch64.rs`; new `packages/sys/src/signal/context_macos_x86_64.rs`; new `packages/sys/src/signal/context_macos_aarch64.rs`; new `packages/sys/src/signal/trampoline_x86_64.rs`; new `packages/sys/src/signal/trampoline_aarch64.rs`; new `packages/runtime/src/signal_delivery.rs` | L1-2.1, L1-2.2 | Repeated caught control-stack overflow succeeds in one process after guard restoration; unknown faults remain fatal; SIGINT/TERM/CHLD are consumed by sigwait and delivered in normal runtime control. |
| L1-2.4: sys and thread module exports | `packages/sys/src/lib.rs`; `packages/threads/src/lib.rs` | L1-2.1 through L1-2.3 | Each new signal and stack API has one public export; no duplicate registry or OS boundary is added. |

FORMAT keeps `FormatError` (`packages/lib/format/src/executor.rs:11-19`) and does not map the `~D`, `~B`, `~O`, or `~X` non-integer fallback to `TYPE-ERROR`; CLHS formats those values with `~A`.
