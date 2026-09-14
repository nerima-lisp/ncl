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

## Bounded contexts and lane estimate

| context | owner | boundary |
| --- | --- | --- |
| IR lowering | ncl-codegen | validated MachineFunction |
| x86-64 encoding | ncl-asm-x86-64 | bytes and Fixup |
| AArch64 encoding | ncl-asm-aarch64 | bytes and Fixup |
| object writing | ncl-objfile | FASL or native bytes |
| runtime publication | ncl-sys/ncl-object | RW to RX and entry release |

The four Phase 1 lanes are estimated as: 1a fixed prologue, constants, two scratch registers, and return; 1b calls, branches, stack slots, and both encoders; 1c allocation, relocation, precise stack maps, and unwind; 2 FASL, native images, diagnostics, and conformance. Lane outputs are `MachineFunction`, `Fixup`, `CodeBlob`, and `SafepointMap`, with stable field order and error semantics.

## Minimal public API

```text
MachineFunction lower(Function, Target) -> Result<MachineFunction, CodegenError>
CodeBlob encode(MachineFunction, Target) -> Result<CodeBlob, EncodeError>
Vec<u8> write_fasl(FaslInput) -> Result<Vec<u8>, ObjfileError>
SafepointMap build_map(FrameLayout, LiveValues) -> Result<SafepointMap, MapError>
```

The encoder has no allocator, OS call, symbol lookup, or GC dependency. `ncl-objfile` emits bytes only and does not call the OS. A code blob contains bytes, relocations, entry offset, frame size, and maps. Relocations are applied through constant slots, never by embedding moving heap addresses in instruction bytes.

## Physical convention

The logical entry is `(ctx, argc, a0, a1, a2, a3, rest) -> (v0, mv_count)`. x86-64 uses `rax=0`, `rdx=1`, `rdi=2`, `rsi=3`, `rcx=4`, `r8=5`, `r9=6`, `r10=7`, `r11=8`, `rbp=9`, `rbx=10`, `r12=11`, `r13=12`, `r14=13`, `r15=14`. `r15` is reserved for ThreadContext; r10/r11 are scratch. AArch64 uses x0..x30 with the same RegisterId numbering, x21 pinned for ThreadContext, x16/x17 scratch, and x29 as frame pointer.

The frame header is four words: previous FP, return PC, function object, and flags. Flags encode frame kind, tail state, and native state. Argument spills, locals, and outgoing slots follow; each slot is 8 bytes and total frame size is 16-byte aligned. Five or more arguments occupy caller outgoing slots. Full calls create frames, local calls target within the code object, self tail calls reuse the frame, and general tail calls transfer after cleanup. Active cleanup forbids tail transfer.

Direct fixed-arity builtins receive a direct `extern "C"` signature. Variadic and keyword builtins use `(ctx, argc, args, mv) -> NclStatus`. `builtin!` produces either form. A pending flag is checked after return. Multiple values use v0 plus count and the ThreadContext MV area.

## Non-local exit records

```text
CatchRecord    { tag, target_frame, value_slot, previous }
CleanupRecord  { cleanup_entry, dynamic_depth, previous }
HandlerRecord  { predicate, handler_entry, frame_address, previous }
ThreadContext  { catch, cleanup, handler }
```

The unwinder sets pending status, runs cleanup in LIFO order, restores bindings and handler depth, and transfers to the selected catch or handler. A cleanup exit replaces the old pending exit only after its record is linked. Rust frames are not machine-unwound: adapters return `NclStatus::NonLocalExit`, callers propagate it, and only the top NCL entry unwinds Lisp records. Panic is abort.

## SafepointMap and code object

Every map has a 16-byte little-endian header: `pc_offset:u32`, `frame_words:u16`, `slot_words:u16`, `word_slot_count:u16`, `register_mask:u16`, `map_flags:u32`. The bitmap uses bit 0 for header word 0 and bit 8 for the first local. Register ids follow the stable table. Flags are call, loop-backedge, allocation-slow, and has-derived-address at bits 0 through 3. PC lookup is binary search over code-relative offsets.

The code object stores entry, size, constant table, stack-map index, debug table, function name, source locations, frame size, and entry offset. The collector resolves a return PC from the function object's code object, finds the map, updates live slots and registers, and follows previous FP. Native transition records a conservative register spill area; JIT frames remain precise.

## Encoder verification

Instruction models reject register, immediate, and displacement overflow with `EncodeError`. Tests must cover instruction-byte golden files, encode/decode round trips, register/immediate/branch boundary values, and seeded randomized instruction sequences. Golden bytes are generated during development with Nix `llvm-mc` or target `as` plus `objdump`; the command and target triple are recorded in test metadata. Runtime never invokes an assembler. The in-tree decoder covers emitted instructions and prologue/epilogue. Golden results are checked against decoder, runtime smoke test, and external disassembler.

## FASL and object images

FASL is little-endian with a fixed 64-byte header.

| offset | size | field |
| ---: | ---: | --- |
| 0 | 8 | magic `NCLFASL\0` |
| 8 | 2 | version 1 |
| 10 | 1 | architecture, 1 x86-64, 2 AArch64 |
| 11 | 1 | pointer width 8 |
| 12 | 1 | endian 1 little |
| 13 | 1 | header size 64 |
| 14 | 2 | reserved |
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

Each relocation is 16 bytes: `u32 section`, `u32 offset`, `u32 kind`, `i32 addend`. Kinds are absolute Word, PC-relative branch, code-entry address, and external symbol. Sections are code, relocations, tagged constants, symbol bindings, stack maps, and debug records. Bounds and target metadata are checked before mapping.

Mach-O uses `__TEXT,__text`, `__DATA,__const`, `__DATA,__ncl`, declared dylibs, a symbol table, and native entry. ELF uses RX/RW `PT_LOAD`, `.text`, `.rodata`, `.ncl`, `.rela.text`, `.rela.ncl`, and native entry. Missing NCL metadata is rejected. `codesign --sign -` is delegated after final bytes on macOS arm64; JIT pages are governed by OS policy.

## JIT, diagnostics, and rejected designs

JIT memory follows RW, populate and relocate, then RX. macOS arm64 brackets writes with `pthread_jit_write_protect_np(0/1)` and invalidates the instruction cache. Linux never requests RWX and uses `mprotect` to RX. Publish the entry with a release store and read with acquire load.

`disassemble` prints bytes, symbolic branches, frame slots, and safepoint markers using the in-tree decoder. Backtrace resolves function and PC-relative source locations and stops safely when metadata is missing. External objdump is a test oracle, not a runtime dependency.

Rejected: delegating physical registers to a third-party backend, conservative-scanning all frames, using host object format as FASL, requiring objdump at runtime, implementing ad-hoc code signing, adding an eighth-word header, and adding an alternate builtin ABI. These conflict with the fixed register/map contract, moving heap, native metadata, platform signing, four-word frame, or direct builtin decision.

### Object and relocation loading sequence

1. Validate magic, version, architecture, pointer width, endian, and feature bitmap.
2. Validate every section offset and count against the input byte length.
3. Allocate the code object and register its constant table as a GC root.
4. Copy code bytes into writable code space.
5. Resolve symbol and object descriptors through Runtime and fill constant slots.
6. Apply relocations with checked arithmetic.
7. Install stack maps and debug records before publication.
8. Change permissions from RW to RX and publish the entry with release ordering.

The loader never treats a generic Mach-O or ELF file as an NCL image. A missing `.ncl` section, invalid map index, relocation outside its section, or mismatched target metadata is an error. The loader keeps the code object alive until all function objects and return-PC tables have been detached.

### Frame walking invariants

Previous FP is either null at native entry or points to an older frame. Return PC is interpreted only relative to the code object named by the function object. Flags distinguish Lisp, builtin, and native frames and carry tail and native state. A tail transfer does not manufacture a return frame. A malformed previous pointer terminates backtrace and GC walking safely.

The map's `frame_words` includes the complete frame span, while `slot_words` describes bitmap capacity. `word_slot_count` bounds live Word slots. A register mask identifies tagged Word values only; integer, floating-point, and raw address registers remain clear. Derived addresses set `has-derived-address` and are reconstructed only when the base slot is live.

### Direct builtin boundary

The fixed form passes `ctx` and typed Word arguments directly and returns one Word. The adapter form receives an argument count and contiguous argument pointer, writes multiple values through ThreadContext, and returns `NclStatus`. The adapter checks pending condition, interrupt, timeout, and non-local-exit state before returning. A builtin that calls back into Lisp propagates status at every boundary.

### Phase acceptance table

| phase | required artifact | acceptance |
| --- | --- | --- |
| 1a | fixed prologue and epilogue | constant return and `fib(5)` smoke |
| 1b | branch, call, local and tail transfer | `fib(25)` and multiple values |
| 1c | allocation and map | moving collection with live locals |
| 2 | FASL, image, diagnostics | compile-file, load, backtrace, disassemble |

No phase is accepted from an encoder byte comparison alone. The runtime smoke test must exercise published code, and the GC phase must move an object referenced by a live mapped slot.

### Encoder and fixup responsibilities

`MachineFunction` contains blocks, virtual values, physical assignments, frame layout, and safepoints. `Fixup` contains section, offset, kind, target, and addend. `CodeBlob` contains final bytes, fixups, entry offset, frame words, and map records.

The code generator validates every terminator, call signature, frame slot, and map before asking an encoder to emit bytes. The x86-64 encoder validates ModRM, SIB, displacement, and immediate widths. The AArch64 encoder validates instruction alignment, immediate fields, branch range, and register width. Both return structured errors and never silently truncate values.

Branch fixups are resolved after block layout. A short branch is selected only when its displacement fits; otherwise the long form is emitted. PC-relative values use the instruction end as the base. Absolute Word relocations target constant slots, not moving heap addresses. External symbols are resolved only by the object writer or runtime loader.

### Runtime publication protocol

Code allocation reserves non-moving code space and associates each range with its code object. During construction the range is writable and not callable. Constants, relocations, maps, and debug records are installed before permission changes. The final RX transition is followed by instruction-cache flush where required, then a release publication of the entry. Readers acquire the entry and never execute a partially initialized blob.

When a code object is released, its entry is first removed from the lookup table, active calls are quiesced, and only then is the range unmapped. A return PC from an unmapped range is treated as an invalid frame and never dereferenced. Static code and JIT code share map semantics even though their lifetime mechanisms differ.

### Debug metadata contract

The debug table maps code-relative ranges to source file id, line, column, form id, and function name. `disassemble` displays the same offsets used by PC binary search, so a printed safepoint can be checked against its bitmap. Backtrace reports frame kind, function object, entry-relative PC, and source location when available. Missing metadata produces an explicit unknown frame and stops the walk.

### Detailed lane deliverables

Lane 1a freezes the entry signature, four-word header, stack alignment, argument spill order, return registers, and direct builtin adapter.

Lane 1b implements block layout, local calls, full calls, self tail calls, general tail calls, branch fixups, and both target register tables.

Lane 1c implements allocation slow paths, write-barrier calls, RootToken transitions, map emission, register masks, derived-address flags, and moving-GC frame updates.

Lane 2 implements FASL serialization, image sections, relocation validation, code-space publication, disassembly, source locations, backtrace, and conformance fixtures.

Every lane preserves the same `Word` representation, logical entry order, physical RegisterId values, frame flags, map header, and error categories. A target-specific optimization is accepted only after the generic template, exact encoder bytes, map, unwind record, and runtime smoke test remain equivalent.

The implementation must also preserve 16-byte stack alignment at every call boundary.

The caller owns outgoing argument slots until the callee returns or tail transfer completes.

The callee spills callee-saved registers before publishing a safepoint.

The map index is immutable after code publication.

The constant table is scanned as part of the code object contract.

The relocation writer rejects integer overflow before calculating an address.

The loader rejects a relocation whose section is not declared by the header.

The JIT writer does not execute code while a page is writable.

The decoder reports unknown bytes instead of guessing an instruction.

The backtrace printer never follows an unvalidated frame pointer.

The conformance runner records target architecture with every backend fixture.
