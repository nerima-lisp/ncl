# Native backend

## 契約の範囲

64-bit little-endian macOS/Linux の `ncl-ir` を x86-64 または AArch64 machine code へ変換する。第三者の compiler、loader、C library wrapper は使わず、OS ABI は `ncl-sys` の自前 `extern "C"` 宣言だけを経由する。

## 決定

`ncl-codegen` は `MachineFunction` を作り、asm crate が bytes と `Fixup` を生成し、`ncl-objfile` が `Relocation` と FASL/object sections に変換する。asm crate は `Reg`、`Inst`、`Fixup`、`EncodeError` を自前定義し何にも依存しない。code object は `ncl-sys` 管理の非移動 code space に page 単位で置き、未参照後に解放する。GC は code object 内の constant slots だけを更新する。

frame header は 4 語以下、`previous FP / return PC / function object / flags` とする。handler、cleanup、catch は ThreadContext の 3 本の現在ポインタで record chain を形成し、record は frame address と dynamic depth を持つ。unwinder は record chain、backtrace と GC scan は frame header chain を辿る。

### 物理呼び出し規約

| 項目 | x86-64 SysV | AArch64 AAPCS64 |
| --- | --- | --- |
| ctx | r15 | x21 |
| argc | rdi | x0 |
| a0..a3 | rsi/rdx/rcx/r8 | x1..x4 |
| rest | r9 | x5 |
| v0 | rax | x0 |
| multiple-value count | rdx | x1 |
| scratch | r10/r11 | x16/x17 |

fixed builtin は直接 `extern "C" fn(ctx: *mut ThreadContext, a0: Word, a1: Word) -> Word` 形式で呼ぶ。adapter と args pointer は variadic/keyword のみ許可する。pending flag と `unbound` marker は呼び出し直後に検査する。`builtin!` は両形式を生成する。

### metadata と object

SafepointMap は 16-byte header、slot bitmap、register id 列である。header は `pc_offset u32, frame_words u16, slot_words u16, word_slot_count u16, register_mask u16, map_flags u32`。FASL header は 64 byte、magic `NCLFASL\\0`、version 1、architecture、pointer width 8、little-endian、feature bitmap、各 section offset/size を固定する。

phase は 1a template、1b linear scan、1c tail call、2 unbox/IC とする。tail transfer は frame header と dynamic records の条件を満たす場合だけ行い、active cleanup では禁止する。

## 根拠

非移動 code space は return PC と絶対 branch target の GC relocation を排除する。直接 fixed builtin call は ABI の引数・戻り値を machine contract に揃える。record chain を header から分離すると call ごとの store 数を抑えつつ dynamic unwind を明示できる。

## 却下した代替案

- 8 語 frame header は毎 call の store 数が多いため却下する。
- code object を moving heap に置く案は return PC を再配置するため却下する。
- 全 builtin を args pointer adapter にする案は fixed call の余計な materialization を生むため却下する。
- encoder が allocation、OS call、GC を行う案は bounded context を混ぜるため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- 表のレジスタ、4 語以下 header、16-byte map、64-byte FASL header、phase 順序を変更しない。
- encoder は `ncl-codegen`、`ncl-object`、`ncl-sys` に依存しない。
- code bytes に移動 heap address を埋め込まず、constant slot と relocation metadata を使う。
- stack map、RootToken、pending flag、unwind record を省略した実装を受け入れない。
