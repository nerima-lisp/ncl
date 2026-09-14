# Calling convention

## 決定

NCL の論理入口は `(ctx, argc, a0, a1, a2, a3, rest) -> (v0, mv_count)` である。物理割当、4 語以下の frame header、Rust builtin、tail transfer の正規仕様は [Native backend](native-backend.md) を参照する。

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

NCL frame header は `previous FP / return PC / function object / flags` の 4 語以下とする。handler、cleanup、catch は `ThreadContext` の現在ポインタ 3 本からレコード連鎖を辿る。frame header 連鎖は GC と backtrace、record 連鎖は unwinder が使う。

## 根拠

レジスタ表を固定すると encoder、stack map、builtin ABI の検証対象が一致する。固定アリティで配列を一度 materialize しないため、直接 call の引数経路と戻り値経路を明示できる。詳細な frame と stack map の byte layout は native backend が所有する。

## 却下した代替案

- 物理レジスタ割当を backend 実装へ委譲する案は、GC map と builtin 境界を検証できないため却下する。
- 全 builtin を `args + argc` 形式にする案は固定アリティの不要な adapter を生むため却下する。
- handler を frame header に追加する案は call ごとの store 数を増やすため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- 表のレジスタ名、論理引数順、4 語以下の header を変更しない。
- fixed builtin に可変長 adapter を追加せず、pending flag の検査を省略しない。
- tail call、multiple values、unwind の追加仕様は native backend の定義を複製せず参照する。
