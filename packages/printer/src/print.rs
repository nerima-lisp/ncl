//! The printing driver and its public entry points.

use std::collections::HashSet;

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, classify_object, make_string, string_length,
    string_ref, symbol_name,
};

use crate::circle::{CircleLabel, CircleState, labelable};
use crate::error::PrintError;
use crate::options::PrintOptions;
use crate::sink::{CharSink, StringSink};

/// The column at which the pretty printer starts a new line.
const DEFAULT_MARGIN: usize = 80;

/// The printer state for one [`write()`] call.
pub struct Printer<'a> {
    pub ctx: &'a mut ThreadContext,
    // The current printing path allocates nothing on the Lisp heap, so it never
    // needs the runtime; the field stays so array and dispatch printing can
    // reach it without an API change.
    #[allow(
        dead_code,
        reason = "reserved for the pprint-dispatch and stream layers"
    )]
    pub runtime: &'a Runtime,
    pub sink: &'a mut dyn CharSink,
    pub options: PrintOptions,
    pub depth: usize,
    pub column: usize,
    pub margin: usize,
    circle: Option<CircleState>,
    active: HashSet<usize>,
}

impl<'a> Printer<'a> {
    pub fn new(
        ctx: &'a mut ThreadContext,
        runtime: &'a Runtime,
        sink: &'a mut dyn CharSink,
        mut options: PrintOptions,
    ) -> Self {
        if options.readably {
            options.escape = true;
        }
        Self {
            ctx,
            runtime,
            sink,
            options,
            depth: 0,
            column: 0,
            margin: DEFAULT_MARGIN,
            circle: None,
            active: HashSet::new(),
        }
    }

    /// Enable `*print-circle*` labelling for this print operation.
    pub fn enable_circle(&mut self, object: Word) {
        self.circle = Some(CircleState::scan(
            &mut *self.ctx,
            object,
            self.options.circle_not_shared,
        ));
    }

    pub fn write_char(&mut self, character: char) -> Result<(), PrintError> {
        if character == '\n' {
            self.column = 0;
        } else {
            self.column += 1;
        }
        self.sink.write_char(character)
    }

    pub fn write_str(&mut self, text: &str) -> Result<(), PrintError> {
        for character in text.chars() {
            if character == '\n' {
                self.column = 0;
            } else {
                self.column += 1;
            }
        }
        self.sink.write_str(text)
    }

    /// Write `count` spaces.
    pub fn write_spaces(&mut self, count: usize) -> Result<(), PrintError> {
        for _ in 0..count {
            self.write_char(' ')?;
        }
        Ok(())
    }

    /// Write the separator between two sequence elements.
    ///
    /// When `*print-pretty*` is on and the current column has reached the
    /// margin, the separator becomes a newline and an indent to `indent`.
    pub fn separator(&mut self, indent: usize) -> Result<(), PrintError> {
        if self.options.pretty && self.column >= self.margin {
            self.write_char('\n')?;
            self.write_spaces(indent)
        } else {
            self.write_char(' ')
        }
    }

    /// Print one object, honoring `*print-level*` and `*print-circle*`.
    pub fn print(&mut self, object: Word) -> Result<(), PrintError> {
        if object == Word::NIL {
            return self.write_str("NIL");
        }
        if object == Word::TRUE {
            return self.write_str("T");
        }
        if let Some(level) = self.options.level
            && self.depth >= level
        {
            return self.write_char('#');
        }
        let labelled = labelable(self.ctx, object);
        if labelled {
            let address = object.address();
            let action = self
                .circle
                .as_mut()
                .and_then(|circle| circle.enter(address));
            match action {
                Some(CircleLabel::Definition(label)) => {
                    self.write_str(&format!("#{label}="))?;
                }
                Some(CircleLabel::Reference(label)) => {
                    self.write_str(&format!("#{label}#"))?;
                    return Ok(());
                }
                None => {
                    if self.circle.is_none() && !self.active.insert(address) {
                        return Err(PrintError::Circularity);
                    }
                }
            }
        }
        self.depth += 1;
        let result = self.print_inner(object);
        self.depth -= 1;
        if labelled && self.circle.is_none() {
            self.active.remove(&object.address());
        }
        result
    }

    /// Whether a list tail must be printed as a labelled or cyclic object
    /// rather than inlined into the current list walk.
    ///
    /// A tail that carries a `*print-circle*` label, or that is already being
    /// printed without `*print-circle*`, has to go through [`Self::print`] so
    /// the label or the cycle error is emitted.
    pub fn tail_is_shared(&self, object: Word) -> bool {
        let address = object.address();
        self.circle.as_ref().map_or_else(
            || self.active.contains(&address),
            |circle| circle.has_label(address),
        )
    }

    /// Read a symbol's name into Rust text.
    pub fn symbol_text(&self, symbol: Word) -> Result<String, PrintError> {
        let name = symbol_name(&*self.ctx, symbol)?;
        let length = string_length(&*self.ctx, name)?;
        let mut text = String::with_capacity(length);
        for index in 0..length {
            text.push(string_ref(&*self.ctx, name, index)?);
        }
        Ok(text)
    }

    /// Read a string object into Rust text.
    pub fn string_text(&self, string: Word) -> Result<String, PrintError> {
        let length = string_length(&*self.ctx, string)?;
        let mut text = String::with_capacity(length);
        for index in 0..length {
            text.push(string_ref(&*self.ctx, string, index)?);
        }
        Ok(text)
    }

    fn print_inner(&mut self, object: Word) -> Result<(), PrintError> {
        // `classify_object` reads a widetag from the first payload word, which
        // a headerless cons does not have, and `Word::character` encodes
        // `(scalar << 4) | 1`, whose lowtag reads as `List`. Both are detected
        // directly instead of through the widetag path.
        if let Some(code) = character_code(object) {
            return self.print_character(code);
        }
        if object.is_cons() {
            return self.print_cons(object);
        }
        match classify_object(self.ctx, object) {
            ObjectRef::Fixnum(value) => self.print_fixnum(value),
            ObjectRef::Character(character) => self.print_character(character),
            ObjectRef::Symbol(symbol) => self.print_symbol(symbol),
            ObjectRef::Cons(list) => self.print_cons(list),
            ObjectRef::String(string) => self.print_string(string),
            ObjectRef::SimpleVector(vector) => self.print_simple_vector(vector),
            ObjectRef::SpecializedArray(array) => self.print_specialized_array(array),
            ObjectRef::Array(array) => self.print_array(array),
            ObjectRef::Bignum(number) => self.print_bignum(number.into()),
            ObjectRef::Ratio(number) => self.print_ratio(number.into()),
            ObjectRef::DoubleFloat(number) => self.print_double(number.into()),
            ObjectRef::Complex(number) => self.print_complex(number.into()),
            ObjectRef::HashTable(word) => self.print_opaque("HASH-TABLE", word),
            ObjectRef::Structure(word) => self.print_opaque("STRUCTURE", word),
            ObjectRef::Instance(word) => self.print_opaque("INSTANCE", word),
            ObjectRef::Function(word) => self.print_opaque("FUNCTION", word),
            ObjectRef::Closure(word) => self.print_opaque("CLOSURE", word),
            ObjectRef::Package(word) => self.print_opaque("PACKAGE", word),
            ObjectRef::Readtable(word) => self.print_opaque("READTABLE", word),
            ObjectRef::Stream(word) => self.print_opaque("STREAM", word),
            ObjectRef::Code(word) => self.print_opaque("CODE", word),
            _ => self.print_opaque("OBJECT", object),
        }
    }
}

/// Decode an immediate character, or `None` for every other value.
///
/// `Word::character` encodes `(scalar << 4) | 1`, so `Word::lowtag()` reports
/// `List` for a character and `Word::is_character` never matches it. A cons
/// address is a heap pointer far above the Unicode scalar range, so the
/// scalar bound separates the two.
pub fn character_code(word: Word) -> Option<u32> {
    const SCALAR_LIMIT: u64 = 1 << 25;
    if word.lowtag() == 1 && word != Word::NIL && word.bits() < SCALAR_LIMIT {
        u32::try_from(word.bits() >> 4).ok()
    } else {
        None
    }
}

/// Render `object` into `sink` under `options`.
///
/// # Errors
///
/// Returns a [`PrintError`] when an object accessor fails, when the sink
/// rejects a write, when the object has no readable form while
/// `*print-readably*` is true, or when a cycle is found while `*print-circle*`
/// is false.
pub fn write(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    sink: &mut dyn CharSink,
    options: &PrintOptions,
) -> Result<(), PrintError> {
    let mut printer = Printer::new(ctx, runtime, sink, *options);
    if options.circle {
        printer.enable_circle(object);
    }
    printer.print(object)
}

/// Render `object` into a fresh Lisp string.
///
/// # Errors
///
/// Returns a [`PrintError`] when an object accessor fails, when the object has
/// no readable form while `*print-readably*` is true, when a cycle is found
/// while `*print-circle*` is false, or when the string allocation fails.
#[must_use = "the printed string is the result"]
pub fn write_to_string(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    options: &PrintOptions,
) -> Result<Word, PrintError> {
    let mut sink = StringSink::new();
    write(ctx, runtime, object, &mut sink, options)?;
    let characters: Vec<char> = sink.into_string().chars().collect();
    make_string(ctx, runtime, &characters).map_err(PrintError::from)
}
