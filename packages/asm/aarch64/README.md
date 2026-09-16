# ncl-asm-aarch64

Dependency-free AArch64 instruction model and encoder for the native backend.

Golden bytes were generated during development with `llvm-mc --triple=aarch64-apple-darwin --show-encoding`; runtime tests do not invoke external tools.

ADR and ADRP fixups encode split `immlo`/`immhi` fields and support signed backward references.
