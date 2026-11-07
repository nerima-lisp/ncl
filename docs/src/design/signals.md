# Signals and guarded stacks

This contract defines FR-A08 and the stack-recovery portion of FR-A02 for L1-2. It assigns OS signal handling to `ncl-sys`, with thread lifecycle and Lisp condition delivery in their owning contexts from `docs/src/design/crates.md`. A POSIX signal handler records a minimal event. It does not run Lisp code, allocate, format an error, search handlers, or invoke a restart.

## Current implementation

The baseline release exits with status 134 after a 10,000,000-call `LABELS` recursion overflows the Rust main-thread stack. The same process cannot catch that overflow or continue. The release probe and expression are recorded in `docs/src/design/conditions.md`.

`packages/sys/src/os.rs:9-72` declares memory mapping, pthread identity, and platform stack-discovery APIs. It has no `sigaction`, `sigaltstack`, or signal-mask declaration. The scoped search `rg -n 'sigaction|sigaltstack|pthread_sigmask|SIG(SEGV|BUS|PIPE|INT|TERM|CHLD|PROF)|--control-stack-size|--dynamic-space-size' packages/sys/src packages/threads/src packages/runtime/src src` found no signal-handler setup or stack-size option; its only signal-name hit is the `SIGTERM` convention comment in `packages/threads/src/process.rs:35`. The crate map lists `signal` under `ncl-sys` (`docs/src/design/crates.md:43,66`), but the current source has no signal-handler API. L1-2 adds that boundary under `packages/sys/src/`; reconcile the crate-map entry as part of that implementation.

`packages/sys/src/thread/stack.rs:4-42` discovers the current pthread bounds for GC scanning: macOS uses `pthread_get_stackaddr_np` and `pthread_get_stacksize_np`; Linux uses `pthread_getattr_np` and `pthread_attr_getstack`. The same file reads `rsp` on x86-64 and `sp` on AArch64 (`packages/sys/src/thread/stack.rs:44-75`). `packages/sys/src/thread.rs:179-181,219-230` publishes those bounds and a scan pointer. It does not set guards or recover from stack exhaustion. `packages/threads/src/thread.rs:133-150` creates Lisp threads with `std::thread::Builder` without configuring their stack size. The existing interrupt flag is cooperative and delivered at a safepoint (`packages/sys/src/thread.rs:301-320`); it has no OS signal producer.

The heap boundary belongs to the heap lane. `HeapConfig` currently defaults to 64 MiB and the allocator returns `StorageCondition::CapacityExceeded` after collection retries (`packages/sys/src/heap.rs:42-48`, `packages/sys/src/heap/allocation.rs:162-178,235-247`). The signal/stack work must route a typed storage failure to the condition boundary in `docs/src/design/conditions.md`; it must not duplicate heap sizing or collector policy. The `--dynamic-space-size` option remains with the heap lane.

## Signal ownership and handler rules

On POSIX systems, signal dispositions are process-wide, while signal masks and alternate signal stacks are per-thread (`pthreads(7)`, `pthread_sigmask(3p)`, and `sigaltstack(2)`). Install each disposition once during runtime initialization. Register a guarded alternate stack for every runtime thread before it can execute Lisp or generated code; do not assume a newly created thread inherits the creator's alternate stack. Use `pthread_sigmask` for per-thread mask changes.

Use `sigaction` with the information/context form for signals that need a fault address or interrupted machine context, and request the alternate stack for stack-fault handling. The OS/architecture adapter owns signal numbers, C struct layouts, register access, and context changes. A handler may write only to preallocated signal state using a mechanism verified safe on that target, and may use an async-signal-safe notification such as a nonblocking `write` where required. It must not allocate, lock, call `format!`, log, panic, access an uninitialized Rust thread-local, call into the Lisp condition system, or run exit hooks.

The normal runtime path drains recorded events at a safepoint or dedicated signal-control point. It constructs typed errors and Lisp conditions there, when allocation and ordinary runtime APIs are legal. Signal masks are restored on every exit path, including thread teardown and a non-local Lisp exit.

### Signal policy

| Signal | Handler-side action | Runtime-side action |
| --- | --- | --- |
| `SIGSEGV`, `SIGBUS` | Read the target context and fault address; record whether the address belongs to a registered control, binding, or alien stack guard. Redirect only a recognized guard fault to the recovery trampoline. | Recover a recognized guard fault as `STORAGE-CONDITION` after moving to a safe stack and unwinding to a recovery boundary. Preserve the guard. Unknown memory faults remain fatal. |
| `SIGPIPE` | Set the process-wide disposition to ignored during runtime setup; do not format or signal from a handler. | Translate a failed stream write (`EPIPE`) to `STREAM-ERROR` with its `:stream`; the write path owns the actual stream object. |
| `SIGINT` | Set a pending interrupt record for the receiving runtime thread. | At a safepoint, deliver the configured interactive-interrupt policy as a Lisp condition. A non-ANSI condition class, if used, is an NCL extension. |
| `SIGTERM` | Set a termination request. | Run registered exit hooks in normal runtime control, once, then exit. |
| `SIGCHLD` | Record that child state may have changed. | Reap children and update process records outside the handler. |
| `SIGPROF` | Copy only the sample data needed from the interrupted context into a fixed-capacity per-thread buffer, or increment a verified lock-free sample counter. | Drain and symbolize samples outside the handler. Lost samples must be counted rather than allocated or logged in signal context. |

`SIGSEGV` and `SIGBUS` are not interchangeable on every target. POSIX.1-2017 `signal(3p)` states that returning from a handler for a computational `SIGSEGV` has undefined behavior. Linux `signal(7)` documents architecture-dependent hardware-fault mapping; Linux AArch64 can report `SIGBUS` for some page faults. A handler that returns to the same faulting instruction without changing the relevant state can fault again. These facts rule out treating every `SIGSEGV` or `SIGBUS` as a recoverable Lisp error.

Only a fault whose address falls inside a registered guard range can enter stack recovery. On the signal stack, preserve the original context and redirect execution to a preallocated recovery trampoline. The trampoline switches to a safe runtime stack, leaves the faulting instruction, and returns to ordinary runtime code. That code unwinds to a saved safe boundary, restores the guard mapping, constructs `STORAGE-CONDITION`, and signals it. If the fault is outside a known guard, the address is unavailable, or recovery state is corrupt, retain the platform's fatal fault behavior.

This recovery sequence is an NCL design, not a portable POSIX promise. POSIX handler-safety constraints require the signal-side portion to remain minimal. In particular, changing page protection, allocating Lisp objects, unwinding Rust frames, or invoking `handler-case` belongs after control has transferred to the recovery trampoline.

## Guarded stack regions

Map each runtime stack as a bounded region with an inaccessible guard page at its growth edge. The `ncl-sys` layer reserves pages and exposes typed operations to query bounds, protect the guard, and restore it. Keep OS calls and unsafe context manipulation inside `ncl-sys`; safe crates hold a stack-region descriptor and request operations through that API.

| Stack | Owner and use | Exhaustion behavior |
| --- | --- | --- |
| Control | Generated Lisp calls and runtime control frames, owned by the runtime thread context. | `--control-stack-size` sets the configured control-stack capacity. A guard hit recovers through the overflow trampoline, unwinds, and signals `STORAGE-CONDITION`. |
| Binding | Dynamic binding records associated with each Lisp thread. | Guard or capacity detection raises a typed storage failure before a record can overwrite adjacent memory. |
| Alien | Foreign/native call frames for FFI work. | The runtime must leave foreign code on a safe recovery path before signaling; a fault in unrelated foreign memory remains fatal. |

Use a runtime-managed control stack with an explicit bound. The available control-stack budget is the distance from the current control stack pointer to its registered guard boundary, measured in the target's stack-growth direction. Generated calls check that budget before entering a callee so ordinary Lisp recursion can signal `STORAGE-CONDITION` through the typed runtime path before touching the guard. Keep the guard as the final boundary for missed checks and native stack growth.

Before entering generated code, save a per-thread recovery record outside the guarded control stack. It contains the safe stack pointer and runtime continuation to use after a recognized guard fault. The recovery trampoline switches to a preallocated emergency recovery stack, unwinds to that record, and leaves enough stack margin for a Lisp handler. Recovery must not jump across live Rust frames or skip their destructors; generated calls should prevent ordinary recursion from reaching the guard, and the trampoline may resume only at a runtime-defined landing point where this invariant holds. If the runtime cannot establish that landing point, retain the fatal fault path. Re-arm the guard before returning to user code. The regression test repeats the same overflow in the same process after the first handler returns.

Binding and alien stacks need independent bounds and guard regions so exhaustion in one cannot corrupt another. The implementation must size the alternate signal stack for the worst supported signal frame and reserve a separate emergency recovery stack; the alternate signal stack itself is not growable and must not be used as the Lisp handler stack.

## Platform adapters

The four supported targets are Linux and macOS on x86-64 and AArch64. Do not share `ucontext` layouts or hard-coded register offsets across them.

| Target family | Context details to isolate in `ncl-sys` |
| --- | --- |
| Linux x86-64 | Linux x86 signal context exposes x86-64 register state including `rsp` and `rip`; fault metadata includes x86 trap/address fields. |
| Linux AArch64 | Linux arm64 context uses `sp` and `pc`, general registers, and a fault-address field. Kernel fault classification can select `SIGSEGV` or `SIGBUS`. |
| macOS x86-64 | Use Darwin's target-specific `ucontext`/machine-context layout and x86-64 thread state; do not reuse Linux signal-context offsets. |
| macOS AArch64 | Use Darwin's arm64 machine-context layout (`sp`, `pc`, and arm registers); do not reuse Linux arm64 offsets. |

The current stack scanner already uses target-specific `rsp`/`sp` reads and OS-specific pthread APIs (`packages/sys/src/thread/stack.rs:4-75`). The new signal adapter must apply the same separation to fault-address extraction, program-counter adjustment, alternate-stack setup, mask operations, and recovery-context construction. macOS and Linux both provide POSIX signal concepts, but their C ABI declarations and machine contexts are platform-specific. `packages/sys/src/os.rs` currently gates its `mprotect` declaration to Linux or tests (`packages/sys/src/os.rs:21-23`); add the protection operation needed by both platform adapters without exposing unsafe calls to safe crates.

The POSIX.1-2017 `signal(3p)`, Linux `sigaction(2)`, `sigaltstack(2)`, `signal-safety(7)`, `signal(7)`, and `pthreads(7)` pages describe handler semantics, per-thread state, context delivery, and async-signal-safety. Apple archived `pthread_sigmask(2)` and `sigaction(2)` pages document thread mask and `SA_ONSTACK` behavior; use the former for mask semantics and the latter only for its alternate-stack contract. SBCL's `src/runtime/thread.c`, `interrupt.c`, and `bsd-os.c` are implementation examples for per-thread alternate stacks and classifying guard, GC, and ordinary memory faults; they are not POSIX or CLHS requirements.

## V5 signal-torture coverage

The V5 job includes GC stress, heap verification, multi-thread stress, signal torture, ASan, TSan, and Miri. L1-2 contributes the signal-specific cases below; it does not claim that the full V5 job passes.

| Case | Required observation |
| --- | --- |
| Control-stack guard | A bounded recursive `LABELS` call signals a catchable `STORAGE-CONDITION`; the same call is caught again in the same process after guard restoration. |
| Binding- and alien-stack guards | Each registered guard is identified independently. Recovery does not turn an unrelated bad address into a condition. |
| Unknown `SIGSEGV`/`SIGBUS` | A fault outside registered guard ranges follows the fatal path in a subprocess. |
| Alternate stack and masks | Every runtime thread has a usable alternate stack and its own mask; thread creation, signal delivery, and teardown do not reuse another thread's registration. |
| `SIGPIPE` | Writing to a closed pipe returns through the stream path as `STREAM-ERROR`; it does not terminate the runtime. |
| `SIGINT` | The handler records the event, then the normal safepoint path delivers the interactive condition to a Lisp handler. |
| `SIGTERM` | Exit hooks execute once after the event is drained in normal runtime control. |
| `SIGCHLD` | A child exit is reaped outside the handler without losing another child's status. |
| `SIGPROF` | Samples can be delivered during allocation, GC, and generated-code execution; the handler performs no allocation, lock, or formatting. |
| Repeated and concurrent delivery | Signals arriving during thread startup/teardown, masked regions, GC, and nested non-local exits neither corrupt the saved context nor leave a signal blocked after recovery. |

Run intentional fault cases in subprocesses so a failure in recovery cannot kill the test runner. The assertions check both the condition/result and process state after recovery; “did not panic” is not sufficient evidence.
