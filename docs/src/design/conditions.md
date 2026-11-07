# Conditions and typed runtime errors

This contract covers FR-A01 condition mapping and restarts, FR-A02 guarded stacks, FR-A04 unhandled reports, and FR-A08 OS signal setup. L1-1 implements condition mapping, signaling, and reports; L1-2 implements signal and stack handling. L2-ch9 is the later Common Lisp chapter 9 condition-system work, and V5 is the run's verification layer that includes signal torture.

A bounded context here is a crate-owned subsystem such as the reader, FORMAT, numbers, streams, or runtime. It keeps its own typed error enum and payload. A context adapter preserves that error while mapping it to the shared Lisp error representation; `packages/conditions/src/conversion.rs` builds the condition object, and the runtime routes it through handler search. This adapter and signaling path is the condition boundary. Crate ownership follows `docs/src/design/crates.md`.

## Baseline on `6aa6639d`

The baseline is main revision `6aa6639d5e9bd104bf9ee54e08e4cd88441563a7`. The release binary was built from that tree with `cargo build --release --bin ncl` (exit 0). These probes ran against that binary, so they describe the baseline rather than later source changes:

| Probe | Exit | Observed output |
| --- | ---: | --- |
| `./target/release/ncl --eval '(car 1)'` | 1 | `ncl: native failure TypeMismatch { operation: Car, operand: 0, value: Word(0x2) }: Lisp(TypeError { datum: Word(0x2), expected: Cons })` |
| `./target/release/ncl --eval "(labels ((recur (n) (if (zerop n) n (recur (1- n))))) (recur 10000000))"` | 134 | Printed `fatal runtime error: stack overflow, aborting`; the shell reported `Abort trap: 6`. |
| `./target/release/ncl --eval '(let ((chunks (make-array 9))) (dotimes (i 9) (setf (aref chunks i) (make-array 1000000))) chunks)'` | 1 | `ncl: object error: Storage(CapacityExceeded)` |

The heap probe is bounded by the current 64 MiB `HeapConfig` default (`packages/sys/src/heap.rs:42-48`); allocation rejects a request beyond that capacity after minor and full collection retries (`packages/sys/src/heap/allocation.rs:162-178, 235-247`). `src/main.rs:19-25, 102-108` exposes `--eval` and `--optimization`; it has no `--dynamic-space-size` or `--control-stack-size` option.

The `(car x)` expression lowers to `OpKind::Builtin` (`packages/compiler/front/src/lower/expr.rs:235-253`) and follows checked-call lowering on x86-64 and AArch64 (`packages/codegen/src/target_x86_64_lowering/checked_ops.rs:253-276`, `packages/codegen/src/target_aarch64_lowering/ops.rs:374-393`). A direct native type mismatch is represented as a typed `NativeFailure` with a Lisp `TypeError` (`packages/runtime/src/native_error.rs:18-52`), but the native-entry path returns pending native errors from `invoke_compiled` before condition conversion or handler search (`packages/runtime/src/lib.rs:407-410`). The CLI prints that `RuntimeError` and exits 1 (`src/main.rs:43-45`), which explains why the release probe does not return the datum through a Lisp handler.

The recursive probe overflows the host main-thread stack and aborts before the runtime can deliver a Lisp condition. Current Lisp threads use `std::thread::Builder` without a configured stack size (`packages/threads/src/thread.rs:133-150`), and the discovered stack bounds are published for GC scanning (`packages/sys/src/thread.rs:179-181, 219-230`); there is no guard-recovery path in the current signal boundary (`docs/src/design/signals.md:9-11`).

The heap probe reaches `ObjectError::Storage`, but that object error is not converted by the current pending-condition path. `pending_object_error` consumes a pending condition only for `UndefinedFunction` and `TypeError`, then returns other object errors unchanged (`packages/runtime/src/native_error.rs:108-149`). The separate `LispError::Object(ObjectError::Storage(_))` mapping to `STORAGE-CONDITION` exists in the condition converter (`packages/conditions/src/conversion.rs:174-177`), but the probe does not enter it; `RuntimeError::Object` formats as `object error: …` and `--eval` prints the runtime error and exits 1 (`packages/runtime/src/error.rs:47-56`, `src/main.rs:43-45`).

Safe builtin arity checks set `LispError::ProgramError(WrongNumberOfArguments)` with the minimum and maximum (`packages/object/src/function_call.rs:307-322`). Runtime converts pending Lisp errors to conditions and the builtin trampoline invokes the handler search (`packages/runtime/src/function_call.rs:140-153`, `packages/runtime/src/builtin_trampoline_dispatch.rs:115-155`). The conversion currently includes the bounds even though `PROGRAM-ERROR` defines no standard arity slots; see the mapping rule below.

## Error-to-condition mapping

The table names standard data, not NCL report fields. `PROGRAM-ERROR` and `STORAGE-CONDITION` have no standard slots. NCL may keep additional diagnostic data in a separate typed report payload, but must not present it as a CLHS slot.

| Bounded-context failure | Condition and standard data | Restart contract |
| --- | --- | --- |
| A value does not satisfy an argument or declared type | `TYPE-ERROR`; preserve `:datum` and the full `:expected-type` type specifier. For a range violation, use a `TYPE-ERROR` whose expected type expresses the range; CLHS defines no `range-error` class. | `TYPE-ERROR` alone does not require a restart. |
| A safe call has too few or too many arguments, or a call has invalid program-level arguments | `PROGRAM-ERROR`; keep function designator, actual count, and expected bounds in the internal diagnostic/report payload. CLHS defines no standard arity slots. | No named restart is required. |
| A variable or function cell is missing | `UNBOUND-VARIABLE` or `UNDEFINED-FUNCTION`, respectively; preserve `:name` through `CELL-ERROR`. | The condition classes do not require `USE-VALUE` or `STORE-VALUE`. Offer one only where the operation has a valid continuation and the restart can implement it. |
| Arithmetic operation fails | `DIVISION-BY-ZERO` where applicable; otherwise `ARITHMETIC-ERROR` or its specific floating-point subclass. Preserve `:operation` and `:operands`. | No named restart is required. Floating-point trap behavior is implementation-dependent. |
| Input ends before a read completes | `END-OF-FILE` with the affected `:stream`, inherited from `STREAM-ERROR`. | No named restart is specified. |
| Reader syntax or token parsing fails | `READER-ERROR` with the affected `:stream`; the class inherits from both `PARSE-ERROR` and `STREAM-ERROR`. | No named restart is specified. |
| A stream operation fails | `STREAM-ERROR` with the affected `:stream`. Keep the stream object at the point where the failure occurs. | No named restart is specified. |
| Package lookup, mutation, or conflict fails | `PACKAGE-ERROR` with the affected `:package`. An `IMPORT` name conflict must be correctable, but CLHS does not prescribe a restart name. | Attach the operation's correcting restart around the operation; do not invent a standard restart name. |
| File operation fails | `FILE-ERROR` with the affected `:pathname`. | No named restart is specified. |
| Heap, control-stack, binding-stack, or alien-stack capacity is exhausted | `STORAGE-CONDITION`. The standard class has no slots and is a `SERIOUS-CONDITION`, not a subtype of `ERROR`. Keep resource kind, requested size, and configured limit in NCL's separate diagnostic payload. | No named restart is specified. |
| FORMAT detects an unsupported or malformed operation | CLHS defines no `FORMAT-ERROR` class and does not generally prescribe a condition or restart for the cases below. Preserve the typed `FormatError`; NCL may report a detected implementation error as `SIMPLE-ERROR` with format control and arguments, marked as NCL policy. | No CLHS restart is specified for these FORMAT cases. |

These classes and slots follow CLHS condition-type pages for `TYPE-ERROR`, `PROGRAM-ERROR`, `CELL-ERROR`, `ARITHMETIC-ERROR`, `STREAM-ERROR`, `END-OF-FILE`, `READER-ERROR`, `PACKAGE-ERROR`, `FILE-ERROR`, and `STORAGE-CONDITION`, and CLHS sections 3.5.1.2 and 3.5.1.3 for argument-count errors. Argument-count errors are `PROGRAM-ERROR` when the call is safe; unsafe-call consequences are undefined.

### Current mapping gaps

`packages/object/src/typed/error.rs:80-158` already defines typed categories for type, program, arithmetic, cell, package, stream, EOF, file, and simple errors. `packages/conditions/src/conversion.rs:98-180` maps several of them, but loses standard data in these cases:

- `ObjectError::TypeError` becomes `TYPE-ERROR` with `NIL` for both `DATUM` and `EXPECTED-TYPE`; callers must retain the actual value and expected type before this fallback.
- The arithmetic slots exist (`packages/conditions/src/builtin_slot_specs.rs:59-85`), but `condition_from_lisp_error` currently creates arithmetic conditions without operation or operands.
- Current stream, EOF, package, and file variants carry no stream, package, or pathname into the converter, so their standard slots are empty. Reader errors are still `RuntimeError::Read` and have no reader-error conversion at this boundary.
- Wrong-arity conversion passes minimum and maximum counts to condition construction, but `PROGRAM-ERROR` has no CLHS-defined slots. Keep those counts in report data instead of making them part of the standard condition API.
- `RuntimeError::Native(String)` remains in `packages/runtime/src/error.rs:9-45`; object-file conversion and executable-memory setup stringify typed errors (`packages/runtime/src/error.rs:82-85`, `packages/runtime/src/builtin_trampoline.rs:448-457`). Several such errors later collapse to `ObjectError::Layout` (`packages/runtime/src/error.rs:77-105`).
- `FormatError` is a typed enum (`packages/lib/format/src/executor.rs:11-19`), but registration turns missing arguments, invalid parameters, non-integers, and most printer errors into `ObjectError::TypeError` (`packages/lib/format/src/registration.rs:136-143`). Preserve the enum and its data across this boundary. CLHS 22.3.10.2 leaves missing-argument consequences undefined; `~D`, `~B`, `~O`, and `~X` use `~A` formatting for non-integer arguments. Do not signal `TYPE-ERROR` for that case.

## Signaling and control flow

The condition boundary must preserve the typed payload through the existing pending-error path:

1. A compiled call or checked operation returns to its Rust builtin/runtime boundary with a context error enum. Machine code must not turn the error into a string or jump across Rust frames.
2. The bounded-context adapter constructs a condition object with the standard class and slots in the table. The runtime stores the condition as pending data and routes it through the condition system.
3. `ncl_conditions::error` searches the dynamic handler chain (`packages/conditions/src/handler.rs:82-145`). A matching handler runs while its dynamic extent is active. `HANDLER-CASE` expands to `HANDLER-BIND` and a `BLOCK`; its clause uses `RETURN-FROM` to transfer control through the existing non-local exit and cleanup path (`packages/lib/macros/src/control/handler_case.rs:114-173`).
4. If no handler matches, retain the condition and report payload until the top-level boundary renders it. Restart invocation is a separate lookup on the active dynamic restart chain; signaling a condition must not silently fabricate or invoke a restart.

Here, a pending condition is the Lisp condition object stored on `ThreadContext`; a pending exit is the non-local control-transfer state carried through generated code. The trampoline performs handler search before generated control-transfer and cleanup dispatch. On an unhandled condition, `ncl_conditions::error` marks the non-local exit and the trampoline retains the condition for reporting (`packages/conditions/src/handler.rs:147-160`, `packages/runtime/src/builtin_trampoline_dispatch.rs:115-155`). The generated dispatcher handles `CATCH`, `UNWIND-PROTECT`, and `PROGV` regions (`packages/ir/src/types.rs:395-414`); it does not search condition handlers or restarts.

Both backends check `RuntimeErrorFlags` and `Pending` before returning from a checked call. If a pending non-local exit must run cleanup, they clear the repeated error flag and propagate the typed payload through the existing exit path; if there is no pending exit, they return through the backend epilogue. The x86-64 and AArch64 implementations differ in instructions and ABI lowering, not in condition classes or handler semantics (`packages/codegen/src/target_x86_64_lowering/dispatch.rs:276-345`, `packages/codegen/src/target_aarch64_lowering/dispatch.rs:335-441`). Tests must cover both backends. The raw `Prim::Car`/`Prim::Cdr` lowerers are a separate route and currently emit loads without a type test (`packages/codegen/src/target_x86_64_lowering/primitives.rs:186-193`, `packages/codegen/src/target_aarch64_lowering/primitives.rs:262-281`); that route must not bypass the type-error contract.

### Restart placement

Restart obligations belong to the operation that can resume, because only that operation knows the corrected value, place, and continuation. The condition converter constructs condition data; it does not install generic restarts.

| Operation | Required restart behavior |
| --- | --- |
| `CERROR` | Establish `CONTINUE`; invoking it resumes the `CERROR` call. The current `cerror` path already pushes and removes this restart (`packages/conditions/src/handler.rs:174-220`). |
| `CHECK-TYPE`, `CCASE`, `CTYPECASE` | Establish `STORE-VALUE` with the place to update. `ECASE` and `ETYPECASE` are non-correctable. These requirements are operation-specific, not a property of every `TYPE-ERROR`. |
| `USE-VALUE` | CLHS does not require any specified function to offer this restart. NCL may add it where a supplied value can safely resume the operation. |
| `UNBOUND-VARIABLE`, `UNDEFINED-FUNCTION` | Neither class alone requires `USE-VALUE` or `STORE-VALUE`; install a restart only at a call/read site with a defined continuation. |
| `IMPORT` name conflict | The error is correctable, but the restart name is unspecified. |

These rules follow CLHS `CERROR`, `CONTINUE`, `CHECK-TYPE`, `CCASE`, `CTYPECASE`, `ECASE`, `ETYPECASE`, `USE-VALUE`, `STORE-VALUE`, `IMPORT`, and the restart-condition association rules in chapter 9. The current `USE-VALUE`, `STORE-VALUE`, and `CONTINUE` builtins invoke an already-active restart (`packages/conditions/src/restart_builtins.rs:182-231`); they do not create one.

## Unhandled-condition reports

An unhandled report must show the condition class, its useful condition report, and a backtrace with Lisp and native frames. Keep these values structured until rendering:

- The pending error record retains the condition object and optional condition report (`packages/runtime/src/error.rs:27-33`). It has no backtrace payload. The existing report printer handles selected conditions and otherwise prints a generic class report (`packages/conditions/src/report.rs:95-127`).
- When handler search returns `Unhandled`, capture Lisp frames and native return addresses before generated frames unwind. The current code-object metadata stores function names and safepoint maps but initializes `source_locations` empty (`packages/runtime/src/lib.rs:276-287`); L1-1.3 must add the code/source mapping needed to identify Lisp frames.
- Store frames in a typed `BacktraceFrame` payload containing frame kind, program counter or code offset, function identity, and source location when available. Keep platform symbol resolution and native unwind details behind `ncl-sys`.
- The top-level boundary renders the condition's CLHS report and captured frames after control reaches a stable runtime boundary. A missing source line may be reported as a code offset; a capture failure must be explicit rather than silently dropping the backtrace.

Do not capture or format a backtrace in an OS signal handler. Handled conditions need not allocate an unhandled-error report.

## Error-string gate

Every production failure path must return a typed context error. `Display` and `format!` may render a report only after the runtime has a condition and its data; they must not construct the value passed through `Result` or pending-error state.

Add a focused source gate that fails on production error-boundary patterns such as `RuntimeError::Native(...)`, `Result<T, String>` in error-returning APIs, and `.map_err(|error| error.to_string())` or `format!(...)` that replaces a typed error. The gate should have fixtures for each forbidden pattern and a positive fixture for formatting a report from an intact condition. An allowlisted call site must leave the original typed error and its data available to condition/report handling; record the path and reason next to the allowlist entry. Do not broadly ban ordinary string formatting or error report text.

## Wave 1 adoption and implementation slices

L1-1 and L1-2 may proceed in parallel after this L0-2 contract is reviewed. Within each lane, complete the slices in order. Crate assignment follows `docs/src/design/crates.md`.

| Lane / slice | Paths | Completion evidence |
| --- | --- | --- |
| L1-1.1: retain typed context errors and condition data | `packages/object/src/typed/`, `packages/conditions/src/conversion.rs`, `packages/conditions/src/builtin_slot_specs.rs`, context error types in `packages/lib/format/`, `packages/reader/`, `packages/lib/numbers/`, `packages/lib/streams/`, and `packages/lib/pathnames/` | Mapping tests assert class and every standard slot; wrong-arity metadata remains report-only; the error-string gate passes. |
| L1-1.2: signal compiled errors through handlers and restarts | `packages/compiler/front/`, `packages/codegen/src/target_x86_64_lowering/`, `packages/codegen/src/target_aarch64_lowering/`, `packages/runtime/src/function_call.rs`, `packages/runtime/src/builtin_trampoline_dispatch.rs`, and `packages/conditions/src/handler.rs` | `(handler-case (car 1) (type-error (c) (type-error-datum c)))` returns `1`; safe arity, unbound-variable, and undefined-function cases signal their mapped classes; restart tests cover each required operation on both code-generation targets. |
| L1-1.3: report unhandled errors with both frame kinds | `packages/runtime/src/error.rs`, runtime report code, `packages/codegen/` frame/source metadata, and `packages/sys/` native unwind adapter | An unhandled type error prints its class and datum plus Lisp and native frames; code/source locations are populated and no error payload is reduced to a string. |
| L1-2.1: install the OS signal boundary | `packages/sys/src/os.rs`, new signal declarations under `packages/sys/src/`, and per-thread registration in `packages/threads/` | Unit checks cover disposition installation, per-thread masks, and alternate-stack registration on Linux and macOS for x86-64 and AArch64. |
| L1-2.2: reserve and guard runtime stacks | `packages/sys/src/thread/`, `packages/threads/src/thread.rs`, and runtime CLI setup | Control, binding, and alien stacks have registered bounds and guard pages; `--control-stack-size` configures the control stack. Boundary tests identify each guard without consuming unrelated memory. |
| L1-2.3: recover and deliver signals | `packages/sys/src/`, `packages/runtime/`, and signal torture tests | A caught control-stack `STORAGE-CONDITION` can be followed by the same overflow test in the same process; guard pages are restored. V5 signal torture passes, including unknown-fault fatal handling and the requested signal policies. |

FORMAT, reader, numbers, and streams follow the same boundary: keep each context enum through its registration/runtime adapter, preserve standard slots, and signal only at the Lisp condition boundary. The FORMAT lane already has `FormatError` (`packages/lib/format/src/executor.rs:11-19`); carry that enum and the offending directive/stream data to the mapper instead of converting it to `ObjectError::TypeError`.

The reader lane maps reader syntax failures to `READER-ERROR` and EOF to `END-OF-FILE`, retaining the input stream. Numeric operations retain operands and operation designators for arithmetic conditions. Stream operations retain the affected stream; filesystem operations retain the pathname. L2-ch9 extends Common Lisp chapter 9 condition-system facilities only after L1-1 has established this conversion and signaling boundary.
