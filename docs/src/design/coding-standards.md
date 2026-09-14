# Coding standards

## 決定

責務ごとの実装コンテキストは次の表で固定する。

| context | crates | 主な契約 |
| --- | --- | --- |
| unsafe platform | `ncl-sys` | pages、threads、OS declarations、roots、code space |
| typed object | `ncl-object`, `ncl-types` | Word、widetag、accessor、Runtime、ThreadContext wrapper |
| language front | `ncl-reader`, `ncl-printer`, `ncl-conditions`, `ncl-clos`, `ncl-compiler-front` | Form、condition、class、IR lowering |
| machine backend | `ncl-ir`, `ncl-codegen`, `ncl-asm-x86-64`, `ncl-asm-aarch64`, `ncl-objfile` | MachineFunction、encoding、Fixup、Relocation、CodeBlob |
| library | `ncl-lib-*` | ANSI/SBCL library surface |
| integration | `ncl-threads`, `ncl-ffi`, `ncl-image`, `ncl-runtime`, `ncl-conformance`, root `ncl` | execution, image, CLI, conformance |

外部 crate はゼロで、OS API は `ncl-sys` の宣言経由だけにする。safe crate は unsafe boundary を再宣言しない。公開型は所有権と root の責務を表し、heap `Word` を未登録 Rust container に保持しない。

## 根拠

境界を crate と型に対応させると、レビュー対象、依存方向、unsafe の範囲が一致する。低レイヤーを先に固定することで 24 以上の Phase 1 lane が同じ契約から作業できる。

## 却下した代替案

- backend 全体を一 crate にする案は encoder と object ownership を混ぜるため却下する。
- 単一の compiler-back 境界は target encoder と file writer の責務を曖昧にするため採用しない。
- unsafe helper を各 crate に分散する案は platform audit を困難にするため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- crate 名、path、依存方向、外部依存ゼロを変更しない。
- unsafe は `ncl-sys` に閉じ込め、public API の root と error semantics を省略しない。
- formatter、parser、test は対象 crate の最小範囲で実行し、設定ファイルを緩めて通過させない。
