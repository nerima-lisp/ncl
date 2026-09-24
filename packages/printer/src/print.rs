//! The printing driver and its public entry points.

use ncl_object::{
    ObjectRef, Runtime, ThreadContext, Word, classify_object, make_string, string_length,
    string_ref, symbol_name,
};

use crate::error::PrintError;
use crate::options::PrintOptions;
use crate::sink::{CharSink, StringSink};

/// The printer state for one [`write`] call.
pub struct Printer<'a> {
    pub ctx: &'a mut ThreadContext,
    // Reserved for array rank lookup and `pprint-dispatch` table access.
    #[allow(dead_code, reason = "consumed by array and pprint-dispatch printing")]
    pub runtime: &'a Runtime,
    pub sink: &'a mut dyn CharSink,
    pub options: PrintOptions,
    pub depth: usize,
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
        }
    }

    pub fn write_char(&mut self, character: char) -> Result<(), PrintError> {
        self.sink.write_char(character)
    }

    pub fn write_str(&mut self, text: &str) -> Result<(), PrintError> {
        self.sink.write_str(text)
    }

    /// Print one object, honoring `*print-level*`.
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
        self.depth += 1;
        let result = self.print_inner(object);
        self.depth -= 1;
        result
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
        match classify_object(self.ctx, object) {
            ObjectRef::Fixnum(value) => self.print_fixnum(value),
            ObjectRef::Character(character) => self.print_character(character),
            ObjectRef::Symbol(symbol) => self.print_symbol(symbol),
            ObjectRef::Cons(list) => self.print_cons(list),
            ObjectRef::String(string) => self.print_string(string),
            ObjectRef::SimpleVector(vector) => self.print_simple_vector(vector),
            // TODO(printer): print specialized and non-simple arrays by rank
            // once `ncl-object` exposes their length and rank accessors.
            ObjectRef::SpecializedArray(array) | ObjectRef::Array(array) => {
                self.print_opaque("ARRAY", array)
            }
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

/// Render `object` into `sink` under `options`.
///
/// # Errors
///
/// Returns a [`PrintError`] when an object accessor fails, when the sink
/// rejects a write, or when the object has no readable form while
/// `*print-readably*` is true.
pub fn write(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
    sink: &mut dyn CharSink,
    options: &PrintOptions,
) -> Result<(), PrintError> {
    let mut printer = Printer::new(ctx, runtime, sink, *options);
    printer.print(object)
}

/// Render `object` into a fresh Lisp string.
///
/// # Errors
///
/// Returns a [`PrintError`] when an object accessor fails, when the object has
/// no readable form while `*print-readably*` is true, or when the string
/// allocation fails.
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
