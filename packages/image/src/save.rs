//! Capture of a reachable object graph into image records.

use std::collections::{HashMap, VecDeque};

use ncl_object::hash_table::{HashTable, HashTest, Weakness};
use ncl_object::{
    Bignum, CodeObject, Complex, DoubleFloat, Function, Instance, ObjectError, ObjectRef, Package,
    Ratio, Runtime, ThreadContext, Word, bignum_limbs, bignum_sign, car, cdr, classify_object,
    closure_ref, code_constants, code_debug, code_entry, code_size, code_stack_map, complex_imag,
    complex_real, double_value, function_code, function_entry, function_lambda_list, function_name,
    instance_class, instance_offset, ratio_denominator, ratio_numerator, simple_vector_length,
    simple_vector_ref, slot_ref, specialized_array_element_type, specialized_array_offset,
    specialized_array_ref, string_length, string_ref, structure_layout, structure_ref,
    symbol_flags, symbol_function, symbol_name, symbol_package, symbol_plist, symbol_value,
};
use ncl_sys::LowTag;

use crate::code::CodeImage;
use crate::domain::{Architecture, Features};
use crate::error::ImageError;
use crate::format::ImageFile;
use crate::record::{Record, Ref};

/// `PACKAGE` payload slot holding the nickname list (`packages/object/src/package.rs`).
const PACKAGE_NICKNAMES: usize = 3;

/// Save a rooted object graph, its symbol table, and optional code blobs.
///
/// The image records each reachable object once and stores references as record
/// indices. Symbols are addressed by package and name so that loading re-interns
/// them into the destination runtime rather than duplicating them.
///
/// # Errors
///
/// Returns [`ImageError::UnsupportedKind`] when the graph reaches an object the
/// image format cannot serialize, and an object or code error when a heap or
/// code-space read fails.
pub fn save(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    roots: &[Word],
    code: &[CodeImage],
) -> Result<Vec<u8>, ImageError> {
    let mut capture = Capture::new(runtime);
    let root_refs = capture.run(ctx, roots)?;
    let objects = capture.finish();
    let file = ImageFile {
        architecture: Architecture::host(),
        gc_epoch: ncl_sys::heap_epoch(ctx.thread_mut()),
        objects,
        roots: root_refs,
        code: code.to_vec(),
        features: Features::new(runtime.features())?,
    };
    file.to_bytes()
}

/// Return the architecture byte for the current target.
/// Object-graph walker that assigns record indices and captures payloads.
struct Capture<'a> {
    runtime: &'a Runtime,
    records: Vec<Option<Record>>,
    seen: HashMap<usize, u32>,
    queue: VecDeque<(u32, Word)>,
}

impl<'a> Capture<'a> {
    /// Create an empty capture for `runtime`.
    fn new(runtime: &'a Runtime) -> Self {
        Self {
            runtime,
            records: Vec::new(),
            seen: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    /// Capture every root and drain the pending-object queue.
    fn run(&mut self, ctx: &mut ThreadContext, roots: &[Word]) -> Result<Vec<Ref>, ImageError> {
        let mut root_refs = Vec::with_capacity(roots.len());
        for &root in roots {
            root_refs.push(self.ref_of(root)?);
        }
        while let Some((id, word)) = self.queue.pop_front() {
            let record = self.capture(ctx, word)?;
            let index = usize::try_from(id).map_err(|_| invalid("object index"))?;
            let slot = self
                .records
                .get_mut(index)
                .ok_or_else(|| invalid("object index"))?;
            *slot = Some(record);
        }
        Ok(root_refs)
    }

    /// Return all captured records in index order.
    fn finish(self) -> Vec<Record> {
        self.records.into_iter().flatten().collect()
    }

    /// Map a word to an immediate reference or a fresh object reference.
    fn ref_of(&mut self, word: Word) -> Result<Ref, ImageError> {
        if !is_heap(word) {
            return Ok(Ref::Immediate(word.bits()));
        }
        let key = usize::try_from(word.bits()).map_err(|_| invalid("object address"))?;
        if let Some(&id) = self.seen.get(&key) {
            return Ok(Ref::Object(id));
        }
        let id = u32::try_from(self.records.len()).map_err(|_| invalid("object count"))?;
        self.seen.insert(key, id);
        self.records.push(None);
        self.queue.push_back((id, word));
        Ok(Ref::Object(id))
    }

    /// Capture one heap object's payload.
    fn capture(&mut self, ctx: &mut ThreadContext, word: Word) -> Result<Record, ImageError> {
        if word.lowtag() == low_tag(LowTag::List) {
            let car = car(ctx, word)?;
            let cdr = cdr(ctx, word)?;
            return Ok(Record::Cons {
                car: self.ref_of(car)?,
                cdr: self.ref_of(cdr)?,
            });
        }
        match classify_object(ctx, word) {
            ObjectRef::Cons(word) => {
                let car = car(ctx, word)?;
                let cdr = cdr(ctx, word)?;
                Ok(Record::Cons {
                    car: self.ref_of(car)?,
                    cdr: self.ref_of(cdr)?,
                })
            }
            ObjectRef::Symbol(word) => self.capture_symbol(ctx, word),
            ObjectRef::Package(word) => Self::capture_package(ctx, word),
            ObjectRef::String(word) => Ok(Record::String(read_string(ctx, word)?)),
            ObjectRef::SimpleVector(word) => {
                let length = simple_vector_length(ctx, word)?;
                let mut elements = Vec::with_capacity(length.min(1024));
                for index in 0..length {
                    let element = simple_vector_ref(ctx, word, index)?;
                    elements.push(self.ref_of(element)?);
                }
                Ok(Record::Vector(elements))
            }
            ObjectRef::SpecializedArray(word) => Self::capture_specialized_array(ctx, word),
            ObjectRef::HashTable(word) => self.capture_hash_table(ctx, word),
            ObjectRef::Structure(word) => self.capture_structure(ctx, word),
            ObjectRef::Instance(word) => self.capture_instance(ctx, word),
            ObjectRef::Function(word) => self.capture_function(ctx, word, false),
            ObjectRef::Closure(word) => self.capture_function(ctx, word, true),
            ObjectRef::Code(word) => self.capture_code_object(ctx, word),
            ObjectRef::Bignum(word) => Self::capture_bignum(ctx, word),
            ObjectRef::Ratio(word) => {
                let numerator = ratio_numerator(ctx, Ratio::from(word))?;
                let denominator = ratio_denominator(ctx, Ratio::from(word))?;
                Ok(Record::Ratio {
                    numerator: self.ref_of(numerator)?,
                    denominator: self.ref_of(denominator)?,
                })
            }
            ObjectRef::DoubleFloat(word) => Ok(Record::DoubleFloat {
                bits: double_value(ctx, DoubleFloat::from(word))?.to_bits(),
            }),
            ObjectRef::Complex(word) => {
                let real = complex_real(ctx, Complex::from(word))?;
                let imag = complex_imag(ctx, Complex::from(word))?;
                Ok(Record::Complex {
                    real: self.ref_of(real)?,
                    imag: self.ref_of(imag)?,
                })
            }
            ObjectRef::Array(_) => Err(unsupported("non-simple array")),
            ObjectRef::Readtable(_) => Err(unsupported("readtable")),
            ObjectRef::Stream(_) => Err(unsupported("stream")),
            ObjectRef::Fixnum(_)
            | ObjectRef::Character(_)
            | ObjectRef::Other { .. }
            | ObjectRef::Immediate(_) => Err(unsupported("non-heap value")),
        }
    }

    fn capture_symbol(&mut self, ctx: &ThreadContext, word: Word) -> Result<Record, ImageError> {
        let package_word = symbol_package(ctx, word)?;
        let package = if package_word == Word::NIL {
            String::new()
        } else {
            read_string(ctx, Package::from(package_word).name(ctx)?)?
        };
        let name = read_string(ctx, symbol_name(ctx, word)?)?;
        let value = symbol_value(ctx, word)?;
        let function = symbol_function(ctx, word)?;
        let plist = symbol_plist(ctx, word)?;
        Ok(Record::Symbol {
            package,
            name,
            flags: symbol_flags(ctx, word)?,
            value: self.ref_of(value)?,
            function: self.ref_of(function)?,
            plist: self.ref_of(plist)?,
        })
    }

    fn capture_package(ctx: &mut ThreadContext, word: Word) -> Result<Record, ImageError> {
        let name = read_string(ctx, Package::from(word).name(ctx)?)?;
        let mut list = read_slot(ctx, word, PACKAGE_NICKNAMES)?;
        let mut nicknames = Vec::new();
        while list != Word::NIL {
            let item = car(ctx, list)?;
            nicknames.push(read_string(ctx, item)?);
            list = cdr(ctx, list)?;
        }
        Ok(Record::Package { name, nicknames })
    }

    fn capture_specialized_array(
        ctx: &mut ThreadContext,
        word: Word,
    ) -> Result<Record, ImageError> {
        let element_type = match specialized_array_element_type(ctx, word)? {
            ncl_object::ArrayElementType::T => 0,
            ncl_object::ArrayElementType::Bit => 1,
            ncl_object::ArrayElementType::Character => 2,
            ncl_object::ArrayElementType::BaseChar => 3,
            ncl_object::ArrayElementType::Fixnum => 4,
            ncl_object::ArrayElementType::Signed => 5,
            ncl_object::ArrayElementType::Unsigned => 6,
            ncl_object::ArrayElementType::SingleFloat => 7,
            ncl_object::ArrayElementType::DoubleFloat => 8,
        };
        let length = read_slot(ctx, word, specialized_array_offset::LENGTH)?
            .as_fixnum()
            .ok_or(ImageError::Object(ObjectError::Layout))?;
        let length = usize::try_from(length).map_err(|_| invalid("array length"))?;
        let mut elements = Vec::with_capacity(length.min(1024));
        for index in 0..length {
            elements.push(specialized_array_ref(ctx, word, index)?.bits());
        }
        Ok(Record::SpecializedArray {
            element_type,
            elements,
        })
    }

    fn capture_hash_table(
        &mut self,
        ctx: &ThreadContext,
        word: Word,
    ) -> Result<Record, ImageError> {
        let table = HashTable::from(word);
        let test = hash_test_tag(table.test(ctx)?);
        let weakness = weakness_tag(table.weakness(ctx)?);
        let mut raw = Vec::new();
        table.for_each_entry(ctx, |key, value| raw.push((key, value)))?;
        let mut entries = Vec::with_capacity(raw.len());
        for (key, value) in raw {
            entries.push((self.ref_of(key)?, self.ref_of(value)?));
        }
        Ok(Record::HashTable {
            test,
            weakness,
            entries,
        })
    }

    fn capture_structure(&mut self, ctx: &ThreadContext, word: Word) -> Result<Record, ImageError> {
        let layout = structure_layout(ctx, word)?;
        let count = self
            .runtime
            .structure_layout_size(layout)
            .ok_or(ImageError::Object(ObjectError::Layout))?;
        let mut slots = Vec::with_capacity(count.min(1024));
        for index in 0..count {
            let slot = structure_ref(ctx, word, index)?;
            slots.push(self.ref_of(slot)?);
        }
        Ok(Record::Structure { slots })
    }

    fn capture_instance(
        &mut self,
        ctx: &mut ThreadContext,
        word: Word,
    ) -> Result<Record, ImageError> {
        let class = instance_class(ctx, Instance::from(word))?;
        let class = self.ref_of(class)?;
        let slot_vector = read_slot(ctx, word, instance_offset::SLOT_VECTOR)?;
        let count = simple_vector_length(ctx, slot_vector)?;
        let mut slots = Vec::with_capacity(count.min(1024));
        for index in 0..count {
            let slot = slot_ref(ctx, Instance::from(word), index)?;
            slots.push(self.ref_of(slot)?);
        }
        Ok(Record::Instance { class, slots })
    }

    fn capture_function(
        &mut self,
        ctx: &ThreadContext,
        word: Word,
        closure: bool,
    ) -> Result<Record, ImageError> {
        let function = Function::from(word);
        let entry = function_entry(ctx, function)?;
        let name = function_name(ctx, function)?;
        let lambda_list = function_lambda_list(ctx, function)?;
        let code = function_code(ctx, function)?;
        let mut captures = Vec::new();
        if closure {
            let mut index = 0;
            // `closure_ref` rejects an out-of-range index, which bounds the inline captures.
            while let Ok(value) = closure_ref(ctx, function, index) {
                captures.push(self.ref_of(value)?);
                index += 1;
            }
        }
        Ok(Record::Function {
            closure,
            entry: u64::try_from(entry).map_err(|_| invalid("entry address"))?,
            name: self.ref_of(name)?,
            lambda_list: self.ref_of(lambda_list)?,
            code: self.ref_of(code.as_word())?,
            captures,
        })
    }

    fn capture_code_object(
        &mut self,
        ctx: &ThreadContext,
        word: Word,
    ) -> Result<Record, ImageError> {
        let code = CodeObject::from(word);
        let entry = code_entry(ctx, code)?
            .as_fixnum()
            .ok_or(ImageError::Object(ObjectError::Layout))?;
        let size = code_size(ctx, code)?
            .as_fixnum()
            .ok_or(ImageError::Object(ObjectError::Layout))?;
        let constants = code_constants(ctx, code)?;
        let stack_map = code_stack_map(ctx, code)?;
        let debug = code_debug(ctx, code)?;
        Ok(Record::CodeObject {
            entry: u64::try_from(entry).map_err(|_| invalid("code entry"))?,
            size: u64::try_from(size).map_err(|_| invalid("code size"))?,
            constants: self.ref_of(constants)?,
            stack_map: self.ref_of(stack_map)?,
            debug: self.ref_of(debug)?,
        })
    }

    fn capture_bignum(ctx: &ThreadContext, word: Word) -> Result<Record, ImageError> {
        let bignum = Bignum::from(word);
        Ok(Record::Bignum {
            negative: bignum_sign(ctx, bignum)?,
            limbs: bignum_limbs(ctx, bignum)?,
        })
    }
}

/// Report whether a word is a heap object the image can address by record.
fn is_heap(word: Word) -> bool {
    let tag = word.lowtag();
    if tag == low_tag(LowTag::List) {
        return word != Word::NIL;
    }
    tag == low_tag(LowTag::Instance) || tag == low_tag(LowTag::OtherPointer)
}

const fn low_tag(tag: LowTag) -> u8 {
    match tag {
        LowTag::Character => 0,
        LowTag::List => 1,
        LowTag::SingleFloat => 2,
        LowTag::Function => 3,
        LowTag::OtherImmediate => 4,
        LowTag::Instance => 5,
        LowTag::Reserved => 6,
        LowTag::OtherPointer => 7,
    }
}

const fn hash_test_tag(test: HashTest) -> u8 {
    match test {
        HashTest::Eq => 0,
        HashTest::Eql => 1,
        HashTest::Equal => 2,
        HashTest::Equalp => 3,
    }
}

const fn weakness_tag(weakness: Weakness) -> u8 {
    match weakness {
        Weakness::None => 0,
        Weakness::Key => 1,
        Weakness::Value => 2,
        Weakness::KeyAndValue => 3,
        Weakness::KeyOrValue => 4,
    }
}

/// Read a string object into a Rust string.
fn read_string(ctx: &ThreadContext, word: Word) -> Result<String, ImageError> {
    let length = string_length(ctx, word)?;
    let mut out = String::with_capacity(length);
    for index in 0..length {
        out.push(string_ref(ctx, word, index)?);
    }
    Ok(out)
}

/// Read one raw payload word of a header object.
fn read_slot(ctx: &mut ThreadContext, object: Word, slot: usize) -> Result<Word, ImageError> {
    ncl_sys::read_object_word(ctx.thread_mut(), object, slot)
        .ok_or(ImageError::Object(ObjectError::Layout))
}

const fn invalid(field: &'static str) -> ImageError {
    ImageError::InvalidLayout { field }
}

const fn unsupported(kind: &'static str) -> ImageError {
    ImageError::UnsupportedKind { kind }
}
