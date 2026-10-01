#![allow(dead_code)]

use ncl_asm_x86_64::Reg;

pub const THREAD_CONTEXT: Reg = Reg::R15;
pub const SCRATCH: [Reg; 2] = [Reg::R10, Reg::R11];
