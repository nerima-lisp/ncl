# Native backend

## 契約の範囲

この文書は、`ncl-ir` を x86-64 または AArch64 の機械語へ変換し、JIT、FASL、実行ファイルから同じコードを実行するバックエンドの契約である。Cranelift、LLVM、`libc`、`libloading` は使わない。OS の ABI は `ncl-sys` の `extern "C"` 宣言だけを経由する。対象は 64-bit little-endian の macOS と Linux であり、Windows ABI、32-bit target、big-endian target は Phase 1 の対象外とする。

既存の object layout の `Word`、lowtag、widetag、cons 2 語、NIL/T の静的配置を変更しない。既存 IR の `OpKind` と `Terminator` を入力契約とし、front end に新しいバックエンド用 IR を要求しない。

## 決定

### 境界づけられたコンテキスト

DDD 上の bounded context と責務を次のように分ける。

| context | crate | 所有する概念 |
| --- | --- | --- |
| IR lowering / 命令選択 | `ncl-codegen` | `MachineFunction`、テンプレート、仮想レジスタ、呼出し列、`SafepointMap` |
| x86-64 命令エンコード | `ncl-asm-x86-64` | `Reg`、`Inst`、`Fixup`、エンコーダ、最小 disassembler |
| AArch64 命令エンコード | `ncl-asm-aarch64` | `Reg`、`Inst`、`Fixup`、エンコーダ、最小 disassembler |
| object / image | `ncl-objfile` | `Section`、`Relocation`、FASL、Mach-O、ELF の writer |
| 実行時リンク / メモリ | `ncl-sys` | JIT page、W^X、icache、`dlopen`/`dlsym`、OS thread primitives |
| ランタイム ABI | `ncl-object` | `Word`、`ThreadContext`、frame header、MV、condition status |

この分割は機械語のビット表現、命令選択、割当、ファイル形式、実行時リンクのモデルを混在させないためのものとする。

### crate と隣接リスト

```text
ncl-ir
  -> ncl-codegen
       -> ncl-asm-x86-64
       -> ncl-asm-aarch64
       -> ncl-object
       -> ncl-sys
ncl-codegen -> ncl-objfile
ncl-asm-x86-64 -> ncl-objfile
ncl-asm-aarch64 -> ncl-objfile
ncl-objfile -> ncl-sys
ncl-object -> ncl-sys
ncl-compiler-front -> ncl-ir
ncl-runtime -> ncl-compiler-front, ncl-codegen, ncl-objfile, ncl-object
```

`ncl-codegen` は target-specific crate を trait の実装として受け取る。asm crate は `ncl-codegen` に依存しない。`ncl-objfile` は命令選択を知らず、relocation の種類と section bytes だけを受け取る。この向き以外の依存は禁止し、依存グラフに cycle を作らない。

Phase 1 の初期実装は 4 レーンに分ける。

| lane | 主なファイル数 | 主要型数 | 対象 |
| --- | ---: | ---: | --- |
| ABI / frame / GC metadata | 8 | 14 | `ncl-codegen` の ABI、frame、stack map、unwind 契約 |
| x86-64 | 10 | 16 | instruction model、encoder、fixup、disassembler、golden test |
| AArch64 | 10 | 16 | instruction model、encoder、fixup、disassembler、golden test |
| object / FASL / JIT adapter | 12 | 20 | `ncl-objfile`、`ncl-sys` の JIT 境界、Mach-O/ELF/FASL |

この数は初期分割の見積りであり、生成コードを含めない。ABI lane が `MachineFunction` と binary metadata schema を先に凍結し、x86-64 lane と AArch64 lane は互いの実装を待たずに進める。object lane は `CodeBlob` と `Relocation` の型が凍結した後に実装する。

公開 API の最小形は次の通りとする。

```text
MachineFunction lower(Function, TargetAbi) -> Result<MachineFunction, CodegenError>
CodeBlob encode(MachineFunction, TargetIsa) -> Result<CodeBlob, EncodeError>
ObjectFile emit_object(CodeBlob, RelocationTable, ObjectTarget) -> Result<Bytes, ObjectError>
JitHandle map_and_publish(CodeBlob, LinkTable, &mut ThreadContext) -> Result<JitHandle, SysError>
```

返却する bytes は所有権を持つ immutable buffer とし、エンコーダは I/O、allocator、global state に触れない。コード生成は `Word` の意味を解釈してよいが、object の内部アドレスを直接保持してはならない。

### 段階的な実装計画

| phase | 決定 | 完了条件 |
| --- | --- | --- |
| 0 | ABI schema、instruction subset、relocation、FASL header を固定する | schema の parser/serializer が round-trip し、各数値に golden test がある |
| 1a | 全 `Op` を固定テンプレートへ展開する。値は frame の 8-byte slot、`r10`/`r11` または `x16`/`x17` のみ scratch とする | `(+ 1 2)`、cons、条件分岐、builtin call、GC safepoint、`fib(25)` が native entry から実行できる |
| 1b | linear scan register allocation、spill、parallel move を追加する | 代表 benchmark の native 実行時間がテンプレート版以下で、spill の stack map が全 safepoint に存在する |
| 1c | self tail call、一般 tail call、`&rest`/`&key` の専用 lowering を追加する | 深さ 1,000,000 の self recursion が frame を増やさず完了する |
| 2 | fixnum と double の unbox、型推論との接続、inline cache を追加する | Phase 2 第一関門として、同一マシン・同一入力で `fib(25)` が SBCL の 2 倍以内。測定条件と commit hash を記録する |
| 3 | full native object/image、debug info、最適化を完成させる | ANSI conformance と SBCL 公開拡張の対象テストが native image で再現する |

Phase 1 は性能目標を達成したとは扱わない。`fib(25)` は実行可能性の回帰検査であり、Phase 2 の比較測定までは性能の主張をしない。

却下した代替案は、初期から SSA 最適化とグラフ彩色を入れる案、portable bytecode を残す案、target ごとに異なる IR を作る案である。いずれも bootstrap の検証範囲を広げ、lane 間の契約を増やす。Cranelift を薄く包む案も、命令・stack map・tail call の所有権が曖昧になるため採用しない。

### 物理呼び出し規約

論理的な入口は `(ctx, argc, a0, a1, a2, a3, rest) -> (v0, mv_count)` を維持する。ただし `ctx` は pinned callee-saved register に常駐し、通常の Lisp-to-Lisp call では引数列に重ねて渡さない。

| 項目 | x86-64 SysV | AArch64 AAPCS64 |
| --- | --- | --- |
| `ThreadContext` pinned | `r15` | `x21` |
| `argc` | `rdi` | `x0` |
| `a0..a3` | `rsi`, `rdx`, `rcx`, `r8` | `x1..x4` |
| `rest` | `r9` | `x5` |
| primary value `v0` | `rax` | `x0` |
| multiple-value count | `rdx` | `x1` |
| additional values | `ThreadContext.mv[0..]` | `ThreadContext.mv[0..]` |
| frame pointer | `rbp` | `x29` |
| link / return PC | frame header の語 1 | frame header の語 1。物理 link は `x30` |
| callee-saved | `rbx`, `rbp`, `r12..r15` | `x19..x28`, `x29`, `x30` |
| fixed scratch | `r10`, `r11` | `x16`, `x17` |
| reserved runtime scratch | `r14` は通常保存、`r15` は ctx | `x18` は platform reserved、`x21` は ctx |

ABI 番号は `RegisterId` に固定し、x86-64 は `rax=0, rdx=1, rdi=2, rsi=3, rcx=4, r8=5, r9=6, r10=7, r11=8, rbp=9, rbx=10, r12=11, r13=12, r14=13, r15=14`、AArch64 は `x0..x30` を同じ番号で表す。stack map の register bit はこの番号を使う。

NCL frame は 8 語、frame pointer の負方向に次の順序で固定する。

| 語 | 内容 |
| ---: | --- |
| 0 | previous frame pointer |
| 1 | return PC |
| 2 | function object `Word` |
| 3 | handler-chain link |
| 4 | cleanup-chain link |
| 5 | catch-chain link |
| 6 | debug function id |
| 7 | flags: frame kind、tail state、native state |

frame header の後ろに引数 spill、local slot、outgoing argument area を順に置く。slot はすべて 8 byte、frame size は 16 byte 境界へ丸める。非 tail call は caller が outgoing area を確保する。引数 4 個を超える値は x86-64 では `16(%rbp)` 以降、AArch64 では `16(%x29)` 以降の outgoing stack area に Word 単位で置く。これは C ABI の overflow 引数位置とは異なるため、Lisp entry と Rust `extern "C"` entry を混同しない。

full call は callee の新しい frame を作り、function object と return PC を header に登録してから entry へ分岐する。local call は既知の code address と同じ NCL ABI を使い、関数値 lookup を省く。self tail call は current frame の header、handler、cleanup、catch が空である場合だけ、引数領域を parallel move して entry へ jump する。一般 tail call または dynamic handler がある tail call は unwind metadata を失わないよう、既存 frame を解放してから新しい frame を作る full tail transfer とする。cleanup が active な frame では tail call を禁止する。

`&rest` は prologue が `argc > 4` の overflow words と register window を一つの contiguous Word array に materialize し、`rest` にその address を置く。`&key` はまず奇数個の keyword/value を検査し、keyword symbol と value の pairs を temporary slots に作る。keyword parse 中の allocation は root slot と stack map を用いる。Phase 1a は generic prologue を使い、Phase 1c から specialized prologue を許可する。

Rust builtin は `extern "C"` の adapter を一つだけ ABI 境界に置く。adapter は入る前に NCL の callee-saved register と MV state を spill し、`ctx: *mut ThreadContext` を第一引数として渡す。固定 arity は `ncl_builtin_fixed(ctx, args, argc) -> NclStatus`、可変長は `ncl_builtin_var(ctx, args, argc, mv) -> NclStatus` とし、SysV/AAPCS64 の普通の C 引数渡しに任せる。復帰時に `NclStatus`、`rax`/`x0` の primary value、`rdx`/`x1` の count を NCL ABI へ移す。Rust panic は process abort とし、Rust unwind を Lisp frame 越しに許さない。

### 非局所脱出

`catch` は frame header の catch link から辿れる `CatchRecord { tag: Word, target_pc: CodeOffset, frame: FrameAddress, depth: u32 }` を作る。`throw` は target tag を比較しながら frame chain を上り、各 frame の cleanup link を dynamic depth の降順で実行し、target frame の saved registers と stack slots を復元して `target_pc` へ jump する。tag comparison と cleanup 中の allocation は通常の safepoint とする。

`unwind-protect` は cleanup block と saved-value slots を `CleanupRecord { entry_pc, next, depth }` として登録する。複数の cleanup は link の LIFO 順に一度だけ実行し、cleanup が別の non-local exit を起こした場合は古い pending exit を置き換える。handler chain は condition handler の dynamic depth と frame address を持ち、throw と同じ巻き戻し機構で無効化する。

Rust frame は machine stack を NCL unwinder が走査しない。Rust adapter は `NclStatus::NonLocalExit { token }` を返し、各 adapter caller が status を返す二段階伝播にする。最上位 NCL entry だけが token を受けて Lisp frame chain を unwind する。この方式は Rust の panic/unwind personality に依存しない。SBCL の unwind block に似た landing block は使うが、例外 table の暗黙の personality ではなく、明示的な frame record を source of truth とする。

### GC メタデータ

各 safepoint は call、loop の後方 edge、allocation slow path の三種類に限定する。`SafepointMap` の wire header は 16 byte とし、little-endian で次を格納する。

```text
u32 pc_offset
u16 frame_words
u16 slot_words
u16 word_slot_count
u16 register_mask
u32 map_flags
```

続く bitmap は slot bit、続く `u16` 配列は live register の stable register id とする。slot bit 0 は frame header 語 0、bit 8 は最初の local slot、bit `8 + n` は nth local/outgoing slot とし、header 0..7 は常に non-root として bitmap に設定しない。`register_mask` の bit n は register id n が Word pointer または tagged immediate を保持することを示す。raw address、I64、F64 は bit を立てない。`map_flags` は bit 0 = call、bit 1 = loop-backedge、bit 2 = allocation-slow、bit 3 = has-derived-address、残りを 0 とする。

code object は moving GC heap 上の object で、header の code widetag、entry address、code size、constant table、stack-map index、debug table を持つ。machine code の relocation は code object の constant slot に `Word` を書くため、GC が code object を移動した後に relocation target を更新する。static NIL/T と immutable symbol names は static root であり、FASL load 中に registered root へ移す。collector は frame header の function object から code object を取得し、return PC の code-relative offset で map を binary search する。map の live slot/register を forwarding rule で更新してから previous frame へ進む。

native transition 中は `enter_native` が register spill area と conservative scan boundary を ThreadContext に記録し、`leave_native` がそれを解除する。JIT frame は precise map、Rust/native frame は既存 GC contract の conservative scan とする。未登録 Rust の `Word` 保持は禁止する。

### 機械語エンコーダの検証

各 ISA の instruction model は encoding field の範囲を検査し、branch displacement と immediate overflow を `EncodeError` にする。エンコーダの単体テストは、(1) 命令ごとの bytes golden、(2) encode/decode の往復、(3) register/immediate/branch target の境界値、(4) seeded randomized instruction sequence の 4 種類とする。

golden bytes は開発時に Nix toolchain の `llvm-mc` または target `as` と `objdump` で作り、生成コマンドと target triple をテスト metadata に記録する。実行時に外部 assembler を呼ばない。decoder は Phase 1 の全 ISA を網羅せず、自分が emit した instruction subset と prologue/epilogue だけを読めればよい。golden は assembler の結果を盲信するものではなく、decoder、runtime smoke test、外部 disassembler の三面で照合する。

### object、FASL、JIT

FASL は NCL 固有の little-endian 形式とし、固定 64-byte header を次の通りにする。

| offset | size | field |
| ---: | ---: | --- |
| 0 | 8 | magic `NCLFASL\0` |
| 8 | 2 | format version `1` |
| 10 | 1 | architecture: `1=x86-64`, `2=aarch64` |
| 11 | 1 | pointer width `8` |
| 12 | 1 | endian `1=little` |
| 13 | 1 | header size `64` |
| 14 | 2 | reserved `0` |
| 16 | 8 | feature bitmap |
| 24 | 4 | code offset |
| 28 | 4 | code size |
| 32 | 4 | relocation offset |
| 36 | 4 | relocation count |
| 40 | 4 | constant offset |
| 44 | 4 | constant count |
| 48 | 4 | symbol offset |
| 52 | 4 | symbol size |
| 56 | 4 | stack-map offset |
| 60 | 4 | stack-map size |

Sections are code bytes, relocation records, tagged constants, symbol bindings, stack maps, and debug records in that order. A relocation record is 16 bytes: `u32 section`, `u32 offset`, `u32 kind`, `i32 addend`. Supported kinds are absolute Word, PC-relative branch, code-entry address, and external symbol address. Every offset is bounds-checked before allocation or pointer arithmetic. Target architecture, pointer width, endian, and feature bitmap must match before any code is mapped.

`compile-file` emits this FASL. `save-lisp-and-die` emits a minimal native image: Mach-O has `__TEXT,__text`, `__DATA,__const`, `__DATA,__ncl`, `LC_LOAD_DYLIB` entries only for declared system libraries, symbol table, and a native entry symbol; ELF has `PT_LOAD` RX/RW segments, `.text`, `.rodata`, `.ncl`, `.rela.text`, `.rela.ncl`, and a native entry symbol. The runtime loader rejects missing NCL metadata rather than treating a generic executable as an image.

`ncl-objfile` writes headers and relocation records itself. macOS arm64 ad-hoc code signing is delegated to the installed `codesign` command after the executable bytes are final. Reimplementing Code Directory, requirements, and page hashing would duplicate a platform security format and risks accepting an image the OS rejects. `codesign --sign -` is a build-time dependency only; JIT pages are not signed as a file and use the OS JIT entitlement/runtime policy.

`ncl-sys` declares `mmap`, `mprotect`, `munmap`, `dlopen`, `dlsym`, `pthread_jit_write_protect_np`, and `sys_icache_invalidate` as platform-specific `extern "C"` items. JIT allocation follows RW -> write -> RX. On macOS arm64, writing is bracketed by `pthread_jit_write_protect_np(0/1)` and instruction cache invalidation. On Linux, RWX is never requested: map RW, populate and relocate, then `mprotect` RX. Publication uses a release store of the code entry after RX transition; readers use acquire load.

### 診断とデバッグ

Each code object records function name, source location table, frame size, entry offset, and stack-map index. `disassemble` uses the small in-tree decoder and prints instruction bytes, symbolic branch targets, frame slots, and safepoint markers. External `objdump` is a test oracle only and is not a runtime dependency. Backtrace walks NCL frame headers, resolves the function object and PC-relative offset, and prints a source location when present. If a frame lacks a matching code object or stack map, it prints an address and stops safely instead of guessing.

### 却下した代替案

- physical register namesをCranelift pinned-register APIに委譲する案は、tail transfer、GC map、Rust adapter の契約が検証できないため却下する。
- 全 frame を conservative scan にする案は、移動 GC が false positive を pointer として更新するため却下する。
- FASL を host object file と同一形式にする案は、Word constants と code-object GC metadata を失うため却下する。
- runtime disassembler に外部 `objdump` を必須化する案は、standalone image の診断経路を壊すため却下する。
- 自前 ad-hoc signing を実装する案は、OS の署名仕様の複製を増やすため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- 前提にしてよいものは 64-bit little-endian、`Word` の既存 layout、表の pinned registers、8 語 frame header、固定 FASL header、precise JIT map と conservative native scan の境界である。
- x86-64 lane と AArch64 lane は `MachineFunction`、`Fixup`、`CodeBlob`、`SafepointMap` のフィールド順と error semantics を変更してはならない。
- Phase 1a は fixed template、stack slot、2 個の scratch register に限定し、独自の register allocator、unboxed representation、inline cache、alternate ABI を追加してはならない。
- encoder は allocation、OS call、symbol lookup、GC を行ってはならない。objfile は machine instruction の意味を再解釈してはならない。
- `ThreadContext`、handler、cleanup、MV、GC root を static global や未登録 Rust container に移してはならない。
- target-specific fast path を追加する場合は、まず ISA の encoder golden、stack map、unwind、JIT smoke test を同じ契約へ追加する。

## 既存契約への反映

後続ストリームは次の置換をそれぞれの文書へ適用する。

- `calling-convention.md`: 「Cranelift が物理レジスタを決める」を削除し、本書の x86-64/AArch64 表、8 語 frame、NCL tail-call 規則、Rust adapter ABI、`NclStatus` 伝播へ置換する。固定窓の論理順序は維持する。
- `compiler-pipeline.md`: 「ncl-ir を Cranelift CLIF へ lower」を「`ncl-codegen` の `MachineFunction`、target encoder、`CodeBlob`、stack-map metadata へ lower」に置換する。Cranelift version、CLIF、Cranelift module の記述を削除する。
- `crates.md`: `ncl-compiler-back` を `ncl-codegen`、`ncl-asm-x86-64`、`ncl-asm-aarch64`、`ncl-objfile` に分割し、`ncl-sys` の外部 crate 依存を除く。`ncl-codegen` は front に依存せず、asm crate は codegen に依存しない隣接リストへ更新する。
- `gc-interface.md`: Cranelift user stack map の記述を本書の 16-byte header、slot/register bitmap、PC index、code object 走査へ置換する。JIT frame は precise、Rust/native frame は conservative という境界を残す。
- `threads.md`: native transition、safepoint request、waitqueue、interrupt の記述に `ThreadContext` pinned register、register spill area、JIT RW/RX publish の境界を追記する。blocking primitive の protocol は変更しない。
