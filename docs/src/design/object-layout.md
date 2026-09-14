# Object layout

## 決定

NCL の値は repr(transparent) の Word(u64) である。fixnum は bit 0 が 0 の n << 1 で、符号付き 63-bit payload、most-positive-fixnum = 2^62-1 = 4611686018427387903 とする。bit 0 が 1 の語は bit 1..3 の lowtag で分類する。

| bit 1..3 | 種別 | 表現 |
| --- | --- | --- |
| 000 | character immediate | bit 4..24 に Unicode scalar value |
| 001 | list pointer | 8-byte aligned address + lowtag。cons はヘッダなし 2 語 |
| 010 | single-float immediate | bit 4..35 に IEEE-754 binary32 |
| 011 | function pointer | simple-fun または closure object |
| 100 | other-immediate | bit 4..63 に payload。unbound marker を含む |
| 101 | instance pointer | structure または CLOS instance |
| 111 | other pointer | symbol、string、vector、number、package 等 |

即値は lowtag 000 の character、010 の single-float、100 の other-immediate とする。unbound marker は other-immediate の固定値とし、NIL と T は即値にしない。

consp は list lowtag、listp は NIL 比較または list lowtag、fixnump は bit 0、characterp/single-float-p は respective immediate lowtag、functionp は function lowtagだけで判定する。symbolp、stringp、simple-vector-p は other lowtag の後の 1 語 widetag を読む。

ヘッダ付き object のヘッダは 1 語固定で、bit 0..7 を widetag、bit 8..15 を GC flags、bit 16..63 を size/length とする。flags は young、marked、forwarded、finalizable、weak、hashed と予約 bit。世代と pin はページ metadata に置き、pin は世代ではない。cons はヘッダなしの (car, cdr) 2 語で、cons 専用ページのページ種別から GC が走査方法を決める。

静的領域には NIL と T の symbol object を起動時に固定配置する。GC は領域を走査するが移動しない。NIL は list lowtag として見た car/cdr が自分自身を指すレイアウトにし、symbol の value/function cell 位置と整合させる。simple-fun と closure は別 widetag とし、closure 値は object 内に inline 配置する。

symbol は value、function、plist、package、name、tls_index: u32、identity-hash slot を持つ。structure/CLOS instance は layout 経由の slot に identity hash を置ける。それ以外の eq hash key はアドレス hash とし、移動時に hashed flag の表へ再ハッシュ通知する。全 object に hash 語は追加しない。bignum は GC ヒープ上の little-endian u32 limb 配列である。package、symbol table、intern は ncl-object の責務、ncl-lib-packages は builtin 登録だけを担う。

## 根拠

bit 0 を fixnum に専有すれば 62-bit の符号付き値域を保ちつつ、残りを immediate と 4 種の pointer lowtag に使える。頻出の cons/function 操作からヘッダ読みを除き、NIL/T を通常の symbol にすれば全 symbol accessor と car nil/cdr nil に特別分岐を追加しない。

## 却下した代替案

- 全値を 3 bit tag で分ける案は fixnum と immediate が衝突するため却下した。
- NIL/T の即値化は symbol accessor と cons accessor の分岐を増やすため却下した。
- cons header、全 object の hash: u64、closure vector の二段間接参照はサイズまたは hot path を悪化させるため却下した。
- intern を ncl-lib-packages に置く案は reader/printer/lib の責務を分断するため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- Word、lowtag、widetag、cons 2 語、NIL/T 固定配置を変更しない。
- Word を生の u64 として公開せず、object pointer を Rust reference として allocation point 越しに保持しない。
- 新しい object type、tag、header flag、hash 方式はこの文書を先に更新する。
