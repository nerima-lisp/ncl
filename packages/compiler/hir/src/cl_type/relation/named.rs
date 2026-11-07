use super::super::compound::BuiltinType;
use super::SubtypeRelation;
pub(super) fn named_relation(sub: BuiltinType, sup: BuiltinType) -> SubtypeRelation {
    if sub == sup || is_supertype(sup, sub) {
        SubtypeRelation::Yes
    } else if is_supertype(sub, sup) || are_disjoint(sub, sup) {
        SubtypeRelation::No
    } else {
        SubtypeRelation::Unknown
    }
}
const fn is_supertype(sup: BuiltinType, sub: BuiltinType) -> bool {
    is_numeric_supertype(sup, sub)
        || is_sequence_supertype(sup, sub)
        || is_other_supertype(sup, sub)
}
#[allow(clippy::unnested_or_patterns)]
const fn is_numeric_supertype(sup: BuiltinType, sub: BuiltinType) -> bool {
    use BuiltinType::{
        Bignum, Complex, DoubleFloat, Fixnum, Float, Integer, LongFloat, Number, Ratio, Rational,
        Real, ShortFloat, SingleFloat,
    };
    matches!(
        (sup, sub),
        (
            Number,
            Real | Rational
                | Integer
                | Fixnum
                | Bignum
                | Ratio
                | Float
                | ShortFloat
                | SingleFloat
                | DoubleFloat
                | LongFloat
                | Complex
        ) | (
            Real,
            Rational
                | Integer
                | Fixnum
                | Bignum
                | Ratio
                | Float
                | ShortFloat
                | SingleFloat
                | DoubleFloat
                | LongFloat
        ) | (Rational, Integer | Fixnum | Bignum | Ratio)
            | (Integer, Fixnum | Bignum)
            | (Float, ShortFloat | SingleFloat | DoubleFloat | LongFloat)
    )
}
#[allow(clippy::unnested_or_patterns)]
const fn is_sequence_supertype(sup: BuiltinType, sub: BuiltinType) -> bool {
    use BuiltinType::{
        Array, BaseString, BitVector, Cons, List, Null, Sequence, SimpleArray, SimpleBaseString,
        SimpleBitVector, SimpleString, SimpleVector, String, Vector,
    };
    matches!(
        (sup, sub),
        (
            Sequence,
            List | Cons
                | Null
                | Vector
                | String
                | BaseString
                | SimpleString
                | SimpleBaseString
                | BitVector
                | SimpleBitVector
                | SimpleVector
        ) | (
            Array,
            Vector
                | String
                | BaseString
                | SimpleString
                | SimpleBaseString
                | BitVector
                | SimpleBitVector
                | SimpleVector
                | SimpleArray
        ) | (
            Vector,
            String
                | BaseString
                | SimpleString
                | SimpleBaseString
                | BitVector
                | SimpleBitVector
                | SimpleVector
        ) | (List, Cons | Null)
            | (String, BaseString | SimpleString | SimpleBaseString)
            | (BaseString | SimpleString, SimpleBaseString)
            | (BitVector, SimpleBitVector)
            | (
                SimpleArray,
                SimpleBitVector | SimpleVector | SimpleString | SimpleBaseString
            )
    )
}
#[allow(clippy::unnested_or_patterns)]
const fn is_other_supertype(sup: BuiltinType, sub: BuiltinType) -> bool {
    use BuiltinType::{
        Array, Atom, BaseChar, BaseString, Bignum, BitVector, Boolean, Character, CompiledFunction,
        Complex, DoubleFloat, ExtendedChar, Fixnum, Float, Function, HashTable, Integer, Keyword,
        LongFloat, Null, Number, Package, RandomState, Ratio, Rational, Real, Restart, ShortFloat,
        SimpleArray, SimpleBaseString, SimpleBitVector, SimpleString, SimpleVector, SingleFloat,
        StandardChar, Stream, String, Structure, Symbol, Vector,
    };
    matches!(
        (sup, sub),
        (Function, CompiledFunction)
            | (Symbol, Null | Keyword | Boolean)
            | (Boolean, Null)
            | (Character, BaseChar | StandardChar | ExtendedChar)
            | (BaseChar, StandardChar)
            | (
                Atom,
                Null | Boolean
                    | Symbol
                    | Keyword
                    | Package
                    | HashTable
                    | RandomState
                    | Restart
                    | Function
                    | CompiledFunction
                    | Number
                    | Real
                    | Rational
                    | Integer
                    | Fixnum
                    | Bignum
                    | Ratio
                    | Float
                    | ShortFloat
                    | SingleFloat
                    | DoubleFloat
                    | LongFloat
                    | Complex
                    | Character
                    | BaseChar
                    | StandardChar
                    | ExtendedChar
                    | String
                    | BaseString
                    | SimpleString
                    | SimpleBaseString
                    | BitVector
                    | SimpleBitVector
                    | Vector
                    | SimpleVector
                    | Array
                    | SimpleArray
                    | Stream
                    | Structure
            )
    )
}
pub(super) fn are_disjoint(left: BuiltinType, right: BuiltinType) -> bool {
    known_disjoint(left, right) || known_disjoint(right, left)
}
#[allow(clippy::unnested_or_patterns)]
fn known_disjoint(left: BuiltinType, right: BuiltinType) -> bool {
    use BuiltinType::{
        Array, Atom, BaseChar, BaseString, BitVector, Boolean, Character, CompiledFunction, Cons,
        ExtendedChar, Function, HashTable, Keyword, List, Null, Package, RandomState, Restart,
        Sequence, SimpleArray, SimpleBaseString, SimpleBitVector, SimpleString, SimpleVector,
        StandardChar, Stream, String, Structure, Symbol, Vector,
    };
    if is_numeric_type(left) && is_numeric_type(right) {
        return numeric_types_disjoint(left, right);
    }
    if is_numeric_type(left) {
        return matches!(
            right,
            Boolean
                | Character
                | BaseChar
                | StandardChar
                | ExtendedChar
                | Symbol
                | Keyword
                | Null
                | String
                | BaseString
                | SimpleString
                | SimpleBaseString
                | Cons
                | List
                | Sequence
                | Function
                | CompiledFunction
                | Array
                | SimpleArray
                | Vector
                | BitVector
                | SimpleBitVector
                | SimpleVector
                | Package
                | Stream
                | Restart
                | HashTable
                | RandomState
                | Structure
        );
    }
    if is_numeric_type(right) {
        return known_disjoint(right, left);
    }
    matches!(
        (left, right),
        (Atom, Cons)
            | (
                Cons,
                Atom | Symbol
                    | Null
                    | String
                    | Character
                    | Function
                    | Array
                    | Package
                    | Stream
                    | Restart
            )
            | (
                Symbol,
                Cons | String | Character | Function | Array | Package | Stream | Restart
            )
            | (
                String,
                Symbol | Character | Function | Package | Stream | Restart
            )
            | (
                Character,
                Symbol | String | Function | Array | Sequence | Package | Stream | Restart
            )
            | (
                Function,
                Symbol | String | Character | Array | Sequence | Package | Stream | Restart | List
            )
            | (
                List,
                String | Character | Function | Array | Package | Stream | Restart
            )
            | (Sequence, Character | Function | Package | Stream | Restart)
            | (
                Array,
                Character | Function | Package | Stream | Restart | List
            )
            | (
                Package,
                String | Character | Function | Array | Sequence | Cons
            )
            | (
                Stream,
                String | Character | Function | Array | Sequence | Cons
            )
            | (
                Restart,
                String | Character | Function | Array | Sequence | Cons
            )
    )
}
const fn numeric_types_disjoint(left: BuiltinType, right: BuiltinType) -> bool {
    known_numeric_disjoint(left, right) || known_numeric_disjoint(right, left)
}
const fn known_numeric_disjoint(left: BuiltinType, right: BuiltinType) -> bool {
    use BuiltinType::{
        Bignum, Complex, DoubleFloat, Fixnum, Float, Integer, LongFloat, Ratio, Rational, Real,
        ShortFloat, SingleFloat,
    };
    matches!(
        (left, right),
        (
            Integer | Rational,
            Float | ShortFloat | SingleFloat | DoubleFloat | LongFloat
        ) | (
            Float | ShortFloat | SingleFloat | DoubleFloat | LongFloat | Complex,
            Integer | Rational
        ) | (Integer | Rational | Real, Complex)
            | (Complex, Real)
            | (Fixnum | Bignum, Ratio)
            | (Ratio, Fixnum | Bignum)
    )
}
const fn is_numeric_type(name: BuiltinType) -> bool {
    matches!(
        name,
        BuiltinType::Number
            | BuiltinType::Real
            | BuiltinType::Rational
            | BuiltinType::Integer
            | BuiltinType::Fixnum
            | BuiltinType::Bignum
            | BuiltinType::Ratio
            | BuiltinType::Float
            | BuiltinType::ShortFloat
            | BuiltinType::SingleFloat
            | BuiltinType::DoubleFloat
            | BuiltinType::LongFloat
            | BuiltinType::Complex
    )
}
