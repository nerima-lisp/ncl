再現コマンド: `sbcl --non-interactive --eval '(format t "~A~%~S~%" (lisp-implementation-version) *features*)' --quit`

# Measurement environment

- 測定日: 2026-09-15
- OS: macOS Darwin、arm64
- CPU: Apple Silicon、16 logical CPUs (`sysctl -n hw.model` は公開用識別子として省略)
- メモリ: 137438953472 bytes (128 GiB)
- SBCL: 2.6.0
- features 要約: `:ARM64 :DARWIN :SB-THREAD :SB-UNICODE :SBCL :UNIX :COMMON-LISP :ANSI-CL :64-BIT`

値は `sysctl -n hw.ncpu` と `sysctl -n hw.memsize` の実測です。
