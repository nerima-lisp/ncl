use crate::{CLOSED, DATA, FILE_OUTPUT, POSITION, STRING_INPUT, STRING_OUTPUT};
use ncl_object::*;

mod adapters;
mod input;
mod output;
mod state;

pub(crate) use adapters::*;
pub(crate) use input::*;
pub(crate) use output::*;
pub(crate) use state::*;
