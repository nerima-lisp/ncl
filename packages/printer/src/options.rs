//! Print options mirroring the Common Lisp `*print-*` variables.

use ncl_object::{
    Package, Runtime, ThreadContext, Word, make_string, pop_root, push_root, symbol_is_special,
    symbol_name, symbol_value,
};

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
// `CLHS` names eight independent boolean print controls; collapsing them into a
// bitfield would hide the one-to-one mapping with the standard variables.
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors eight independent CL variables"
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct PrintOptions {
    /// `*print-escape*`: escape characters and strings.
    pub escape: bool,
    /// `*print-readably*`: only produce reader-visible output.
    pub readably: bool,
    /// `*print-base*`: integer radix, 2 through 36.
    pub base: u32,
    /// `*print-radix*`: print an explicit radix prefix.
    pub radix: bool,
    /// `*print-case*`: symbol-name case conversion.
    pub case: PrintCase,
    /// `*print-circle*`: detect and label shared structure.
    pub circle: bool,
    /// `*print-length*`: maximum list or vector length.
    pub length: Option<usize>,
    /// `*print-level*`: maximum nesting depth.
    pub level: Option<usize>,
    /// `*print-pretty*`: enable line breaks and indentation.
    pub pretty: bool,
    /// `*print-array*`: print array contents rather than an opaque form.
    pub array: bool,
    /// `*print-gensym*`: print `#:` before uninterned symbol names.
    pub gensym: bool,
    /// `SB-EXT:*PRINT-CIRCLE-NOT-SHARED*`: label only genuinely shared objects.
    pub circle_not_shared: bool,
    /// `SB-EXT:*PRINT-VECTOR-LENGTH*`: maximum length for arrays.
    pub vector_length: Option<usize>,
}

impl PrintOptions {
    /// The standard defaults: `*print-escape*` and `*print-array*` on,
    /// base ten, no length or level limit.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            escape: true,
            readably: false,
            base: 10,
            radix: false,
            case: PrintCase::Upcase,
            circle: false,
            length: None,
            level: None,
            pretty: false,
            array: true,
            gensym: true,
            circle_not_shared: false,
            vector_length: None,
        }
    }

    /// Return a copy with `*print-escape*` replaced.
    #[must_use]
    pub const fn with_escape(mut self, escape: bool) -> Self {
        self.escape = escape;
        self
    }

    /// Return a copy with `*print-readably*` replaced.
    #[must_use]
    pub const fn with_readably(mut self, readably: bool) -> Self {
        self.readably = readably;
        self
    }

    /// Return a copy with `*print-base*` replaced.
    #[must_use]
    pub const fn with_base(mut self, base: u32) -> Self {
        self.base = base;
        self
    }

    /// Return a copy with `*print-radix*` replaced.
    #[must_use]
    pub const fn with_radix(mut self, radix: bool) -> Self {
        self.radix = radix;
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
        self.circle = circle;
        self
    }

    /// Return a copy with `*print-length*` replaced.
    #[must_use]
    pub const fn with_length(mut self, length: Option<usize>) -> Self {
        self.length = length;
        self
    }

    /// Return a copy with `*print-level*` replaced.
    #[must_use]
    pub const fn with_level(mut self, level: Option<usize>) -> Self {
        self.level = level;
        self
    }

    /// Return a copy with `*print-pretty*` replaced.
    #[must_use]
    pub const fn with_pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }

    /// Return a copy with `*print-array*` replaced.
    #[must_use]
    pub const fn with_array(mut self, array: bool) -> Self {
        self.array = array;
        self
    }

    /// Return a copy with `*print-gensym*` replaced.
    #[must_use]
    pub const fn with_gensym(mut self, gensym: bool) -> Self {
        self.gensym = gensym;
        self
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
        options.escape = bool_special(ctx, runtime, "*PRINT-ESCAPE*", options.escape);
        options.readably = bool_special(ctx, runtime, "*PRINT-READABLY*", options.readably);
        options.radix = bool_special(ctx, runtime, "*PRINT-RADIX*", options.radix);
        options.circle = bool_special(ctx, runtime, "*PRINT-CIRCLE*", options.circle);
        options.pretty = bool_special(ctx, runtime, "*PRINT-PRETTY*", options.pretty);
        options.array = bool_special(ctx, runtime, "*PRINT-ARRAY*", options.array);
        options.gensym = bool_special(ctx, runtime, "*PRINT-GENSYM*", options.gensym);
        options.base = base_special(ctx, runtime, options.base);
        options.case = case_special(ctx, runtime, options.case);
        options.length = length_special(ctx, runtime, "*PRINT-LENGTH*");
        options.level = length_special(ctx, runtime, "*PRINT-LEVEL*");
        options.circle_not_shared = bool_special(
            ctx,
            runtime,
            "SB-EXT:*PRINT-CIRCLE-NOT-SHARED*",
            options.circle_not_shared,
        );
        options.vector_length = length_special(ctx, runtime, "SB-EXT:*PRINT-VECTOR-LENGTH*");
        options
    }
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Find an interned special variable, or `None` when it is absent.
///
/// A name without a `PACKAGE::` prefix is looked up in `COMMON-LISP`.
fn find_special(ctx: &mut ThreadContext, runtime: &Runtime, qualified: &str) -> Option<Word> {
    let (package_name, symbol_name) = qualified
        .split_once("::")
        .unwrap_or(("COMMON-LISP", qualified));
    let package = runtime.find_package(ctx, package_name)?;
    let mut name = make_string(ctx, runtime, &symbol_name.chars().collect::<Vec<char>>()).ok()?;
    let token = push_root(ctx, &mut name);
    let found = Package::from(package)
        .find_symbol(ctx, name)
        .ok()
        .flatten()
        .map(|(symbol, _status)| symbol);
    let _ = pop_root(ctx, token);
    let symbol = found?;
    if symbol_is_special(ctx, symbol).unwrap_or(false) {
        Some(symbol)
    } else {
        None
    }
}

/// Read a special's value cell, or `None` when it is absent or unbound.
fn special_value(ctx: &mut ThreadContext, runtime: &Runtime, qualified: &str) -> Option<Word> {
    let symbol = find_special(ctx, runtime, qualified)?;
    match symbol_value(ctx, symbol) {
        Ok(Word::UNBOUND) | Err(_) => None,
        Ok(value) => Some(value),
    }
}

/// Read a boolean special, keeping `fallback` when it is absent.
fn bool_special(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    qualified: &str,
    fallback: bool,
) -> bool {
    special_value(ctx, runtime, qualified).map_or(fallback, |value| value != Word::NIL)
}

/// Read a length or level special; `NIL` means no limit.
fn length_special(ctx: &mut ThreadContext, runtime: &Runtime, qualified: &str) -> Option<usize> {
    special_value(ctx, runtime, qualified)
        .and_then(Word::as_fixnum)
        .and_then(|value| usize::try_from(value).ok())
}

/// Read `*print-base*`, clamped to the range `CLHS` allows.
fn base_special(ctx: &mut ThreadContext, runtime: &Runtime, fallback: u32) -> u32 {
    special_value(ctx, runtime, "*PRINT-BASE*")
        .and_then(Word::as_fixnum)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|base| (2..=36).contains(base))
        .unwrap_or(fallback)
}

/// Read `*print-case*` from the name of its value symbol.
fn case_special(ctx: &mut ThreadContext, runtime: &Runtime, fallback: PrintCase) -> PrintCase {
    let Some(value) = special_value(ctx, runtime, "*PRINT-CASE*") else {
        return fallback;
    };
    let Ok(name) = symbol_name(ctx, value) else {
        return fallback;
    };
    let Ok(length) = ncl_object::string_length(ctx, name) else {
        return fallback;
    };
    let mut text = String::with_capacity(length);
    for index in 0..length {
        match ncl_object::string_ref(ctx, name, index) {
            Ok(character) => text.push(character),
            Err(_) => return fallback,
        }
    }
    match text.to_uppercase().as_str() {
        "UPCASE" => PrintCase::Upcase,
        "DOWNCASE" => PrintCase::Downcase,
        "CAPITALIZE" => PrintCase::Capitalize,
        _ => fallback,
    }
}
