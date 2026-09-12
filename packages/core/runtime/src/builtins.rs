use crate::environment::normalize_name;
use crate::evaluator::quoted_form_value;
use crate::package::{self, COMMON_LISP_PACKAGE, KEYWORD_PACKAGE};
use crate::{Environment, Rational, RuntimeError, Value};

#[cfg(test)]
mod file_tests;

mod builtin_integer;
pub(crate) use builtin_integer::parse_integer;

mod builtin_characters;
pub(crate) use builtin_characters::{
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
pub(crate) use builtin_arrays::{
    adjust_array, adjustable_array_p, aref, array_dimension, array_dimensions, array_displacement,
    array_element_type, array_has_fill_pointer_p, array_in_bounds_p, array_rank,
    array_row_major_index, array_total_size, arrayp, bit, fill_pointer, make_array, row_major_aref,
    simple_array_p, svref, vector, vector_pop, vector_push, vector_push_extend,
};

mod builtin_helpers;
use builtin_helpers::{arity, exact, type_error};

mod builtin_reading;
pub(crate) use builtin_reading::{read, read_from_string, read_preserving_whitespace};
pub(crate) use builtin_reading::{read_from_string_in, read_in, read_preserving_whitespace_in};

mod builtin_hash_tables;
pub use builtin_hash_tables::hash_table_key_equal;
pub(crate) use builtin_hash_tables::set_hash_table_entry;
#[allow(clippy::wildcard_imports)]
pub(crate) use builtin_hash_tables::*;
pub(crate) use builtin_hash_tables::{
    hash_table_rehash_size_value as hash_table_rehash_size,
    hash_table_rehash_threshold_value as hash_table_rehash_threshold,
    hash_table_size_value as hash_table_size,
};

mod builtin_array_helpers;
#[allow(clippy::wildcard_imports)]
pub(crate) use builtin_array_helpers::*;

mod builtin_stream_predicates;
pub(crate) use builtin_stream_predicates::{
    close_stream, input_stream_p, open_stream_p, output_stream_p, stream_element_type,
    stream_external_format, streamp,
};

mod builtin_stream_bytes;
pub mod standard_streams;

mod builtin_random;
pub(crate) use builtin_random::with_random_state_context;
pub(crate) use builtin_random::{
    bind_dynamic_random_state, dynamic_random_state_depth, make_random_state, random,
    random_state_p, set_dynamic_random_state, truncate_dynamic_random_states,
};

mod builtin_time;
pub(crate) use builtin_time::sleep;

mod builtin_format_data;
use builtin_format_data::{ENGLISH_NUMBER_GROUPS, FORMAT_DIGITS};

mod registry;
pub use registry::install;
mod builtin_numeric_ops;
#[allow(clippy::wildcard_imports)]
pub use builtin_numeric_ops::*;
pub(crate) use builtin_numeric_ops::{imagpart as imaginary_part, realpart as real_part};
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
pub(crate) use type_predicates::*;

mod builtin_printer;
pub(crate) use builtin_printer::{
    complement, constantly, identity, prin1, princ, print_value, printed_value, type_of,
    write_to_string, write_value,
};

mod builtin_stream_constructors;
pub(crate) use builtin_stream_constructors::{
    make_string_input_stream, make_string_output_stream, stream_bound,
};

mod builtin_stream_context;
pub(crate) use builtin_stream_context::{standard_input, standard_output, with_stream_context};

mod builtin_stream_position;
pub(crate) use builtin_stream_position::{file_length, file_position};

mod builtin_file_helpers;
pub(crate) use builtin_file_helpers::{pathname_argument, stream_keyword_name};

mod builtin_file_metadata;
pub(crate) use builtin_file_metadata::{
    delete_file, file_write_date, probe_file, rename_file, truename,
};

mod builtin_file_open_modes;
pub(crate) use builtin_file_open_modes::{
    open_binary_input_file, open_binary_io_file, open_binary_output_file, open_input_file,
    open_io_file, open_output_file,
};

mod builtin_file_open;
pub(crate) use builtin_file_open::open_file;

mod builtin_stream_helpers;
pub(crate) use builtin_stream_helpers::{
    end_of_file_error, input_stream_reference, peek_character, stream_reference, stream_state_error,
};

mod builtin_stream_reading;
pub(crate) use builtin_stream_reading::{
    clear_input, get_output_stream_string, listen, peek_char, read_byte, read_char,
    read_char_no_hang, read_line, read_sequence, unread_char,
};

mod builtin_stream_writing;
pub(crate) use builtin_stream_writing::{
    clear_output, finish_output, force_output, fresh_line, terpri, write_byte, write_char,
    write_destination, write_line, write_sequence, write_string,
};

mod format;
pub use format::format_control;
pub(crate) use format::format_value;
mod numbers;
#[allow(clippy::wildcard_imports)]
pub(crate) use numbers::*;
