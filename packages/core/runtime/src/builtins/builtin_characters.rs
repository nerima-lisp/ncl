use super::{
    RuntimeError, Value, arity, character_argument, character_designator, exact, index_argument,
    integer_argument, out_of_bounds, string_designator, type_error,
};

mod access;
pub use access::{character, make_string, simple_character, string_value};

mod case;
pub use case::{both_case_p, character_downcase, character_upcase, lower_case_p, upper_case_p};

mod comparison;
pub use comparison::{
    character_equal, character_greater_equal, character_greater_than, character_less_equal,
    character_less_than, character_not_equal,
};

mod comparison_case_insensitive;
pub use comparison_case_insensitive::{
    character_case_equal, character_case_greater_equal, character_case_greater_than,
    character_case_less_equal, character_case_less_than, character_case_not_equal,
};

mod conversion;
pub use conversion::{char_code, char_int, character_value, code_char, int_char};

mod digits;
pub use digits::{digit_character, digit_character_p};

mod names;
pub use names::{character_name, name_character};

mod predicates;
pub use predicates::{
    alpha_character_p, alphanumeric_p, graphic_character_p, standard_character_p,
};
