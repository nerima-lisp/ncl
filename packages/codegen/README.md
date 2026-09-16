# ncl-codegen

`ncl-codegen` lowers validated NCL IR into fixed-template native code. The
current execution lane is AArch64 on macOS arm64, with four-word native frame
headers, stack slots for SSA values, runtime builtin calls, TLAB allocation,
and cooperative safepoint polls.

## Phase 1a execution status

| Item | Executed test | Result |
| --- | --- | --- |
| (a) Constant and `(+ 1 2)` | `executes_constant_return_in_published_code`, `executes_fixnum_add_of_two_arguments` | Pass |
| (b) cons and `car`/`cdr` | `executes_cons_allocation_car_and_cdr_on_tlab_fast_path`, `executes_cons_allocation_on_slow_path` | Pass |
| (c) Branch paths and block arguments | `executes_both_branch_paths_with_block_arguments` | Pass |
| (d) Builtin call and rest argument | `executes_builtin_call_with_context_and_arguments`, `loads_fifth_argument_from_rest_storage` | Pass |
| (e) Safepoint poll | `executes_safepoint_poll_without_and_with_request` | Pass |
| (e2) Function object root forwarding | `forwards_function_object_from_generated_frame_map_simulation`, `forwards_function_object_from_real_frame_after_safepoint_collection` | Pass (debug/release) |
| (f) Recursive call and `fib(25)` | `executes_recursive_fib_twenty_five_with_four_word_frames` | Pass, 75025 |

The release measurement command was:

```text
nix develop '<worktree>' --command cargo test --release -p ncl-codegen --test exec_aarch64 -- --nocapture
```

The test invokes `fib(25)` ten times and reports the wall-clock median. At
commit `f10b581a`, the median on macOS arm64 was `687875 ns`.

## Contract differences

The stable ABI and frame/map contracts are specified in [Calling convention](../../docs/src/design/calling-convention.md) and [Native backend](../../docs/src/design/native-backend.md). The notes below are implementation observations for the Phase 1a fixture.

- Native calls place the callee function object in `x16` and its published entry
  address in `x17`; the callee prologue stores `x16` in frame-header word 2.
  The sys snapshot collector forwards that word and writes the result back to
  the active generated frame before the runtime callback returns.
- `alloc_slow` receives `(ctx, words)` and returns an untagged address. The
  fast path advances `Thread`'s TLAB bump by `words * 8` and returns the old
  bump address.
- The AArch64 test ABI is named `Aarch64Abi`; fixnum encoding remains the
  shared `value << 3` representation.
- Safepoint maps for allocation and polling point immediately after the slow
  path `blr`. The decoder tests inspect those emitted instructions.
- A safepoint slow path receives the registered context in `x0`, the active
  generated frame pointer in `x1`, and the continuation PC in `x2`. The
  continuation PC is the address immediately after `blr`, which is also the
  safepoint map PC.
- When multiple registered threads share one OS thread, every thread other
  than the collector must be in native state during collection.

## Module layout

The lowering is split between `target_aarch64.rs`,
`target_aarch64_lowering.rs`, and its `target_aarch64_lowering/ops.rs` child;
ABI and safepoint metadata live in `abi.rs` and `safepoint.rs`. AArch64
execution coverage is split across `tests/exec_aarch64.rs` and its fixture
children, while decoder-oriented fixtures are split between `tests.rs`,
`tests_aarch64.rs`, and `tests_aarch64_fixtures.rs`. Each Rust source module
stays below the repository's 500-line limit.
