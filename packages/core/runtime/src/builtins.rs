use crate::environment::normalize_name;
use crate::evaluator::quoted_form_value;
use crate::package::{self, COMMON_LISP_PACKAGE, KEYWORD_PACKAGE};
use crate::{Environment, Rational, RuntimeError, Value};

#[cfg(test)]
mod file_tests;

mod builtin_integer;
pub(crate) use builtin_integer::parse_integer;

mod builtin_characters;
pub use builtin_characters::{
    alpha_character_p, alphanumeric_p, both_case_p, char_code, char_int, character,
    character_case_equal, character_case_greater_equal, character_case_greater_than,
    character_case_less_equal, character_case_less_than, character_case_not_equal,
    character_downcase, character_equal, character_greater_equal, character_greater_than,
    character_less_equal, character_less_than, character_name, character_not_equal,
    character_upcase, character_value, code_char, digit_character, digit_character_p,
    graphic_character_p, int_char, lower_case_p, make_string, name_character, simple_character,
    standard_character_p, string_value, upper_case_p,
};

mod builtin_arrays;
pub use builtin_arrays::{
    aref, adjustable_array_p, array_dimension, array_dimensions, array_displacement, array_element_type, array_has_fill_pointer_p, array_in_bounds_p, array_rank,
    adjust_array, array_row_major_index, array_total_size, bit, fill_pointer, make_array, row_major_aref,
    svref, vector, vector_pop, vector_push, vector_push_extend,
};
pub use builtin_arrays::{arrayp, simple_array_p};

mod builtin_helpers;
use builtin_helpers::{arity, exact, type_error};

mod builtin_reading;
pub(crate) use builtin_reading::{read, read_preserving_whitespace};
pub(crate) use builtin_reading::read_from_string;

mod builtin_hash_tables;
pub use builtin_hash_tables::{
    clrhash, gethash, hash_table_count, hash_table_key_equal, hash_table_keys, hash_table_p,
    hash_table_size, hash_table_test_value, hash_table_values, make_hash_table, remhash,
};

mod builtin_array_helpers;
#[allow(clippy::wildcard_imports)]
use builtin_array_helpers::*;

mod builtin_stream_predicates;
pub use builtin_stream_predicates::{
    input_stream_p, open_stream_p, output_stream_p, stream_element_type, stream_external_format,
    streamp,
};
pub(crate) use builtin_stream_predicates::{file_length, file_position};
pub(crate) use builtin_stream_writing::{clear_output, finish_output, force_output};
pub(crate) use builtin_stream_predicates::close_stream;

mod builtin_random;
pub(crate) use builtin_random::{
    bind_dynamic_random_state, default_random_state_value, dynamic_random_state_depth,
    make_random_state, random, random_state_p, set_dynamic_random_state,
    truncate_dynamic_random_states,
};

mod builtin_format_data;
use builtin_format_data::{ENGLISH_NUMBER_GROUPS, FORMAT_DIGITS};

mod registry;
pub use registry::install;
mod builtin_numeric_ops;
#[allow(clippy::wildcard_imports)]
pub use builtin_numeric_ops::*;
mod builtin_sequences;
#[allow(clippy::wildcard_imports)]
pub use builtin_sequences::*;
mod builtin_list_ops;
#[allow(clippy::wildcard_imports)]
pub use builtin_list_ops::*;
mod types;
#[allow(clippy::wildcard_imports)]
pub use types::*;

#[cfg(test)]
mod builtins_tests;

pub mod type_predicates;
pub use type_predicates::eql_value;
#[allow(clippy::wildcard_imports)]
pub use type_predicates::*;

mod builtin_printer;
use builtin_printer::{complement, constantly, printed_value};
pub(crate) use builtin_printer::{prin1, princ, print_value, write_value};
pub(crate) use builtin_printer::write_to_string;
pub(crate) use builtin_printer::{identity, type_of};

mod builtin_stream_constructors;
pub(crate) use builtin_stream_constructors::{make_string_input_stream, make_string_output_stream};
use builtin_stream_constructors::stream_bound;

mod builtin_file_helpers;
use builtin_file_helpers::{pathname_argument, stream_keyword_name};

mod builtin_file_metadata;
pub(crate) use builtin_file_metadata::{delete_file, file_write_date, probe_file, rename_file, truename};

mod builtin_file_open_modes;
use builtin_file_open_modes::{open_input_file, open_io_file, open_output_file};

mod builtin_file_open;
pub(crate) use builtin_file_open::open_file;

mod builtin_stream_helpers;
use builtin_stream_helpers::{
    end_of_file_error, input_stream_reference, peek_character, stream_reference, stream_state_error,
};

mod builtin_stream_bytes;
pub(crate) use builtin_stream_bytes::{read_byte, write_byte};

mod builtin_stream_reading;
pub(crate) use builtin_stream_reading::{clear_input, listen, peek_char, read_char, read_char_no_hang, read_line, read_sequence, unread_char};
pub(crate) use builtin_stream_reading::get_output_stream_string;

mod builtin_stream_writing;
pub(crate) mod standard_streams;
use builtin_stream_writing::write_destination;
pub(crate) use builtin_stream_writing::{write_line, write_sequence, write_string};
pub(crate) use builtin_stream_writing::{fresh_line, terpri, write_char};

mod format;
pub use format::format_control;
use format::format_value;
mod numbers;
#[allow(clippy::wildcard_imports)]
use numbers::*;
