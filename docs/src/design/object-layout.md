# Object layout

## 決定

`Word` は 64-bit tagged value で、NIL と T は静的に固定配置する。cons は header なしの `(car, cdr)` 2 語、header object は 1 語 header の後ろに payload を置く。bit 0..7 は widetag、bit 8..15 は GC flags、bit 16..63 は size/length とする。generation と pin は page metadata に置く。

lowtag の契約は次のとおりである。`listp` は list lowtag の検査だけ、`consp` は list lowtag かつ NIL でない値、`symbolp` は NIL または other-pointer と symbol widetag の組み合わせを検査する。character、single-float、function は対応する immediate/function lowtag を使う。unbound marker は予約済み other-immediate である。

symbol は value、function、plist、package、name、`tls_index: u32`、identity-hash slot、flags word を持つ。flags word は bit 0 special、bit 1 constant、bit 2 macro、bit 3 package-lock、残りを予約とする。cons 専用 page と header-object page は混在させず、pin は page attribute とする。

所有境界は [GC interface](gc-interface.md) と [Native backend](native-backend.md) に従う。heap、code space、per-thread roots は `ncl-sys` が所有し、`ncl-object` は widetag、accessor、symbol/package/intern、`Runtime`、`ThreadContext` wrapper、register 型を提供する。weak pointer は強 root ではなく、到達不能後の finalizer queue だけを強く保持する。

## 根拠

NIL を list lowtag として扱うことで list predicate を高速にし、symbol predicate だけは NIL の言語仕様を明示できる。固定 header と page 種別は collector が payload の解釈を推測せずに済む。移動対象と code space を分けることで return PC の再配置を不要にする。

## 却下した代替案

- 全 object に hash word を追加する案は payload と GC scan を膨らませるため却下する。
- cons と header object を同一 page に置く案は scan mode を曖昧にするため却下する。
- pin を generation として表す案は collector の状態を誤分類するため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- lowtag、widetag、header bit 割当、cons 2 語、symbol flags の値を変更しない。
- Rust の未登録領域に heap `Word` を保持せず、移動 object の address を code bytes に埋め込まない。
- accessor は object ownership を越えて raw OS API を直接呼ばない。
