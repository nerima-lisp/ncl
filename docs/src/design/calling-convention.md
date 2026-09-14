# Calling convention

## 決定

関数呼び出しは関数オブジェクトの entry pointer へ間接 jump/call する。引数個数は専用 register、引数は architecture ABI の汎用 registers、残りは caller frame の argument area に置く。primary value は return register、multiple values は count register と frame area に置く。

| role | x86-64 System V | AArch64 |
| --- | --- | --- |
| arguments 0..5 | `rdi,rsi,rdx,rcx,r8,r9` | `x0..x5` |
| argc | `r10` | `x15` |
| primary result | `rax` | `x0` |
| second result / MV count | `rdx` | `x1` |
| frame pointer | `rbp` | `x29` |
| stack pointer | `rsp` | `sp` |
| thread/runtime | `r14` | `x28` |

`r11`/`x16` は call scratch、`r12`/`x19` は callee-saved scratch とする。NCL entry は `extern "C"` と同じ machine ABI を使うが、argc と NCL frame metadata を追加する。frame header は previous frame pointer, return PC, function object, code object, stack-map id, handler-chain pointer の 6 words。argument area は 16-byte aligned とする。

full call は新 frame と function entry を作る。local call は compiler が lexical target を証明した場合だけ同じ frame の known entry へ jump する。tail call は現在 frame の arguments を上書きして jump し、handler または unwind-protect が active なら full call に戻す。

ordinary lambda-list は front end が required, optional, rest, key, allow-other-keys, aux を positional metadata に lower する。arity error は entry prologue の condition。multiple values は `mv_count` (0..2^32-1) と frame slots の組であり、single-value consumer は count 0 を nil、count 1 を primary、count >1 を primary とする。

Rust builtin は `extern "C" fn(*mut RuntimeHandle, argc: u32, args: *const Word, out: *mut MultipleValues) -> NclStatus`。builtin は entry 時に thread 登録状態を確認し、GC root を登録してから allocation する。`NclStatus` は `Ok`, `Condition`, `NonLocalExit` の 3 種で、condition object と payload は runtime の pending slot に置く。

catch/throw と handler の unwind は Cranelift `try_call`/`try_call_indirect` の exception table を使う。例外 tag は catch tag と dynamic handler depth を含む 64-bit token。Cranelift は table と分岐を生成するが unwinder 自体は提供しないため、ncl-sys の `ncl_unwind_to(depth, token)` が pending non-local exit を設定し、landing block が handler chain を巻き戻す。`unwind-protect` は frame header の cleanup record として登録し、target handler に移る前に LIFO で実行する。試行呼び出しのない C/Rust callback が longjmp してはならない。

compiler macro は `Function` object の inline-cache hook を参照する。cache は call-site, generic function identity, class layout generation の triple を key とし、最大 4 entry、miss は runtime dispatch へ戻る。self tail call は引数再束縛と back-edge に変換する。

## 根拠

register passing keeps the common one-to-six argument path free of heap arrays. A count register makes zero, one, and many values distinguishable without sentinel values. The explicit frame header is required by stack maps, GC, debugger backtraces, and non-local exits. Cranelift exposes `try_call` and exception tables and documents that the embedder defines exception-tag meaning, so the unwinder remains in ncl-sys.

## 却下した代替案

- Lisp arguments を常に heap vector にする案は fixed-arity calls の allocation を増やすため却下した。
- Rust panic を Lisp condition に使う案は unwind ABI と GC roots を混在させるため却下した。
- Cranelift の trap を catch/throw に流用する案は trap が recoverable Lisp condition の payload/handler depth を表せないため却下した。
- full call だけの ABI は local call と tail recursion の frame overhead を固定化するため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- register allocation table と frame header の順序は固定である。
- builtin は指定された `extern "C"` signature と `NclStatus` を使う。
- allocation/call/back-edge には safepoint metadata を付け、raw Rust pointer を跨がせない。
- handler chain を直接編集せず、runtime API で establish/unwind する。
- `try_call` の presence を unwinder 実装済みの意味に解釈してはならない。
