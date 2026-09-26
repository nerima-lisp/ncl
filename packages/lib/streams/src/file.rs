use crate::{CLOSED, DATA, POSITION};
use crate::character::{ensure_open, position, set_position, state_kind, stream_from_args, StreamKind};
use ncl_object::*;
use std::fs;
use std::io::{IsTerminal, Read, Seek, SeekFrom, Write};

mod adapters;
mod core;
mod io;

pub(crate) use adapters::*;
pub(crate) use core::*;
pub(crate) use io::*;
