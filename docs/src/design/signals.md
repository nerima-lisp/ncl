# Signals and guarded stacks

This contract defines FR-A08 signal handling and the FR-A02 stack boundary. `ncl-sys` owns OS mappings, signal dispositions, alternate-stack registration, context adapters, and recovery trampolines. Runtime and thread contexts own stack records and Lisp condition delivery. A signal handler does not allocate, format, search Lisp handlers, or run a restart.

## Current implementation

The release recursion probe documented in `docs/src/design/conditions.md` exits 134 with Rust std's `fatal runtime error: stack overflow, aborting`. That probe executes on the process main thread. No NCL signal API or control-stack-size option exists yet.

`packages/sys/src/os.rs:9-72` declares mapping, pthread identity, stack discovery, and loader APIs, but no `sigaction`, `sigaltstack`, or signal-mask API. The scoped search `rg -n 'sigaction|sigaltstack|pthread_sigmask|SIG(SEGV|BUS|PIPE|INT|TERM|CHLD|PROF)|--control-stack-size|--dynamic-space-size' packages/sys/src packages/threads/src packages/runtime/src src` found no handler setup or stack-size option; its only signal-name hit is the `SIGTERM` convention comment in `packages/threads/src/process.rs:35`.

`packages/sys/src/thread/stack.rs:4-42` discovers pthread bounds: macOS uses `pthread_get_stackaddr_np` and `pthread_get_stacksize_np`; Linux uses `pthread_getattr_np` and `pthread_attr_getstack`. It reads `rsp` on x86-64 and `sp` on AArch64 (`:44-75`). `packages/sys/src/thread.rs:179-181, 219-230` publishes those bounds for GC scanning; conservative scanning is at `:248-269`. Collection scans roots and frames at `packages/sys/src/heap/collect.rs:47-64, 186-205`; Lisp frames use mapped metadata in `packages/codegen/src/frames.rs:236-285`. These paths describe the pthread stack, not a separate Lisp control stack.

## Signal installation and ownership

POSIX dispositions are process-wide; signal masks and alternate signal stacks are per-thread. Install dispositions once after Rust std initialization. Rust std source in repository `rust-lang/rust` at revision `52d0866` installs its handler only for default dispositions and exposes no public chaining API (`library/std/src/sys/pal/unix/stack_overflow.rs`, install path lines 154-176). NCL therefore replaces Rust std's `SIGSEGV` and `SIGBUS` dispositions. The required alternate-stack size is the page-rounded maximum of the target's `SIGSTKSZ` and `MINSIGSTKSZ`, plus 64 KiB of NCL handler reserve. Reuse an enabled Rust std alternate stack only when `sigaltstack` reports at least that size; otherwise `ncl-sys` installs a runtime-owned stack of that size. Thread startup verifies its own alternate stack before Lisp execution. Rust std's stack-overflow diagnostic is not preserved: Rust-only stack overflows and every unrecognized fault remain fatal. For an unrecognized fault, restore the default disposition and terminate by redelivery; do not return to a computational `SIGSEGV` instruction.

Use `sigaction` with fault information and machine context for `SIGSEGV`/`SIGBUS`, and `SA_ONSTACK`. Masks are configured per thread with `pthread_sigmask`. Block `SIGINT`, `SIGTERM`, and `SIGCHLD` in the main thread before creating runtime threads, then receive them on a dedicated `sigwait` control thread. Runtime threads keep them blocked. The control thread writes fixed signal events to a runtime queue; it never mutates a `ThreadContext`. Process `SIGINT` is routed to the designated main Lisp thread at its next safepoint, independent of which thread the kernel selected for delivery. Normal runtime code drains the event and calls `request_interrupt(&mut self)` there (`packages/sys/src/thread.rs:301-320`). The signal thread handles child reaping outside signal-handler context and requests orderly termination through normal runtime control.

An asynchronous handler may only inspect its supplied context, recognize a registered fault range, and transfer control through a target adapter. It must not allocate, lock, format, log, panic, access uninitialized thread-local state, call Lisp, unwind Rust frames, call exit hooks, or invoke `request_interrupt(&mut self)`. Normal runtime code constructs conditions after control reaches a safe stack and boundary.

### Signal policy

| Signal | Handler or sigwait action | Runtime action |
| --- | --- | --- |
| `SIGSEGV`, `SIGBUS` | `sigaction` classifies the registered active growth guard in `Running` state as overflow and the exact return-guard page in `Returning` state as a restoration event. It redirects either recognized address to the `ncl-sys` recovery trampoline; unrecognized addresses, permanent outer-edge guards, and invalid states restore default disposition and terminate fatally. | The active growth-guard fault signals the preallocated `STORAGE-CONDITION`; the return-guard event only restores the guard and resumes execution. |
| `SIGPIPE` | Ignore the process signal during runtime setup. | Convert a failed stream write (`EPIPE`) to `STREAM-ERROR` with the affected `:stream`. |
| `SIGINT` | Dedicated `sigwait` thread records an interrupt event. | Deliver `NCL-EXT::INTERACTIVE-INTERRUPT` to the designated main Lisp thread at its next safepoint. This condition is an NCL extension. |
| `SIGTERM` | Dedicated `sigwait` thread records a termination event. | Run exit hooks once in normal runtime control, then exit. |
| `SIGCHLD` | Dedicated `sigwait` thread receives the event. | Reap children and update process records outside a handler. |
| `SIGPROF` | When profiling is enabled, the `ncl-profiler` timer selects a running Lisp thread and sends it a thread-directed `SIGPROF`. The handler copies bounded sample data to that thread's preallocated ring buffer and increments its lost-sample `AtomicUsize`; the four supported targets provide pointer-width atomics (`target_has_atomic = "ptr"`). | Drain and symbolize samples outside signal context; profiling is disabled by default. |

`SIGSEGV` and `SIGBUS` are not interchangeable on every target. POSIX specifies undefined behavior for simply returning from a handler for a computational `SIGSEGV`; Linux AArch64 can report `SIGBUS` for some page faults. A handler that returns to the same faulting instruction without changing its mapping or context will fault again. Only registered guard addresses are recoverable; all other addresses stay fatal.

## Runtime-mapped stacks

Every thread that executes Lisp runs Lisp code on a runtime-mapped control stack, including the process main thread. At startup, the main thread switches from its OS-provided pthread stack to its mapped control stack before evaluating Lisp. Lisp-created threads do the same at thread entry. `std::thread::Builder` continues to create the native Rust stack; it is not the Lisp control stack and does not determine Lisp recursion capacity. `--control-stack-size` sets the usable mapped control-stack size.

Reserve each stack mapping with an inaccessible guard region at both mapping edges. For a downward-growing control stack, addresses from low to high are: the permanent low-edge guard, the active growth guard, a reserved return-guard page, the usable frame interval, and the permanent high-edge guard. `--control-stack-size` counts only the usable interval. Binding and alien stacks have independent bounds, guards, and overflow records. Each preallocated per-thread `OverflowRecord` lives outside the Lisp stack and stores stack kind, active and return-guard bounds, saved machine context, last valid frame pointer and PC, recovery landing SP/PC, and the pending condition root.

The control stack holds generated Lisp frames and Lisp continuations. GC must scan that mapped range using the existing precise frame maps. The current pthread scan remains for native runtime roots; Rust frames that can hold Lisp words publish roots through the rooting API. FFI entry records the control-stack pointer and Lisp roots before switching to the native/alien stack. Foreign code and Rust runtime frames execute off the Lisp control stack. A callback into Lisp switches back to that thread's control stack and restores the published root record before running generated code. Return transitions restore the previous stack pointer and root state.

Generated-code prologues do not add a dynamic “remaining control-stack budget” check. The active guard owns overflow detection; normal frames grow on the mapped stack and large dynamic data stays in the heap. Codegen emits ordinary ABI frame setup, and L1-2.2 owns stack mapping and guard registration.

## Single guard and return-guard recovery sequence

Use the SBCL guard/return-guard protocol for control- and binding-stack exhaustion. Each has an independent `OverflowRecord` and usable bounds. An alien-stack fault is recoverable only for an FFI call declared abort-safe and with a valid saved transition record; the runtime cancels that foreign call and signals on the restored Lisp control stack. Faults inside a foreign call without that declaration and record remain fatal. The transition cannot skip a live Rust frame. For downward control-stack growth:

1. A write into the registered active growth guard raises `SIGSEGV` or `SIGBUS`. `ncl-sys` classifies it only by comparing the supplied fault address with this thread's registered guard ranges and recovery state; classification never reads or probes the faulted page. An active growth-guard hit in the matching `Running` state is stack exhaustion and maps to the preallocated `STORAGE-CONDITION`. A return-guard hit is a restoration event only while the matching `OverflowRecord` is in `Returning` state and names that exact page; it does not signal a second condition. A permanent outer-edge guard hit, unknown address, or invalid recovery state is fatal.
2. The signal adapter moves execution to the preallocated recovery stack and saves the interrupted context. Normal recovery code removes protection from the active guard and protects the adjacent return-guard page. Only then does it create or select `STORAGE-CONDITION` and enter Lisp handler search. The released guard page provides bounded headroom while the handler unwinds the exhausted frames.
3. As the unwind returns to a shallower stack pointer, it crosses the return-guard page. That exact registered fault enters the same `ncl-sys` adapter. Normal recovery code clears the released page, restores protection on the original active guard, removes protection from the return-guard page, and resumes at the interrupted return point without signaling another condition.
4. The same overflow can then be caught again in the same process. The guard is not considered restored until step 3 completes.

During handler execution the active guard is deliberately unprotected and the return guard is protected. The machine-code recovery trampoline is in `ncl-sys` at `packages/sys/src/signal/trampoline_x86_64.rs` and `packages/sys/src/signal/trampoline_aarch64.rs`, using target-specific `global_asm!`. OS context layouts and fault-address extraction live in separate platform/architecture adapters beside those files. No Rust or Lisp function is entered on the exhausted control stack from the signal handler.

GC starts at the saved SP and walks to the usable stack base; neither endpoint includes a guard page. The walk follows precise frame maps only within usable Lisp-frame intervals. If the faulting SP lies in the active guard during recovery, GC starts from the last valid in-range frame pointer saved in `OverflowRecord` and walks toward the usable base instead. It skips the active guard, reserved return-guard page, and both permanent edge guards; the interrupted context and pending condition root stay published in `OverflowRecord` while the active page is released. This makes the active growth guard the recoverable stack-capacity fault described by the GC contract, while the return guard is only a restoration event and permanent outer-edge guards remain fatal. No Lisp frame is placed in a guard page, and GC never reads or probes one.

The alternate signal stack and recovery stack are distinct. The alternate stack is for kernel signal delivery; the recovery stack is for the trampoline and transition to normal runtime control. They are preallocated per thread and never grow through the exhausted guard. Rust runtime frames must not be skipped by a recovery jump; the landing point is a runtime-owned transition record outside the Lisp control stack.

## Platform adapters

The supported targets are Linux and macOS on x86-64 and AArch64. Keep fault-address extraction, signal-context layout, stack-pointer/program-counter access, mask handling, and context modification in target-specific `ncl-sys` files. Linux x86-64 uses `rsp`/`rip`; Linux AArch64 uses `sp`/`pc`; Darwin uses its own machine-context layouts. The existing stack scanner already separates `rsp`/`sp` and OS pthread APIs (`packages/sys/src/thread/stack.rs:4-75`). `packages/sys/src/os.rs:21-23` currently gates `mprotect` to Linux or tests; the implementation must expose platform-appropriate protection operations for both OSes without leaking unsafe calls to safe crates.

The `sbcl/sbcl` repository's 2.6.10 source uses guard helpers and a fault classifier as the reference for this sequence (`src/runtime/os-common.c:522-539`, `src/runtime/interrupt.c:1552-1573, 1640-1686`). This is an implementation example, not a POSIX guarantee. POSIX signal dispositions, per-thread masks, alternate stacks, and async-signal-safety remain the platform contract.

## V5 signal-torture coverage

L1-2 contributes these cases to V5; it does not claim the full V5 job passes.

| Case | Required observation |
| --- | --- |
| Control-stack guard | A recursive `LABELS` call signals a catchable `STORAGE-CONDITION`; the same overflow is caught again in the same process after return-guard restoration. |
| Binding-stack guard | The matching return-guard sequence restores the binding-stack guard, then signals `STORAGE-CONDITION` on the Lisp control stack. |
| Alien-stack guard | For an FFI call declared abort-safe with a saved transition record, cancel the foreign call, restore the Lisp control stack, and signal `STORAGE-CONDITION`; an undeclared call, unregistered fault, or missing transition record is fatal. |
| Unknown `SIGSEGV`/`SIGBUS` | A fault outside every registered guard terminates a subprocess through the fatal path. |
| Main and worker stacks | Main and Lisp-created threads execute Lisp on mapped control stacks with distinct guard/scan bounds. |
| Alternate stack and masks | Every runtime thread has an enabled alternate stack at least the target minimum plus the NCL reserve; `SIGINT`, `SIGTERM`, and `SIGCHLD` are blocked outside the sigwait thread. |
| `SIGPIPE` | A write to a closed pipe returns as `STREAM-ERROR` rather than terminating the process. |
| `SIGINT` | The sigwait event reaches the main Lisp thread at a safepoint and a Lisp handler catches `NCL-EXT::INTERACTIVE-INTERRUPT`. |
| `SIGTERM` | Exit hooks run once in normal runtime control. |
| `SIGCHLD` | Child status is reaped by the control thread without losing another child's status. |
| `SIGPROF` | The `ncl-profiler` timer directs samples during allocation, GC, and generated execution; the handler does not allocate, lock, or format, and full buffers increment the lost-sample counter. |
| Repeated delivery | Startup, teardown, GC, and nested non-local exits do not corrupt context or leave a signal blocked. |

Run intentional faults in subprocesses. Assert both the condition/result and process state after recovery; “did not panic” is not sufficient evidence.
