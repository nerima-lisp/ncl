#![expect(
    clippy::bad_bit_mask,
    clippy::cast_sign_loss,
    clippy::fn_params_excessive_bools,
    clippy::if_same_then_else,
    clippy::manual_ilog2,
    clippy::missing_const_for_fn,
    clippy::similar_names,
    clippy::doc_markdown,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    reason = "The AArch64 decoder maps instruction fields and aliases directly from ISA encodings; its casts and branches preserve that field-oriented layout."
)]

include!("aarch64.inc");
