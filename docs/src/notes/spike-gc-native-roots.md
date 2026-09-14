# GC native roots spike

2026-09-15 に macOS arm64 で、保守的な native stack root、ページ pin、移動 nursery、native transition、weak/finalizer の最小試作を行った。試作はリポジトリ外の `~/.cache/ncl-spikes/gc-native-roots/` に置いた。Linux x86-64 は API 差分のため未実行である。

## 結果

| 項目 | 判定 | 根拠と制約 |
| --- | --- | --- |
| stack 境界・レジスタ | 制約付きで動いた | macOS の `pthread_get_stackaddr_np` / `pthread_get_stacksize_np` を `agent-12/src/main.rs:56-70` で取得し、協調 poll 内で AArch64 x19〜x28 を退避した (`:78-117`)。Linux の `pthread_getattr_np` は未実行。 |
| 保守的走査 | 制約付きで動いた | lowtag 除去後にページ表とオブジェクト開始位置を確認する (`agent-12/src/main.rs:28-52`)。固定 seed、ランダム語 1,000,000 語、lowtag 候補 500,159、開始位置6、通過0で、観測誤検出率は `0 / 500159 = 0.000000000`。実ヒープ密度を代表する測定ではない。 |
| pin と移動 nursery | 動いた | `Word`、`PageKind::{Cons,Object}`、世代、ページ bump 割当を `agent-3/src/lib.rs:8-65, 134-157` に実装。保守的 root のページを pin (`:161-171`)、精密 root は copy+forward と推移参照更新 (`:173-244`)。4 tests passed。cons はヘッダなし2語、object は header 付きでページ種別を分離した。 |
| Rust builtin の allocation point | 制約付きで動いた | `fn car(thread: &mut Thread, w: Word) -> Word` で `black_box(w)` を保持し、強制GCを跨いで値を返す試作 (`ergonomics/src/main.rs:31-40`) は `cargo run` で `forced_gc=1`。これは実スタック走査の証明ではなく、Rust が値をレジスタだけに置く場合の保証にもならない。Phase 1 は `RootToken` を標準経路とし、poll で全 callee-saved を退避する補助策を採用する。 |
| enter_native / leave_native | 動いた | `agent-56/src/lib.rs:231-245`、2 thread の sleep と GC 完了を `:265-282` で検証。native thread は safe state となり、GC 完了まで復帰しない。2 tests passed。loom、sanitizer、長時間 stress は未実行。 |
| weak / finalizer | 動いた | weak の移動更新/NIL 化は `agent-56/src/lib.rs:159-164`、finalizer の一回だけの enqueue/drain は `:106-128, 285-305`。weak は強い root ではなく、finalizer queue は強い root とした。 |

## 試作 API と unsafe

ページ試作の中心 API は `Heap::alloc_cons`、`Heap::alloc_object`、`Heap::minor_gc(&mut self, precise: &[Word], conservative: &[Word])` 相当で、`PageKind` が cons の2語走査と header object の走査を選ぶ。native transition 試作は `Thread::enter_native`、`Thread::leave_native`、`Runtime::collect` を持つ。実契約の `Word alloc(&mut ThreadContext, &Runtime, TypeTag, usize)`、`push_root`、`poll_safepoint` に対応させる際は、allocation point を跨ぐ Rust slot を明示 root にする。

agent-12 の unsafe は4ブロック。pthread API 呼出し3箇所 (`src/main.rs:64-66`) は macOS API と thread identity が有効であること、inline asm (`:91-101`) は poll が停止を観測した後に x19〜x28 を保存することを不変条件とする。agent-3 と agent-56 は unsafe 0だった。したがって、実 runtime で stack memory を dereference する処理は新たな unsafe 境界として、範囲内、停止済み、語幅整合、ページ表の寿命を明示的に検査する必要がある。

## 契約文書への修正案

- `gc-interface.md`: native stack root は「保守的に走査」とだけ書かず、macOS arm64 の stack bounds API、協調 poll、レジスタ退避、ページ内の有効 object start/interior 判定、pin 単位がページであることを API 契約に追加する。Rust builtin の `Word` ローカルは保守的走査に依存せず `RootToken` を要求し、保守的 root は pin されるため移動後の slot 更新対象外、と明記する。
- `threads.md`: `poll_safepoint` が停止要求を観測して callee-saved register snapshot と native stack bounds を公開する状態遷移を追加する。`enter_native` 後は GC 完了まで thread が safe state に留まり、`leave_native` が復帰点になること、任意の native instruction を割り込まないことを明記する。
- `object-layout.md`: conservative candidate は lowtag だけでは root にせず、ページ表、ページ種別、object start または object interior の逆引きに通過した場合だけ採用することを追加する。cons のヘッダなし2語と header object の混在は同一ページに置かず、pin metadata は page metadata に置くことを固定する。
- `crates.md`: `ncl-sys` の unsafe 責務に stack bounds/register snapshot、page pin、conservative scan を列挙し、他 crate は unsafe を禁止する。weak/finalizer の更新・一回性と native transition の責務を `ncl-sys` API surface に追加する。

## L1 の最初の落とし穴

1. lowtag が正しいだけの整数や interior pointer を object と誤認しない逆引き表が必要。
2. 保守的 root が指すページを pin するとページ全体が移動せず、精密 root と同じ forwarding 前提で slot を更新すると破綻する。
3. `Word` を Rust の `Vec`、`HashMap`、static、`Box` に置いたまま allocation point を跨がない。必要なら先に `push_root` する。
4. `black_box` や callee-saved 退避は ABI/compiler の保証を補助するだけで、任意の Rust ローカルが stack に存在する保証ではない。
5. blocking primitive は poll、`enter_native`、interrupt/deadline protocol を一組で実装し、GC mutex を保持したまま待たない。
6. weak を強く root 化せず、finalizer queue だけを強く保持し、enqueue と callback 実行を一回に制限する。

## 検証

各 spike は指定 worktree を開発環境として、リポジトリ外の manifest を対象に実行した。agent-12 は `cargo fmt --check`、`cargo check`、`cargo run` が exit 0、`cargo test` は exit 0 だが0 tests選択。agent-3 は `cargo fmt -- --check` と `cargo test --all-targets` が exit 0、4 tests passed。agent-56 は `cargo fmt -- --check`、`cargo test --all-targets`、`cargo clippy --all-targets -- -D warnings` が exit 0、2 tests passed。ergonomics は `cargo run` exit 0、`cargo test` は exit 0だが0 tests選択。`nix develop path:<worktree>` の文字列形式はこの環境の Nix 2.34.8 で相対解釈され exit 1 となったため、同じ worktree の絶対パス installable 形式で再実行した。

実 repository の `ncl-sys` 統合、Linux 実行、sanitizer/loom、長時間 concurrency stress、実 native stack map との接続はこの spike の対象外であり、動いたとは判定していない。
