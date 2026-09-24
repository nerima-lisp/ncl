# Native backend

## 契約の範囲

64-bit little-endian macOS/Linux の `ncl-ir` を x86-64 または AArch64 machine code へ変換する。第三者の compiler、loader、C library wrapper は使わず、OS ABI は `ncl-sys` の自前 `extern "C"` 宣言だけを経由する。

## 決定

`ncl-codegen` は `MachineFunction` を作り、asm crate が bytes と `Fixup` を生成し、`ncl-objfile` が `Relocation` と FASL/object sections に変換する。asm crate は `Reg`、`Inst`、`Fixup`、`EncodeError` を自前定義し何にも依存しない。code object は `ncl-sys` 管理の非移動 code space に page 単位で置き、未参照後に解放する。GC は code object 内の constant slots だけを更新する。

frame header は 4 語、`previous FP / return PC / function object / flags` とする。function object は移動 heap 上の精密 root であり、return PC は非移動 code space のため更新しない。handler、cleanup、catch は ThreadContext の 3 本の現在ポインタで record chain を形成し、全 record は frame address と dynamic depth を持つ。unwinder は record chain、backtrace と GC scan は frame header chain を辿る。

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

SafepointMap は 16-byte header、slot bitmap、`u16` register id 列である。header は `pc_offset u32, frame_words u16, slot_words u16, word_slot_count u16, register_mask u16, map_flags u32`。bitmap bit 0..3 は frame header 語 0..3、bit 2 は常に 1、他は常に 0、bit 4 は最初の local slot である。列挙するのは `Word` を持ちうるレジスタだけである。FASL header は 64 byte、magic `NCLFASL\0`、version 1、architecture、pointer width 8、little-endian、feature bitmap、各 section offset/size を固定する。version 2 は frame-state section を加える(Phase 5)。version 1 の loader は version 2 を拒否する。

phase は compiler-pipeline と同じく、1a 固定テンプレート展開、1b 線形走査レジスタ割当と spill、1c self tail call と一般 tail transfer および `&rest`/`&key` 専用プロローグ、2 fixnum/double の unbox、型推論接続、inline cache とする。3〜6 は compiler-pipeline.md に記述されている。1a はスタックスロット、scratch 2 本、プロローグ/定数/return/呼び出し/分岐/割当/safepoint map/unwind を含む動く系である。Phase 1 は性能を主張しない。tail transfer は frame header と dynamic records の条件を満たす場合だけ行い、active cleanup では禁止する。

## 根拠

非移動 code space は return PC と絶対 branch target の GC relocation を排除する。直接 fixed builtin call は ABI の引数・戻り値を machine contract に揃える。record chain を header から分離すると call ごとの store 数を抑えつつ dynamic unwind を明示できる。

## 却下した代替案

- 8 語 frame header は毎 call の store 数が多いため却下する。
- code object を moving heap に置く案は return PC を再配置するため却下する。
- 全 builtin を args pointer adapter にする案は fixed call の余計な materialization を生むため却下する。
- encoder が allocation、OS call、GC を行う案は bounded context を混ぜるため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- 表のレジスタ、4 語 header、16-byte map、64-byte FASL header、phase 順序を変更しない。
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

The four implementation lanes are frozen as follows.

| lane | file count | type count | frozen concern |
| --- | ---: | ---: | --- |
| ABI/frame/GC metadata | 8 | 14 | entry ABI, four-word frame, maps, unwind records |
| x86-64 | 10 | 16 | register model, encoder, disassembler, fixups |
| AArch64 | 10 | 16 | register model, encoder, disassembler, fixups |
| object/FASL/JIT adapter | 12 | 20 | sections, relocation, publication, loader adapter |

Lane order is strict: the ABI lane freezes `MachineFunction` and the metadata schema first. The object lane starts only after `CodeBlob` and `Relocation` are frozen. The bounded contexts are Reg/Inst/Fixup/disassembler; Section/Relocation/Mach-O/ELF; and W^X/icache/dlopen.

The lane table above is the only lane partition. Phase definitions come from
`compiler-pipeline.md`: 1a is the complete fixed-template running system, 1b
is linear-scan allocation and spill, 1c is tail transfer plus specialized
prologues, and 2 is unboxing, type-inference connection, and inline cache.
Lane outputs are `MachineFunction`, `Fixup`, `CodeBlob`, and `SafepointMap`,
with stable field order and error semantics.

## Minimal public API

```text
MachineFunction lower(Function, Target) -> Result<MachineFunction, CodegenError>
CodeBlob encode(MachineFunction, Target) -> Result<CodeBlob, EncodeError>
Vec<u8> write_fasl(FaslInput) -> Result<Vec<u8>, ObjfileError>
SafepointMap build_map(FrameLayout, LiveValues) -> Result<SafepointMap, MapError>
map_and_publish(CodeBlob, LinkTable, &mut ThreadContext) -> Result<JitHandle, SysError>
```

The encoder has no allocator, OS call, symbol lookup, or GC dependency. `ncl-objfile` emits bytes only and does not call the OS. A code blob contains bytes, relocations, entry offset, frame size, and maps. Relocations are applied through constant slots, never by embedding moving heap addresses in instruction bytes.

## Physical convention

The logical entry is `(ctx, argc, a0, a1, a2, a3, rest) -> (v0, mv_count)`. x86-64 uses `rax=0`, `rdx=1`, `rdi=2`, `rsi=3`, `rcx=4`, `r8=5`, `r9=6`, `r10=7`, `r11=8`, `rbp=9`, `rbx=10`, `r12=11`, `r13=12`, `r14=13`, `r15=14`. `r15` is reserved for ThreadContext; r10/r11 are scratch. AArch64 uses x0..x30 with the same RegisterId numbering, x21 pinned for ThreadContext, x16/x17 scratch, and x29 as frame pointer.

The frame header is four words: previous FP, return PC, function object, and flags. Flags encode frame kind, tail state, and native state. Argument spills, locals, and outgoing slots follow; each slot is 8 bytes and total frame size is 16-byte aligned. Five or more arguments occupy caller outgoing slots. Full calls create frames, local calls target within the code object, self tail calls reuse the frame, and general tail calls transfer after cleanup. Active cleanup forbids tail transfer.

Direct fixed-arity builtins receive a direct `extern "C"` signature. Variadic and keyword builtins use `(ctx, argc, args, mv) -> NclStatus`. `builtin!` produces either form. A pending flag is checked after return. Multiple values use v0 plus count and the ThreadContext MV area.

## Non-local exit records

```text
CatchRecord    { tag, target_frame, target_pc, value_slot, depth, previous }
CleanupRecord  { cleanup_entry, frame_address, depth, previous }
HandlerRecord  { predicate, handler_entry, frame_address, depth, previous }
ThreadContext  { catch, cleanup, handler }
```

The unwinder sets pending status, runs cleanup in LIFO order, restores bindings and handler depth, and transfers to the selected frame address and `target_pc`. A cleanup exit replaces the old pending exit only after its record is linked. Rust frames are not machine-unwound: adapters return `NclStatus::NonLocalExit`, callers propagate it, and only the top NCL entry unwinds Lisp records. Panic is abort.

## SafepointMap and code object

Every map has a 16-byte little-endian header: `pc_offset:u32`, `frame_words:u16`, `slot_words:u16`, `word_slot_count:u16`, `register_mask:u16`, `map_flags:u32`. The bitmap uses bits 0..3 for header words 0..3, with bit 2 always one and the other header bits always zero, and bit 4 for the first local. The function object in header word 2 is forwarded during frame walking; return PC is not. Register ids are `u16` and list only registers that can hold `Word`. Flags are call, loop-backedge, allocation-slow, and has-derived-address at bits 0 through 3. PC lookup is binary search over code-relative offsets.

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
| 1a | fixed template expansion | native entry executes `(+ 1 2)`, cons, branch, builtin call, safepoint, and `fib(25)`; performance is not measured |
| 1b | linear-scan allocation and spill | representative benchmarks are no slower than template version |
| 1c | self/general tail transfer and specialized prologues | self recursion depth 1,000,000 adds no frames |
| 2 | unbox, type inference, inline cache | same-machine `fib(25)` is within 2x SBCL, with commit hash and conditions recorded |

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

Lane 1a freezes the entry signature, four-word header, stack alignment, argument spill order, return registers, direct builtin adapter, constants, calls, branches, allocation, maps, and unwind.

Lane 1b implements block layout, local calls, full calls, self tail calls, general tail calls, branch fixups, and both target register tables.

Lane 1c implements self and general tail transfer, specialized `&rest`/`&key` prologues, allocation slow paths, write-barrier calls, RootToken transitions, map emission, register masks, derived-address flags, and moving-GC frame updates.

Lane 2 implements fixnum/double unboxing, type-inference connection, inline-cache dispatch, FASL serialization, image sections, relocation validation, code-space publication, disassembly, source locations, backtrace, and conformance fixtures.

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

### Codegen runtime ABI

`ncl-codegen::RuntimeAbi` is the boundary between lowering and the runtime lanes. The code generator requests byte offsets for the TLAB bump and limit, safepoint request bits, pending condition state, multiple-value count and area, and the current handler, cleanup, and catch records. It requests addresses for the allocation slow path, safepoint slow path, non-local-exit unwinder, builtin entry, and constant table. The code generator does not assume a `ThreadContext` Rust layout or embed a runtime address.

`ncl-object` supplies the typed `ThreadContext` view and builtin signatures. `ncl-sys` supplies code-space allocation and publication, safepoint delivery, heap allocation, and the eventual field layout used to implement these ABI requests. Until those fields are exposed as stable offsets, an embedding runtime must return `None` and codegen reports the operation as unavailable.

The AArch64 target uses x21 for context, x0 for entry `argc` and return value, x1 for multiple-value count, x1..x4 for the first four logical arguments, x5 for `rest`, x16/x17 as scratch, and x29/x30 for frame and link. Its fixed-template backend emits four-byte aligned code and retains the same safepoint map wire format and frame header as x86-64.

Phase 1a's AArch64 lowering has two allocation paths. The fast path loads `tlab_bump` and `tlab_limit`, checks `bump + words * 8 <= limit`, returns the old bump, and stores the advanced bump. The slow path places the pinned context in `x0`, the requested word count in `x1`, and branches through the `RuntimeFunction::AllocateSlow` address. Safepoint lowering loads the `safepoint_request` word and uses `cbz` to skip the slow path when it is zero; otherwise it passes the pinned context in `x0`, the active generated frame pointer (`x29`) in `x1`, and the continuation PC in `x2` (loaded with `adr` as the address immediately after `blr`, which is the safepoint map `pc_offset`), then branches through `RuntimeFunction::SafepointSlow`. (`packages/codegen/src/target_aarch64_lowering.rs`, `lower_alloc`, `lower_safepoint`.)

The four-word frame header and 16-byte alignment remain the native contract. Generated code stores the function object in header word 2: a native call places the callee function object in `x16`, and the callee prologue stores `x16` in word 2. The Phase 1 collector snapshots the top frame's four-word header from the frame pointer the slow path passed in, forwards header word 2 (the header slots the map marks live) and writes the forwarded values back into the real frame; locals and outgoing slots beyond the four-word header are not captured by the `set_native_frame` path in Phase 1 (`packages/sys/src/thread.rs`, `set_native_frame`, `write_back_frame_snapshot`.) Word 1 (the return PC) points into non-moving code space and is not written back. Walking older frames and forwarding register values are outside Phase 1 and remain Phase 1b work. The conformance test is `forwards_function_object_from_real_frame_after_safepoint_collection` (`packages/codegen/tests/exec_aarch64/cons.rs`.)

Known limitations at the Phase 1 landing point ((b) は Phase 1b(L13)で解消する):

- (b) only the top frame is walked; the real previous-fp in word 0 is not followed;
