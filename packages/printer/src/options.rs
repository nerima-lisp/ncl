//! Print options mirroring the Common Lisp `*print-*` variables.

mod specials;

use ncl_object::{Runtime, ThreadContext};

use self::specials::{base_special, bool_special, case_special, length_special};

/// A validated radix accepted by the printer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrintBase(u32);

impl PrintBase {
    /// The smallest supported radix.
    pub const MIN: u32 = 2;
    /// The largest supported radix.
    pub const MAX: u32 = 36;

    /// Construct a radix in the range 2 through 36.
    ///
    /// # Errors
    /// Returns `None` when `base` is outside the supported range.
    #[must_use]
    pub const fn new(base: u32) -> Option<Self> {
        if base >= Self::MIN && base <= Self::MAX {
            Some(Self(base))
        } else {
            None
        }
    }

    /// Return the radix as an integer.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A non-negative printer limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonNegative(usize);

impl NonNegative {
    /// Construct a non-negative value from its representation.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Return the represented value.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

macro_rules! mode {
    ($name:ident, $on:ident, $off:ident) => {
        #[doc = "A semantic mode for the corresponding Common Lisp print option."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[non_exhaustive]
        pub enum $name {
            /// Enable the option.
            $on,
            /// Disable the option.
            $off,
        }

        #[allow(dead_code)]
        impl $name {
            const fn as_bool(self) -> bool {
                matches!(self, Self::$on)
            }

            const fn from_bool(value: bool) -> Self {
                if value { Self::$on } else { Self::$off }
            }
        }
    };
}

mode!(EscapeMode, Escaped, Raw);
mode!(ReadabilityMode, Readable, Unreadable);
mode!(RadixMode, WithRadix, WithoutRadix);
mode!(CircleMode, Circle, NoCircle);
mode!(PrettyMode, Pretty, Plain);
mode!(ArrayMode, Contents, Opaque);
mode!(GensymMode, WithPrefix, WithoutPrefix);
mode!(CircleSharingMode, OnlyShared, AllOccurrences);

/// How `*print-case*` renders symbol names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrintCase {
    /// Uppercase letters, the `:upcase` default.
    Upcase,
    /// Lowercase letters, the `:downcase` value.
    Downcase,
    /// Capitalize each word, the `:capitalize` value.
    Capitalize,
}

/// The options controlling one print operation.
///
/// The fields mirror the `*print-*` variables one for one, so the printer can
/// be driven from a binding environment without a second options type. `CLHS`
/// 22.1.1 defines each variable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct PrintOptions {
    /// `*print-escape*`: escape characters and strings.
    escape: EscapeMode,
    /// `*print-readably*`: only produce reader-visible output.
    readably: ReadabilityMode,
    /// `*print-base*`: integer radix, 2 through 36.
    base: PrintBase,
    /// `*print-radix*`: print an explicit radix prefix.
    radix: RadixMode,
    /// `*print-case*`: symbol-name case conversion.
    pub(crate) case: PrintCase,
    /// `*print-circle*`: detect and label shared structure.
    circle: CircleMode,
    /// `*print-length*`: maximum list or vector length.
    length: Option<NonNegative>,
    /// `*print-level*`: maximum nesting depth.
    level: Option<NonNegative>,
    /// `*print-pretty*`: enable line breaks and indentation.
    pretty: PrettyMode,
    /// `*print-array*`: print array contents rather than an opaque form.
    array: ArrayMode,
    /// `*print-gensym*`: print `#:` before uninterned symbol names.
    gensym: GensymMode,
    /// `NCL-EXT:*PRINT-CIRCLE-NOT-SHARED*`: label only genuinely shared objects.
    circle_not_shared: CircleSharingMode,
    /// `NCL-EXT:*PRINT-VECTOR-LENGTH*`: maximum length for arrays.
    vector_length: Option<NonNegative>,
}

impl PrintOptions {
    /// The standard defaults: `*print-escape*` and `*print-array*` on,
    /// base ten, no length or level limit.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            escape: EscapeMode::Escaped,
            readably: ReadabilityMode::Unreadable,
            base: PrintBase(10),
            radix: RadixMode::WithoutRadix,
            case: PrintCase::Upcase,
            circle: CircleMode::NoCircle,
            length: None,
            level: None,
            pretty: PrettyMode::Plain,
            array: ArrayMode::Contents,
            gensym: GensymMode::WithPrefix,
            circle_not_shared: CircleSharingMode::AllOccurrences,
            vector_length: None,
        }
    }

    /// Return a copy with `*print-escape*` replaced.
    #[must_use]
    pub const fn with_escape(mut self, escape: bool) -> Self {
        self.escape = EscapeMode::from_bool(escape);
        self
    }

    /// Return a copy with `*print-readably*` replaced.
    #[must_use]
    pub const fn with_readably(mut self, readably: bool) -> Self {
        self.readably = ReadabilityMode::from_bool(readably);
        self
    }

    /// Return a copy with `*print-base*` replaced.
    #[must_use]
    pub const fn with_base(self, base: u32) -> Self {
        match PrintBase::new(base) {
            Some(base) => self.with_print_base(base),
            None => self,
        }
    }

    /// Return a copy with a validated print base.
    #[must_use]
    pub const fn with_print_base(mut self, base: PrintBase) -> Self {
        self.base = base;
        self
    }

    /// Return a copy with `*print-radix*` replaced.
    #[must_use]
    pub const fn with_radix(mut self, radix: bool) -> Self {
        self.radix = RadixMode::from_bool(radix);
        self
    }

    /// Return a copy with `*print-case*` replaced.
    #[must_use]
    pub const fn with_case(mut self, case: PrintCase) -> Self {
        self.case = case;
        self
    }

    /// Return a copy with `*print-circle*` replaced.
    #[must_use]
    pub const fn with_circle(mut self, circle: bool) -> Self {
        self.circle = CircleMode::from_bool(circle);
        self
    }

    /// Return a copy with `*print-length*` replaced.
    #[must_use]
    pub const fn with_length(mut self, length: Option<usize>) -> Self {
        self.length = match length {
            Some(value) => Some(NonNegative::new(value)),
            None => None,
        };
        self
    }

    /// Return a copy with `*print-level*` replaced.
    #[must_use]
    pub const fn with_level(mut self, level: Option<usize>) -> Self {
        self.level = match level {
            Some(value) => Some(NonNegative::new(value)),
            None => None,
        };
        self
    }

    /// Return a copy with `*print-pretty*` replaced.
    #[must_use]
    pub const fn with_pretty(mut self, pretty: bool) -> Self {
        self.pretty = PrettyMode::from_bool(pretty);
        self
    }

    /// Return a copy with `*print-array*` replaced.
    #[must_use]
    pub const fn with_array(mut self, array: bool) -> Self {
        self.array = ArrayMode::from_bool(array);
        self
    }

    /// Return a copy with `*print-gensym*` replaced.
    #[must_use]
    pub const fn with_gensym(mut self, gensym: bool) -> Self {
        self.gensym = GensymMode::from_bool(gensym);
        self
    }

    /// Return a copy with `NCL-EXT:*PRINT-CIRCLE-NOT-SHARED*` replaced.
    #[must_use]
    pub const fn with_circle_not_shared(mut self, not_shared: bool) -> Self {
        self.circle_not_shared = if not_shared {
            CircleSharingMode::OnlyShared
        } else {
            CircleSharingMode::AllOccurrences
        };
        self
    }

    /// Return a copy with `NCL-EXT:*PRINT-VECTOR-LENGTH*` replaced.
    #[must_use]
    pub const fn with_vector_length(mut self, length: Option<usize>) -> Self {
        self.vector_length = match length {
            Some(value) => Some(NonNegative::new(value)),
            None => None,
        };
        self
    }

    /// Return whether character and string escaping is enabled.
    #[must_use]
    pub const fn escape(self) -> bool {
        self.escape.as_bool()
    }
    /// Return whether readable output is requested.
    #[must_use]
    pub const fn readably(self) -> bool {
        self.readably.as_bool()
    }
    /// Return the print base.
    #[must_use]
    pub const fn print_base(self) -> PrintBase {
        self.base()
    }
    /// Return whether radix prefixes are enabled.
    #[must_use]
    pub const fn radix(self) -> bool {
        self.radix.as_bool()
    }
    /// Return the symbol case.
    #[must_use]
    pub const fn case(self) -> PrintCase {
        self.case
    }
    /// Return whether circular structures are labelled.
    #[must_use]
    pub const fn circle(self) -> bool {
        self.circle.as_bool()
    }
    /// Return the list/vector length limit.
    #[must_use]
    pub const fn length(self) -> Option<NonNegative> {
        self.length
    }
    /// Return the nesting level limit.
    #[must_use]
    pub const fn level(self) -> Option<NonNegative> {
        self.level
    }
    /// Return whether pretty printing is enabled.
    #[must_use]
    pub const fn pretty(self) -> bool {
        self.pretty.as_bool()
    }
    /// Return whether array contents are printed.
    #[must_use]
    pub const fn array(self) -> bool {
        self.array.as_bool()
    }
    /// Return whether uninterned symbol prefixes are printed.
    #[must_use]
    pub const fn gensym(self) -> bool {
        self.gensym.as_bool()
    }
    /// Return the vector length limit.
    #[must_use]
    pub const fn vector_length(self) -> Option<NonNegative> {
        self.vector_length
    }

    /// Return the current escape mode.
    #[must_use]
    pub const fn escape_mode(self) -> EscapeMode {
        self.escape
    }

    /// Return the current readability mode.
    #[must_use]
    pub const fn readability_mode(self) -> ReadabilityMode {
        self.readably
    }

    /// Return the validated print base.
    #[must_use]
    pub const fn base(self) -> PrintBase {
        // `new` is guaranteed by `new` and `from_specials`; this fallback is
        // only for preserving the invariant if an internal caller changes.
        self.base
    }

    /// Return the current radix mode.
    #[must_use]
    pub const fn radix_mode(self) -> RadixMode {
        self.radix
    }

    /// Return the current circle mode.
    #[must_use]
    pub const fn circle_mode(self) -> CircleMode {
        self.circle
    }

    /// Return the current pretty-printing mode.
    #[must_use]
    pub const fn pretty_mode(self) -> PrettyMode {
        self.pretty
    }

    /// Return the current array mode.
    #[must_use]
    pub const fn array_mode(self) -> ArrayMode {
        self.array
    }

    /// Return the current gensym mode.
    #[must_use]
    pub const fn gensym_mode(self) -> GensymMode {
        self.gensym
    }

    /// Return whether circle labels are restricted to shared objects.
    #[must_use]
    pub const fn circle_sharing_mode(self) -> CircleSharingMode {
        self.circle_not_shared
    }

    /// Set the escape mode.
    #[must_use]
    pub const fn with_escape_mode(self, mode: EscapeMode) -> Self {
        self.with_escape(mode.as_bool())
    }

    /// Set the readability mode.
    #[must_use]
    pub const fn with_readability_mode(self, mode: ReadabilityMode) -> Self {
        self.with_readably(mode.as_bool())
    }

    /// Set the print base after validating it.
    ///
    /// # Errors
    /// Returns `None` when `base` is outside 2 through 36.
    #[must_use]
    pub const fn try_with_base(mut self, base: u32) -> Option<Self> {
        let Some(base) = PrintBase::new(base) else {
            return None;
        };
        self.base = base;
        Some(self)
    }

    /// Read the ambient `*print-*` specials, falling back to [`Self::new`].
    ///
    /// Most `*print-*` variables belong to `ncl-lib-streams`; a variable that
    /// is absent, unbound, or holds an unexpected type leaves the default in
    /// place instead of failing. This reads the symbol's value cell, not a
    /// dynamic binding, because `ThreadContext` exposes no binding lookup yet.
    #[must_use]
    pub fn from_specials(ctx: &mut ThreadContext, runtime: &Runtime) -> Self {
        let mut options = Self::new();
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
        options
    }
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self::new()
    }
}
