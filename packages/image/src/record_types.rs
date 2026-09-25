use super::record_refs::Ref;

/// One serialized heap object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Record {
    /// A cons cell.
    Cons {
        /// Car reference.
        car: Ref,
        /// Cdr reference.
        cdr: Ref,
    },
    /// An interned symbol addressed by its home package and name.
    Symbol {
        /// Home package name; empty for an uninterned symbol.
        package: String,
        /// Symbol name.
        name: String,
        /// Raw symbol flag bits.
        flags: u32,
        /// Value cell.
        value: Ref,
        /// Function cell.
        function: Ref,
        /// Property list.
        plist: Ref,
    },
    /// A package addressed by name.
    Package {
        /// Primary package name.
        name: String,
        /// Package nicknames.
        nicknames: Vec<String>,
    },
    /// A string.
    String(String),
    /// A simple vector.
    Vector(Vec<Ref>),
    /// A specialized array.
    SpecializedArray {
        /// Element type tag.
        element_type: u8,
        /// Raw element words.
        elements: Vec<u64>,
    },
    /// A hash table addressed by its live entries.
    HashTable {
        /// Comparison mode.
        test: u8,
        /// Weakness mode.
        weakness: u8,
        /// Live key/value pairs.
        entries: Vec<(Ref, Ref)>,
    },
    /// A structure instance.
    Structure {
        /// Structure slots.
        slots: Vec<Ref>,
    },
    /// A CLOS instance.
    Instance {
        /// Class reference.
        class: Ref,
        /// Slot vector contents.
        slots: Vec<Ref>,
    },
    /// A simple function or a closure.
    Function {
        /// Whether the object is a closure with inline captures.
        closure: bool,
        /// Raw entry address.
        entry: u64,
        /// Function name.
        name: Ref,
        /// Lambda list.
        lambda_list: Ref,
        /// Code object reference.
        code: Ref,
        /// Inline captures; empty for a simple function.
        captures: Vec<Ref>,
    },
    /// A code object descriptor.
    CodeObject {
        /// Raw entry address.
        entry: u64,
        /// Code byte size.
        size: u64,
        /// Constant table.
        constants: Ref,
        /// Stack-map index.
        stack_map: Ref,
        /// Debug table.
        debug: Ref,
    },
    /// A bignum.
    Bignum {
        /// Whether the value is negative.
        negative: bool,
        /// Little-endian 32-bit limbs.
        limbs: Vec<u32>,
    },
    /// A ratio.
    Ratio {
        /// Numerator.
        numerator: Ref,
        /// Denominator.
        denominator: Ref,
    },
    /// A double float.
    DoubleFloat {
        /// Raw IEEE-754 binary64 bits.
        bits: u64,
    },
    /// A complex number.
    Complex {
        /// Real component.
        real: Ref,
        /// Imaginary component.
        imag: Ref,
    },
}
