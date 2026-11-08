#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::many_single_char_names,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::too_many_lines,
    clippy::unused_self,
    reason = "The x86-64 decoder maps ModRM, SIB, and immediate fields directly from instruction bytes; its names, casts, and shared arms preserve that representation."
)]

include!("x86_64.inc");
