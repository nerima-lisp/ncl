# Calling convention

## 決定

NCL の論理入口は `(ctx, argc, a0, a1, a2, a3, rest) -> (v0, mv_count)` である。物理割当、4 語の frame header、Rust builtin、tail transfer の正規仕様は [Native backend](native-backend.md) を参照する。

| 項目 | x86-64 SysV | AArch64 AAPCS64 |
| --- | --- | --- |
| `ctx` | `r15` | `x21` |
| `argc` | `rdi` | `x0` |
| `a0..a3` | `rsi`, `rdx`, `rcx`, `r8` | `x1..x4` |
| `rest` | `r9` | `x5` |
| `v0` | `rax` | `x0` |
| `mv_count` | `rdx` | `x1` |
| scratch | `r10`, `r11` | `x16`, `x17` |

固定アリティの builtin は `extern "C" fn(ctx: *mut ThreadContext, a0: Word, a1: Word) -> Word` のような直接シグネチャを使う。可変長または keyword builtin だけが引数配列 adapter を使う。condition と非局所脱出は `ThreadContext` の pending flag と予約戻り値 `unbound` marker で通知し、呼び出しテンプレートは復帰直後に flag を検査する。`builtin!` は両形式を生成する。

NCL frame header は `previous FP / return PC / function object / flags` の 4 語とする。handler、cleanup、catch は `ThreadContext` の現在ポインタ 3 本からレコード連鎖を辿る。frame header 連鎖は GC と backtrace、record 連鎖は unwinder が使う。

## 根拠

レジスタ表を固定すると encoder、stack map、builtin ABI の検証対象が一致する。固定アリティで配列を一度 materialize しないため、直接 call の引数経路と戻り値経路を明示できる。詳細な frame と stack map の byte layout は native backend が所有する。

## 却下した代替案

- 物理レジスタ割当を backend 実装へ委譲する案は、GC map と builtin 境界を検証できないため却下する。
- 全 builtin を `args + argc` 形式にする案は固定アリティの不要な adapter を生むため却下する。
- handler を frame header に追加する案は call ごとの store 数を増やすため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- 表のレジスタ名、論理引数順、4 語の header を変更しない。
- fixed builtin に可変長 adapter を追加せず、pending flag の検査を省略しない。
- tail call、multiple values、unwind の追加仕様は native backend の定義を複製せず参照する。

## Physical register contract

The `RegisterId` table below is only the correspondence `number -> x86-64 name -> AArch64 name`; it does not assign a shared role. Roles are specified in separate target tables so that AArch64 `argc` is not inferred from the x86-64 row.

| RegisterId | x86-64 | AArch64 |
| ---: | --- | --- |
| 0 | rax | x0 |
| 1 | rdx | x1 |
| 2 | rdi | x2 |
| 3 | rsi | x3 |
| 4 | rcx | x4 |
| 5 | r8 | x5 |
| 6 | r9 | x6 |
| 7 | r10 | x7 |
| 8 | r11 | x8 |
| 9 | rbp | x9 |
| 10 | rbx | x10 |
| 11 | r12 | x11 |
| 12 | r13 | x12 |
| 13 | r14 | x13 |
| 14 | r15 | x14 |
| 15..30 | target-defined | x15..x30 |

| x86-64 role | register |
| --- | --- |
| return value | rax |
| multiple-value count | rdx |
| argc | rdi |
| a0..a3 | rsi, rdx, rcx, r8 |
| rest | r9 |
| scratch | r10, r11 |
| ThreadContext | r15 |
| frame pointer | rbp |

| AArch64 role | register |
| --- | --- |
| return value | x0 |
| multiple-value count | x1 |
| argc | x0 on entry |
| a0..a3 | x1..x4 |
| rest | x5 |
| scratch | x16, x17 |
| ThreadContext | x21 |
| frame pointer | x29 |
| physical link | x30 |
| platform reserved | x18 |
| callee-saved | x19..x28, x29, x30 |

The AArch64 `RegisterId` is x0 through x30 with the same numeric id. `x21` is the pinned ThreadContext register, `x16` and `x17` are scratch, `x18` is platform reserved, and `x30` is the physical link register. `ThreadContext.mv[0..]` holds additional values. In Lisp-to-Lisp calls, `ctx` is not overlaid on the argument sequence; only the pinned context register carries it.

## Frames, calls, and exits

The frame begins with four words: previous FP, return PC, function object, and flags. Flags encode frame kind, tail state, and native state. It is followed by argument spills, local slots, and outgoing area. Every slot is 8 bytes and the frame is 16-byte aligned. The fifth and later arguments are at `16(%rbp)` on x86-64 and `16(%x29)` on AArch64. This Lisp frame area is distinct from the C ABI overflow area and must not be confused with it.

A full call creates a frame and publishes a safepoint. A local call uses the same frame but an intra-code target. A self tail call reuses the frame after replacing arguments. A general tail call cleans its outgoing area and transfers directly; tail transfer is forbidden while cleanup is active. For `&rest`, argc > 4 overflow words are copied first, then the register window is appended into one continuous array. `&key` rejects an odd number of keyword/value arguments before binding pairs. Phase 1a uses a generic prologue; specialized `&rest` and `&key` prologues begin in 1c.

Multiple values return `v0` and count in the ABI registers; additional values are in the ThreadContext MV area. Direct Rust builtins use their fixed signature, while variadic or keyword builtins use the adapter returning `NclStatus`; both forms use a pending flag checked immediately after return.

CatchRecord contains `{ tag, target_frame, target_pc, value_slot, depth, previous }`. CleanupRecord and HandlerRecord also contain their frame address and dynamic depth. ThreadContext holds the three current pointers. Unwind marks the pending exit, runs LIFO cleanup, restores binding and handler chains, then moves to the selected `target_pc` in the selected frame. Rust frames propagate `NclStatus` in two stages: adapter to builtin caller, then caller to the top NCL entry. Rust panic is abort.

IR handler kinds select the record operation at `EnterHandler` and `LeaveHandler`:
`Catch` calls `enter-catch`/`leave-catch` with the region depth and catch tag;
`UnwindProtect` calls `enter-unwind-protect`/`leave-unwind-protect` with its cleanup
entry; and `Progv` calls `enter-progv`/`leave-progv` with the binding target values.
The first operation links the corresponding record through the current ThreadContext
pointer. The unwinder runs cleanup records in LIFO order and restores Progv bindings
before transferring to a catch target.

## Phase 1 runtime ABI binding

`ncl-codegen::RuntimeAbi` is the only lowering-to-runtime boundary. Lowering asks the embedding runtime for typed byte offsets for TLAB bump/limit, the `safepoint_request` and `pending` words, multiple-value state, and the handler/cleanup/catch pointers. It asks for addresses of allocation and safepoint slow paths, unwind, builtins, and the constant table. An unavailable offset or address is reported as an unavailable operation; codegen does not infer an offset from a Rust layout or embed a runtime address. (`packages/codegen/src/abi.rs`, `RuntimeAbi`, `ContextField`, `RuntimeFunction`.)

On AArch64, `x21` remains the pinned context register, `x0` is entry `argc` and the return value, `x1` is the multiple-value count, `x1..x4` carry the first four logical arguments, `x5` carries `rest`, and `x16`/`x17` are scratch. The machine-visible `ThreadLayout` offsets are the source of the context memory operands. (`packages/codegen/src/abi.rs`, `Aarch64Abi`; `packages/sys/src/thread.rs`, `ThreadLayout`.)

The AArch64 builtin lowering supplies context in `x0`, loads at most four logical arguments into `x1..x4`, and calls the address returned by `builtin_address`. TLAB allocation first reads bump and limit, advances the bump by `words * 8` when the fast path fits, and otherwise calls the `(ctx, words)` allocation slow path. The safepoint slow path supplies the pinned context in `x0`, the active generated frame pointer (`x29`) in `x1`, and the continuation PC in `x2`. (`packages/codegen/src/target_aarch64_lowering.rs`, `lower_alloc`, `lower_safepoint`, `lower_builtin`.)
