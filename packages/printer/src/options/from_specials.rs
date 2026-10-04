use ncl_object::{Runtime, ThreadContext};

use super::specials::{base_special, bool_special, case_special, length_special, usize_special};
use super::{ArrayMode, CircleMode, CircleSharingMode, EscapeMode, GensymMode, NonNegative};
use super::{PrettyMode, RadixMode, ReadabilityMode};

/// Read the ambient `*print-*` specials, falling back to [`Self::new`].
///
/// Most `*print-*` variables belong to `ncl-lib-streams`; a variable that
/// is absent, unbound, or holds an unexpected type leaves the default in
/// place instead of failing. This reads the symbol's value cell, not a
/// dynamic binding, because `ThreadContext` exposes no binding lookup yet.
#[must_use]
pub(super) fn from_specials(ctx: &mut ThreadContext, runtime: &Runtime) -> super::PrintOptions {
    let mut options = super::PrintOptions::new();
    options.escape = EscapeMode::from_bool(bool_special(
        ctx,
        runtime,
        "*PRINT-ESCAPE*",
        options.escape(),
    ));
    options.readably = ReadabilityMode::from_bool(bool_special(
        ctx,
        runtime,
        "*PRINT-READABLY*",
        options.readably(),
    ));
    options.radix =
        RadixMode::from_bool(bool_special(ctx, runtime, "*PRINT-RADIX*", options.radix()));
    options.circle = CircleMode::from_bool(bool_special(
        ctx,
        runtime,
        "*PRINT-CIRCLE*",
        options.circle(),
    ));
    options.pretty = PrettyMode::from_bool(bool_special(
        ctx,
        runtime,
        "*PRINT-PRETTY*",
        options.pretty(),
    ));
    options.array =
        ArrayMode::from_bool(bool_special(ctx, runtime, "*PRINT-ARRAY*", options.array()));
    options.gensym = GensymMode::from_bool(bool_special(
        ctx,
        runtime,
        "*PRINT-GENSYM*",
        options.gensym(),
    ));
    options.base = base_special(ctx, runtime, options.base);
    options.case = case_special(ctx, runtime, options.case);
    options.length = length_special(ctx, runtime, "*PRINT-LENGTH*");
    options.level = length_special(ctx, runtime, "*PRINT-LEVEL*");
    options.circle_not_shared = if bool_special(
        ctx,
        runtime,
        "NCL-EXT:*PRINT-CIRCLE-NOT-SHARED*",
        matches!(options.circle_not_shared, CircleSharingMode::OnlyShared),
    ) {
        CircleSharingMode::OnlyShared
    } else {
        CircleSharingMode::AllOccurrences
    };
    options.vector_length = length_special(ctx, runtime, "NCL-EXT:*PRINT-VECTOR-LENGTH*");
    options.right_margin = NonNegative::new(
        usize_special(ctx, runtime, "*PRINT-RIGHT-MARGIN*")
            .map_or_else(|| options.right_margin.get(), |value| value.max(1)),
    );
    options.miser_width = NonNegative::new(
        usize_special(ctx, runtime, "*PRINT-MISER-WIDTH*")
            .unwrap_or_else(|| options.miser_width.get()), // check-added-lines: allow(panic) eager default is a stored scalar
    );
    options.print_lines = usize_special(ctx, runtime, "*PRINT-LINES*").map(NonNegative::new);
    options
}
