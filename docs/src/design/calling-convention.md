# Calling convention

## 決定

NCL の Lisp entry は Cranelift の Signature で定義する。固定窓は (ctx: i64, argc: i64, a0: i64, a1: i64, a2: i64, a3: i64, rest: i64) -> (v0: i64, mv_count: i64) とする。ctx は ThreadContext、argc は実引数数、a0..a3 は最初の 4 引数、rest は caller が用意した残余引数配列への tagged Word address である。&rest/&key の解析と arity check は prologue で行う。Cranelift が ABI のレジスタ割当を決定し、任意の物理レジスタ名を契約にしない。

ThreadContext は Cranelift 0.134.3 の enable_pinned_reg と get_pinned_reg/set_pinned_reg を使い、x86-64 では r15、AArch64 では x21 に保持する。通常の引数と戻り値は SystemV または AppleAarch64 の target ABI、Lisp の本物の tail call は Tail call convention と return_call/return_call_indirect を使う。multiple values は v0 と mv_count の 2 戻り値とし、3 個目以降は ThreadContext の MV 領域へ格納する。

Cranelift が決める frame layout に NCL の 6 語 frame header を要求しない。handler chain、unwind-protect record、catch tag は ThreadContext にぶら下がる連鎖とし、record は stack slot address と dynamic depth を登録する。backtrace は preserve_frame_pointers と JIT module が管理する code-address -> function-object table で作る。active handler/cleanup 中の tail call は通常 call にする。

固定 arity builtin は extern C fn(ctx: *mut ThreadContext, a0: Word, a1: Word) -> Word のような直接 signature で登録する。condition または non-local exit は ThreadContext の pending flag と unbound marker 等の予約戻り値で伝える。可変長・keyword builtin だけが (ctx, argc, *const Word, *mut MultipleValues) -> NclStatus 形式を使う。builtin! は両形式を生成する。

Cranelift 0.134.3 の try_call/try_call_indirect は exception table の normal/exception destination と payload を生成するが、Cranelift は unwinder を実装せず exception tag の意味を embedder に委ねる。NCL はこれを Lisp unwinder の landing block として使う。tail call は caller に戻らないため catch site にできない。Rust は panic = abort とし、Lisp frame は NCL unwinder、Rust frame は NclStatus::NonLocalExit の戻り値伝播で脱出する。Rust builtin が funcall/apply/mapcar 等で Lisp を呼び返す場合、status/pending flag を必ず検査して上位へ返す。

## 根拠

固定窓を Cranelift signature にすれば引数を全て配列へ退避せず、物理レジスタ名にも依存しない。pinned register は context の常駐だけに使い、他の allocation は Cranelift に任せる。frame layout は backend が決めるため独自 header を ABI に埋め込まない。exception table の tag と unwinder を分ければ Cranelift の責務境界を越えない。

## 却下した代替案

- argc/r10、context/r14 の固定レジスタ表は Cranelift の一般 ABI 契約でないため却下した。
- 全 builtin を pointer-array ABI にする案は固定 arity の hot path で配列退避を要求するため却下した。
- Cranelift frame に 6 語 header を強制する案は backend の frame layout と競合するため却下した。
- Rust panic/unwind、trap の catch/throw 流用は Rust の abort 契約または Lisp payload と両立しないため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- ABI は上記 Cranelift signature、pinned register、固定 arity/可変長 builtin の二形式を使う。
- 物理レジスタ、Cranelift frame layout、Lisp handler chain を独自に固定しない。
- try_call の存在を unwinder 実装済みの意味に解釈せず、landing block と status propagation を実装する。
- Lisp 呼出しから戻る Rust builtin は status と pending flag を検査してから返す。
