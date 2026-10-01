# Image, startup, and executable scoreboard

測定日は 2026-10-01、macOS arm64、同一 worktree の release build です。比較対象は `conformance/baselines/startup-and-size-sbcl-2.6.0.md` の SBCL 2.6.0 baseline です。

## 指標

| 指標 | NCL 現状 | SBCL baseline | 判定 |
| --- | ---: | ---: | --- |
| `ncl --eval 1` wall-clock median | 78.309 ms | 17.331 ms | 未達 |
| release executable | 2,552,176 bytes | 82,501,760 bytes | 達成 |
| cl-bench `files/*.lisp` compile-file median | 未計測 | 0.08 s | 未計測 |

NCL の起動測定は 11 回、最小 66.689 ms、最大 116.197 ms。各試行は `target/release/ncl --eval 1` を別プロセスで起動し、出力 `1` と exit 0 を確認した。実行ファイルサイズは `stat -f '%z %N' target/release/ncl` で測定した。

## 実行コマンド

```sh
nix develop --command cargo build --release --locked
stat -f '%z %N' target/release/ncl
/usr/bin/perl -MTime::HiRes=time -e '$n=11; @x=(); for(1..$n){$t=time; system @ARGV; push @x,(time-$t)*1000} @x=sort{$a<=>$b}@x; printf "median=%.3f min=%.3f max=%.3f\\n",$x[int($n/2)],$x[0],$x[-1]' target/release/ncl --eval 1
```

ビルドは exit 0、11 試行はすべて exit 0。cl-bench の固定 checkout と `files/*.lisp` はこの worktree に存在せず、compile-file の acceptance measurement は実行していない。既存の conformance driver はロード/codegen failure によりベンチ本体へ到達しないため、代用値として扱っていない。

## 律速と実装判断

`packages/runtime/src/lib.rs:52-75` の `Runtime::new()` は object runtime、builtin trampoline、標準ライブラリ全登録、追加 builtin 登録を毎回実行する。`packages/image` の save/load は caller-provided roots の graph と一部 code bytes の形式であり、runtime registry、constant relocation、safepoint/debug metadata、whole-heap snapshot を保存しない (`packages/image/src/lib.rs:28-46`)。したがって現状 API を CLI 起動 image として接続する変更は、起動短縮を正しく保証できず、保存 code を executable 化する信頼境界も拡大する。今回は接続を行わず、native FASL/image の registry・relocation 契約を先に拡張することを残作業とした。

現行 FASL は source payload を保存し、load 時に再コンパイルする (`packages/runtime/src/compile.rs:292-309`, `packages/runtime/src/load.rs:15-26`)。compile-file の cl-bench 測定ができる固定入力を取得できるまで、律速の断定や改善値の主張はしない。
