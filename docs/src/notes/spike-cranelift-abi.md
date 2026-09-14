# Cranelift 0.134.3 ABI spike

## 実行条件

本機の macOS arm64、Nix flake の devShell（Rust 1.98.0）から、リポジトリ外の独立 Cargo プロジェクトで実行した。試作の説明上の場所は ncl-spikes/cranelift-abi である。依存 crate は codegen、frontend、jit、module、native をすべて =0.134.3 に固定した。

実行コマンドは次の通りで、終了 status は 0 だった。

    nix develop <worktree> --command cargo run --manifest-path <external-spike>/Cargo.toml

出力は version=0.134.3 identity=(1, 2)、mutual_return_call_steps=1000000 result=0、pinned_set_get=305419896、builtin_result=37 だった。Linux x86-64 は実行していない。

## 確認結果

### 1. Tail 規約、末尾呼び出し、複数戻り値

**動いた。** Signature::new(CallConv::Tail) に i64 の引数7個 (ctx, argc, a0..a3, rest) と戻り値2個 (v0, mv_count) を登録でき、return_ で戻せた。Tail 関数を SystemV のホスト wrapper から呼び、戻り値 (1, 2) を得た。

別の Tail 規約関数2個を return_call で相互再帰させ、各回で引数を1減らして 1,000,000 回実行した。結果は 0 で、プロセスはスタックオーバーフローなしに終了した。実装の要点は Tail Signature、module.declare_func_in_func、b.ins().return_call である。

注意点として、Tail 関数の finalized pointer を Rust の extern "C" として直接呼ぶ試験は不正な ABI 呼び出しになり、戻り値破壊と終了時の malloc エラーを起こした。外部からは SystemV/AppleAarch64 wrapper を置いて Tail 関数を呼ぶ必要がある。

### 2. pinned register

**制約付きで動いた。** enable_pinned_reg=true を設定し、Tail 関数内で set_pinned_reg(value)、続けて get_pinned_reg(types::I64) を実行できた。SystemV wrapper 経由で 0x12345678 を設定し、305419896 を読み戻した。

この試作では Tail 関数間の呼び出しと Rust builtin 呼び出しを同じ pinned 値で跨ぐ試験までは実施していない。したがって「Rust の extern C が pinned register を保全する」という契約は未確認である。pinned register の物理名を契約にする根拠にもならない。

### 3. Rust builtin 呼び出し

**動いた。** JITBuilder に ncl_builtin を登録し、SystemV の imported signature fn(i64, i64, i64) -> i64 として Tail 関数から通常の call で呼んだ。実体は Rust の extern C fn(*mut u8, u64, u64) -> u64 で、10 + 20 + 7 = 37 を戻した。

これは固定 arity builtin の直接呼び出しが macOS arm64 の本機で動くことの確認である。Tail 関数から builtin への呼び出しは return_call ではなく通常の call にしている。

### 4. user stack map と移動模擬

**未実行。** 0.134.3 の frontend には declare_value_needs_stack_map があり、非 tail call は safepoint として扱われる。producer は値を stack slot に spill し、call instruction に stack-map entry を付ける責任がある。

codegen の MachBufferFinalized::user_stack_maps() は (CodeOffset, span, UserStackMap) を返すが、cranelift-jit の通常の JITModule API から CompiledCode またはこの finalized buffer を取得し、実行時 PC から逆引きする経路はこの試作では実装していない。Rust callee から caller frame の slot address を得て値を書き換える試験も未実行である。

よって gc-interface.md の「collector は stack map ... で roots を更新する」は現時点では producer metadata の設計前提に留め、実行時の PC table、frame layout、slot address 計算を backend/runtime の追加契約にする必要がある。

### 5. try_call と unwinder

**未実行。** 0.134.3 の IR には try_call と ExceptionTable（normal/exception destination、tag、context）がある。しかし Cranelift は unwinder を提供せず、例外 tag の意味と payload 処理も embedder の責任である。

この試作では Rust の例外発生、SP/FP 復元、payload 受け渡し、JIT landing pad への実帰還を行う unwinder は実装していない。したがって try_call が NCL の非局所脱出を実機で成立させること、Tail 規約と組み合わせられること、macOS arm64 で実帰還できることは未確認である。return_call は caller が stack 上に残らないため catch site にはできない、という制約だけは IR の設計から確認できる。

現行契約の「try_call ... landing block として使う」は、Cranelift が unwinding を実施するように読めるため修正が必要である。Phase 1 は当面 NclStatus::NonLocalExit と pending flag の戻り値伝播を主経路にし、実 unwinder を採用するなら ISA ごとの unwind ABI、exception-table registry、FP chain、payload ABI を別設計してから実装する。

### 6. cranelift-jit の macOS arm64 メモリ

**制約付きで動いた。** 上記 JIT 実行は macOS arm64 で動作した。cranelift-jit 0.134.3 の system memory allocator は memmap2::MmapMut::map_anon を使い、依存する region の Unix allocator は arm64 hardened runtime 向けに MAP_JIT を mmap flags に追加している。試作側で pthread_jit_write_protect_np を呼ぶ設定は不要だった。

したがってこの構成では MAP_JIT の切り替えは cranelift-jit の依存 allocator 側が担う。embedder が独自にコード領域を mmap する場合は別途 W^X 管理が必要であり、NCL の契約に「embedder が常に MAP_JIT を設定する」と書くべきではない。

### 7. 版と ISA フラグ

**動いた。** Cargo は全 Cranelift family を 0.134.3 に解決した。cranelift_native::builder() から native ISA builder を作り、次の shared settings を設定して finish できた。

    opt_level = speed
    enable_pinned_reg = true
    preserve_frame_pointers = true
    unwind_info = true

frontend の finalize には isa.frontend_config() を渡す必要がある。0引数の FunctionBuilder::finalize() は 0.134.3 では存在せず、TargetFrontendConfig が必要だった。

## 契約文書の修正案

### docs/src/design/calling-convention.md

- Tail 関数の入口を Rust extern C pointer として直接呼べる、という含意を削除し、SystemV/AppleAarch64 wrapper からのみ呼ぶと明記する。
- enable_pinned_reg は get_pinned_reg/set_pinned_reg の生成機構として記述し、物理レジスタ名と Rust callee-saved 性を保証しないと明記する。
- try_call は exception table と landing block を生成するだけで、unwinder、payload ABI、FP復元を提供しないと書き換える。実 unwinder 未検証の間は NclStatus::NonLocalExit を必須の実装経路にする。

### docs/src/design/gc-interface.md

- user stack map は「値を宣言すれば collector が実行時に slot を発見できる」という短い記述を改め、producer の spill、safepoint call への entry 付与、code address と compiled-code metadata の registry、PC lookup、slot offset の解釈を必須段階として列挙する。
- cranelift-jit の JITModule API だけでは runtime stack-map lookup が完結しないため、NCL が code metadata table を所有する契約を追加する。
- 今回は移動模擬が未実行なので、実機検証済みの前提として扱わない。

### docs/src/design/compiler-pipeline.md

- backend の Safepoint lowering に、declare_value_needs_stack_map、live value の stack slot spill、non-tail call への entry、compiled-code metadata の登録を具体的な成果物として追加する。
- try_call の lowering は exception table の構築までとし、unwinder は compiler pipeline の機能ではなく runtime/ABI 設計の別成果物に分離する。
- Tail 関数の外部テスト入口には host-ABI wrapper を生成する規則を追加する。

## L7 と L1 の最初の落とし穴

- L7: Tail 関数の finalized pointer を C ABI として直接呼ばない。host wrapper と Tail Signature を分ける。
- L7: FunctionBuilder::finalize(isa.frontend_config()) を忘れない。0.134.3 の API は引数必須である。
- L7: try_call を unwinder と誤認しない。例外 table は metadata であり、SP/FP 復元と payload ABI は別実装である。
- L1: declare_value_needs_stack_map だけでは runtime root scanning は完成しない。producer の spill と PC→map lookup が必要である。
- L1: Rust の Vec<Word>、builtin の引数領域、pinned register を stack map の代替にしない。未登録の保存場所は移動 GC の更新対象にならない。
- 両レーン: pinned register の物理名を固定契約にせず、ISA flags と Cranelift の value API の境界で扱う。

## 一次資料

- Cranelift 0.134.3 settings: https://docs.rs/cranelift-codegen/0.134.3/cranelift_codegen/settings/struct.Flags.html
- Cranelift user stack maps source: https://docs.rs/cranelift-codegen/0.134.3/src/cranelift_codegen/ir/user_stack_maps.rs.html
- Cranelift 0.134.3 crate source: https://docs.rs/crate/cranelift/0.134.3/source/
