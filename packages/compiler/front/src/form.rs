//! Reading `ncl-object` values as AST data.
//!
//! The reader produces `Word` values and the AST stores none, so these helpers
//! walk a form and convert it to [`Literal`] and [`SymbolRef`] values. They
//! never allocate, which keeps the `Word` identities they observe stable for
//! the duration of one conversion.

use std::collections::HashMap;

use ncl_object::{
    ArrayElementType, Bignum, Complex, DoubleFloat, Instance, ObjectRef, Package, Ratio, Runtime,
    ThreadContext, Word, array_dimensions, array_row_major_ref, bignum_limbs, bignum_sign, car,
    cdr, classify, classify_object, complex_imag, complex_real, double_value, instance_class,
    ratio_denominator, ratio_numerator, simple_vector_length, simple_vector_ref, slot_ref,
    specialized_array_element_type, string_length, string_ref, symbol_name, symbol_package,
};

use crate::error::FrontError;
use crate::literal::{Literal, NumberLiteral};
use crate::symbols::SymbolRef;

/// Assigns expansion-local identities to symbols and arrays.
///
/// An uninterned symbol has no home package, so two of them with the same name
/// are distinct objects. The table keys on the symbol's raw `Word` bits, which
/// are stable while a form is converted because the conversion allocates
/// nothing.
#[derive(Debug, Default)]
pub struct UninternedTable {
    identities: HashMap<Word, u32>,
    arrays: HashMap<Word, u32>, // check-added-lines: allow(word-table) Conversion does not allocate, so observed Words stay stable.
    specialized_arrays: HashMap<Word, u32>, // check-added-lines: allow(word-table) Conversion does not allocate, so observed Words stay stable.
    next: u32,
}

impl UninternedTable {
    /// Create an empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the identity of `symbol`, assigning one on first sight.
    pub fn identity(&mut self, symbol: Word) -> u32 {
        if let Some(identity) = self.identities.get(&symbol) {
            return *identity;
        }
        let identity = self.next;
        self.next = self.next.wrapping_add(1);
        self.identities.insert(symbol, identity);
        identity
    }

    /// Assign an expansion-local identity to a general array.
    pub fn array_identity(&mut self, array: Word) -> u32 {
        if let Some(identity) = self.arrays.get(&array) {
            return *identity;
        }
        let identity = self.next;
        self.next = self.next.wrapping_add(1);
        self.arrays.insert(array, identity);
        identity
    }

    /// Assign an expansion-local identity to a specialized array.
    pub fn specialized_array_identity(&mut self, array: Word) -> u32 {
        if let Some(identity) = self.specialized_arrays.get(&array) {
            return *identity;
        }
        let identity = self.next;
        self.next = self.next.wrapping_add(1);
        self.specialized_arrays.insert(array, identity);
        identity
    }
}

/// Classify a word by its lowtag first, falling back to the widetag.
///
/// [`ncl_object::classify_object`] reads a widetag for every word, including
/// conses and immediates, where no object header exists, so it reports a
/// garbage widetag for them. This helper consults the lowtag first and reads a
/// widetag only for a general heap pointer.
#[must_use]
pub fn classify_form(ctx: &ThreadContext, word: Word) -> ObjectRef {
    match classify(word) {
        ObjectRef::Other { .. } => classify_object(ctx, word),
        other => other,
    }
}

/// Read a string word as an owned Rust string.
///
/// # Errors
///
/// Returns [`FrontError::Object`] when the word is not a string.
pub fn word_string(ctx: &ThreadContext, word: Word) -> Result<String, FrontError> {
    let length = string_length(ctx, word)?;
    let mut characters = String::with_capacity(length);
    for index in 0..length {
        characters.push(string_ref(ctx, word, index)?);
    }
    Ok(characters)
}

/// Collect the elements of a proper list.
///
/// # Errors
///
/// Returns [`FrontError::ImproperList`] when the list is dotted, and
/// [`FrontError::Object`] when the object layer rejects a cell.
pub fn list(ctx: &mut ThreadContext, word: Word) -> Result<Vec<Word>, FrontError> {
    let mut elements = Vec::new();
    let mut cursor = word;
    while cursor != Word::NIL {
        if !cursor.is_cons() {
            return Err(FrontError::ImproperList);
        }
        elements.push(car(ctx, cursor)?);
        cursor = cdr(ctx, cursor)?;
    }
    Ok(elements)
}

/// Convert a symbol word into a [`SymbolRef`].
///
/// # Errors
///
/// Returns [`FrontError::Object`] when the word is not a symbol.
pub fn symbol_ref(
    ctx: &ThreadContext,
    table: &mut UninternedTable,
    word: Word,
) -> Result<SymbolRef, FrontError> {
    if word == Word::NIL {
        return Ok(SymbolRef::interned("COMMON-LISP", "NIL"));
    }
    let name = word_string(ctx, symbol_name(ctx, word)?)?;
    let package = symbol_package(ctx, word)?;
    if package == Word::NIL {
        let identity = table.identity(word);
        return Ok(SymbolRef::uninterned(name, identity));
    }
    let package = word_string(ctx, Package::from_word(package).name(ctx)?)?;
    Ok(SymbolRef::interned(package, name))
}

/// Convert a form word into a [`Literal`].
///
/// # Errors
///
/// Returns [`FrontError::UnsupportedLiteral`] for an object the frozen literal
/// set cannot represent, and [`FrontError::Object`] when the object layer
/// rejects a sub-object.
pub fn literal(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: &mut UninternedTable,
    word: Word,
) -> Result<Literal, FrontError> {
    if word == Word::NIL {
        return Ok(Literal::Nil);
    }
    match classify_form(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(Literal::fixnum(value)),
        ObjectRef::Character(value) => Ok(Literal::Character(value)),
        ObjectRef::Symbol(symbol) => symbol_literal(ctx, table, symbol),
        ObjectRef::Cons(_) => cons_literal(ctx, runtime, table, word),
        ObjectRef::String(_) => Ok(Literal::String(word_characters(ctx, word)?)),
        ObjectRef::SimpleVector(_) => vector_literal(ctx, runtime, table, word),
        ObjectRef::SpecializedArray(array) => specialized_array_literal(ctx, runtime, table, array),
        ObjectRef::Array(_) => array_literal(ctx, runtime, table, word),
        ObjectRef::Instance(_) => pathname_literal(ctx, runtime, word),
        ObjectRef::Bignum(_)
        | ObjectRef::Ratio(_)
        | ObjectRef::DoubleFloat(_)
        | ObjectRef::Complex(_) => Ok(Literal::Number(number_literal(ctx, word)?)),
        _ => Err(FrontError::UnsupportedLiteral),
    }
}

fn pathname_literal(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    word: Word,
) -> Result<Literal, FrontError> {
    let Some(class) = runtime.class(ctx, "PATHNAME") else {
        return Err(FrontError::UnsupportedLiteral);
    };
    let instance = Instance::from_word(word);
    if instance_class(ctx, instance)? != class {
        return Err(FrontError::UnsupportedLiteral);
    }
    let namestring = slot_ref(ctx, instance, 0)?;
    Ok(Literal::Pathname(word_characters(ctx, namestring)?))
}

/// Convert a number word into a [`NumberLiteral`].
///
/// # Errors
///
/// Returns [`FrontError::UnsupportedLiteral`] when the word is not a number
/// the literal set represents, and [`FrontError::Object`] for a malformed
/// number.
pub fn number_literal(ctx: &mut ThreadContext, word: Word) -> Result<NumberLiteral, FrontError> {
    match classify_form(ctx, word) {
        ObjectRef::Fixnum(value) => Ok(NumberLiteral::Fixnum(value)),
        ObjectRef::Bignum(_) => {
            let object = Bignum::from_word(word);
            Ok(NumberLiteral::Bignum {
                negative: bignum_sign(ctx, object)?,
                limbs: bignum_limbs(ctx, object)?,
            })
        }
        ObjectRef::Ratio(_) => {
            let object = Ratio::from_word(word);
            let numerator = ratio_numerator(ctx, object)?;
            let denominator = ratio_denominator(ctx, object)?;
            Ok(NumberLiteral::Ratio {
                numerator: Box::new(number_literal(ctx, numerator)?),
                denominator: Box::new(number_literal(ctx, denominator)?),
            })
        }
        ObjectRef::DoubleFloat(_) => Ok(NumberLiteral::DoubleFloat(double_value(
            ctx,
            DoubleFloat::from_word(word),
        )?)),
        ObjectRef::Complex(_) => {
            let object = Complex::from_word(word);
            let real = complex_real(ctx, object)?;
            let imaginary = complex_imag(ctx, object)?;
            Ok(NumberLiteral::Complex {
                real: Box::new(number_literal(ctx, real)?),
                imaginary: Box::new(number_literal(ctx, imaginary)?),
            })
        }
        _ => Err(FrontError::UnsupportedLiteral),
    }
}

/// Read every character of a string word.
fn word_characters(ctx: &ThreadContext, word: Word) -> Result<Vec<char>, FrontError> {
    let length = string_length(ctx, word)?;
    let mut characters = Vec::with_capacity(length);
    for index in 0..length {
        characters.push(string_ref(ctx, word, index)?);
    }
    Ok(characters)
}

/// Convert a symbol word, folding `nil` and `t` into their literals.
fn symbol_literal(
    ctx: &ThreadContext,
    table: &mut UninternedTable,
    symbol: Word,
) -> Result<Literal, FrontError> {
    let reference = symbol_ref(ctx, table, symbol)?;
    if reference.is_named("COMMON-LISP", "NIL") {
        return Ok(Literal::Nil);
    }
    if reference.is_named("COMMON-LISP", "T") {
        return Ok(Literal::T);
    }
    Ok(Literal::Symbol(reference))
}

/// Convert a cons word into a [`Literal::Cons`].
fn cons_literal(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: &mut UninternedTable,
    word: Word,
) -> Result<Literal, FrontError> {
    let head = car(ctx, word)?;
    let tail = cdr(ctx, word)?;
    let head = literal(ctx, runtime, table, head)?;
    let tail = literal(ctx, runtime, table, tail)?;
    Ok(Literal::Cons(Box::new(head), Box::new(tail)))
}

/// Convert a simple-vector word into a [`Literal::Vector`].
fn vector_literal(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: &mut UninternedTable,
    word: Word,
) -> Result<Literal, FrontError> {
    let length = simple_vector_length(ctx, word)?;
    let mut elements = Vec::with_capacity(length);
    for index in 0..length {
        elements.push(literal(
            ctx,
            runtime,
            table,
            simple_vector_ref(ctx, word, index)?,
        )?);
    }
    Ok(Literal::Vector(elements))
}

/// Convert a general-array word into a [`Literal::Array`].
fn array_literal(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: &mut UninternedTable,
    word: Word,
) -> Result<Literal, FrontError> {
    let dimensions = array_dimensions(ctx, word)?;
    let element_type = ncl_object::array::array_element_type(ctx, word)?;
    let identity = table.array_identity(word);
    let total = dimensions
        .iter()
        .try_fold(1_usize, |size, dimension| size.checked_mul(*dimension))
        .ok_or(FrontError::UnsupportedLiteral)?;
    let mut elements = Vec::with_capacity(total);
    for index in 0..total {
        elements.push(literal(
            ctx,
            runtime,
            table,
            array_row_major_ref(ctx, word, index)?,
        )?);
    }
    Ok(Literal::Array {
        identity,
        dimensions,
        element_type,
        elements,
    })
}

/// Convert a specialized-array word into a typed literal.
fn specialized_array_literal(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    table: &mut UninternedTable,
    word: Word,
) -> Result<Literal, FrontError> {
    let element_type = specialized_array_element_type(ctx, word)?;
    if element_type == ArrayElementType::T {
        return Err(FrontError::UnsupportedLiteral);
    }
    let dimensions = array_dimensions(ctx, word)?;
    if dimensions.len() != 1 {
        return Err(FrontError::UnsupportedLiteral);
    }
    let [length] = dimensions.as_slice() else {
        return Err(FrontError::UnsupportedLiteral);
    };
    let length = *length;
    let identity = table.specialized_array_identity(word);
    let mut elements = Vec::with_capacity(length);
    for index in 0..length {
        elements.push(literal(
            ctx,
            runtime,
            table,
            array_row_major_ref(ctx, word, index)?,
        )?);
    }
    Ok(Literal::SpecializedArray {
        identity,
        element_type,
        elements,
    })
}
