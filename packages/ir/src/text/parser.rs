//! Text serialization parser.
use super::ParseError;
use crate::{
    BasicBlock, BlockId, BlockParam, Compare, Constant, ConstantIndex, Convert, DebugLocation,
    DebugLocationId, FileId, FormId, Function, FunctionId, HandlerKind, HandlerRegion,
    HandlerRegionId, Local, LocalId, Op, OpKind, Param, Prim, Terminator, Ty, ValueId,
};

fn u32(value: u64) -> Result<u32, ParseError> {
    u32::try_from(value).map_err(|_| ParseError("integer out of range".into()))
}

fn u8(value: u64) -> Result<u8, ParseError> {
    u8::try_from(value).map_err(|_| ParseError("integer out of range".into()))
}

fn unesc(s: &str) -> Result<String, ParseError> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'_' {
            out.push(bytes[i]);
            i += 1;
        } else if i + 2 >= bytes.len() {
            return Err(ParseError("bad escape".into()));
        } else {
            let part = std::str::from_utf8(&bytes[i + 1..i + 3])
                .map_err(|_| ParseError("bad escape".into()))?;
            out.push(u8::from_str_radix(part, 16).map_err(|_| ParseError("bad escape".into()))?);
            i += 3;
        }
    }
    String::from_utf8(out).map_err(|_| ParseError("invalid utf8".into()))
}
/// Parses a function produced by its `Display` implementation.
///
/// # Errors
///
/// Returns a [`ParseError`] when the input is malformed or contains values
/// outside the representable IR ranges.
pub fn parse(input: &str) -> Result<Function, ParseError> {
    let payload = input
        .strip_prefix("fn @")
        .and_then(|s| s.split_once(" { "))
        .map(|(_, rest)| rest.strip_suffix(" }\n").unwrap_or(rest))
        .ok_or_else(|| ParseError("expected fn header".into()))?;
    let mut r = Reader::new(payload);
    let function = decode_function(&mut r)?;
    if !r.done() {
        return Err(ParseError("trailing input".into()));
    }
    Ok(function)
}
struct Reader<'a> {
    fields: Vec<&'a str>,
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            fields: s.split(',').filter(|x| !x.is_empty()).collect(),
            at: 0,
        }
    }
    fn next(&mut self) -> Result<&'a str, ParseError> {
        let value = self
            .fields
            .get(self.at)
            .copied()
            .ok_or_else(|| ParseError("unexpected end".into()))?;
        self.at += 1;
        Ok(value)
    }
    fn u(&mut self) -> Result<u64, ParseError> {
        u64::from_str_radix(self.next()?, 16).map_err(|_| ParseError("bad integer".into()))
    }
    fn i(&mut self) -> Result<i64, ParseError> {
        let s = self.next()?;
        i64::from_str_radix(
            s.strip_prefix('i')
                .ok_or_else(|| ParseError("bad signed integer".into()))?,
            16,
        )
        .map_err(|_| ParseError("bad integer".into()))
    }
    fn s(&mut self) -> Result<String, ParseError> {
        unesc(self.next()?)
    }
    fn b(&mut self) -> Result<bool, ParseError> {
        Ok(self.u()? != 0)
    }
    const fn done(&self) -> bool {
        self.at == self.fields.len()
    }
}

fn read_ty(r: &mut Reader<'_>) -> Result<Ty, ParseError> {
    Ok(match r.u()? {
        0 => Ty::Word,
        1 => Ty::I64,
        2 => Ty::F64,
        3 => Ty::Address,
        4 => Ty::Bool,
        5 => Ty::Unit,
        _ => return Err(ParseError("bad type".into())),
    })
}

fn decode_function(r: &mut Reader<'_>) -> Result<Function, ParseError> {
    let id = FunctionId(u32(r.u()?)?);
    let name = r.s()?;
    let params = (0..r.u()?)
        .map(|_| {
            Ok(Param {
                name: r.s()?,
                ty: read_ty(r)?,
            })
        })
        .collect::<Result<Vec<_>, ParseError>>()?;
    let return_types = (0..r.u()?)
        .map(|_| read_ty(r))
        .collect::<Result<Vec<_>, _>>()?;
    let constants = (0..r.u()?)
        .map(|_| constant_read(r))
        .collect::<Result<Vec<_>, _>>()?;
    let blocks = (0..r.u()?)
        .map(|_| block_read(r))
        .collect::<Result<Vec<_>, _>>()?;
    let locals = (0..r.u()?)
        .map(|_| {
            Ok(Local {
                id: LocalId(u32(r.u()?)?),
                name: r.s()?,
                ty: read_ty(r)?,
            })
        })
        .collect::<Result<Vec<_>, ParseError>>()?;
    let handler_regions = (0..r.u()?)
        .map(|_| handler_read(r))
        .collect::<Result<Vec<_>, _>>()?;
    let debug = (0..r.u()?)
        .map(|_| {
            Ok(DebugLocation {
                file: FileId(u32(r.u()?)?),
                line: u32(r.u()?)?,
                column: u32(r.u()?)?,
                form: FormId(u32(r.u()?)?),
            })
        })
        .collect::<Result<Vec<_>, ParseError>>()?;
    Ok(Function {
        id,
        name,
        params,
        return_types,
        blocks,
        locals,
        constants,
        handler_regions,
        debug,
    })
}

fn constant_read(r: &mut Reader<'_>) -> Result<Constant, ParseError> {
    Ok(match r.u()? {
        0 => Constant::Fixnum(r.i()?),
        1 => Constant::Character(u32(r.u()?)?),
        2 => Constant::SingleFloat(f32::from_bits(u32(r.u()?)?)),
        3 => Constant::DoubleFloat(f64::from_bits(r.u()?)),
        4 => Constant::Symbol {
            package: r.s()?,
            name: r.s()?,
        },
        5 => Constant::Object(ConstantIndex(u32(r.u()?)?)),
        6 => Constant::StringBytes(
            (0..r.u()?)
                .map(|_| Ok(u8(r.u()?)?))
                .collect::<Result<Vec<_>, ParseError>>()?,
        ),
        7 => Constant::Nil,
        8 => Constant::T,
        9 => Constant::Unbound,
        10 => Constant::FunctionEntry(FunctionId(u32(r.u()?)?)),
        _ => return Err(ParseError("bad constant".into())),
    })
}

fn block_read(r: &mut Reader<'_>) -> Result<BasicBlock, ParseError> {
    let id = BlockId(u32(r.u()?)?);
    let params = (0..r.u()?)
        .map(|_| {
            Ok(BlockParam {
                value: ValueId(u32(r.u()?)?),
                ty: read_ty(r)?,
            })
        })
        .collect::<Result<Vec<_>, ParseError>>()?;
    let ops = (0..r.u()?)
        .map(|_| {
            let results = (0..r.u()?)
                .map(|_| Ok((ValueId(u32(r.u()?)?), read_ty(r)?)))
                .collect::<Result<Vec<_>, ParseError>>()?;
            let loc = match r.u()? {
                u64::MAX => None,
                n => Some(DebugLocationId(u32(n)?)),
            };
            Ok(Op {
                results,
                loc,
                kind: op_read(r)?,
            })
        })
        .collect::<Result<Vec<_>, ParseError>>()?;
    Ok(BasicBlock {
        id,
        params,
        ops,
        terminator: term_read(r)?,
    })
}

fn vals(r: &mut Reader<'_>) -> Result<Vec<ValueId>, ParseError> {
    (0..r.u()?).map(|_| Ok(ValueId(u32(r.u()?)?))).collect()
}
fn op_read(r: &mut Reader<'_>) -> Result<OpKind, ParseError> {
    let tag = r.u()?;
    let v = |r: &mut Reader<'_>| Ok(ValueId(u32(r.u()?)?));
    Ok(match tag {
        0 => OpKind::Const {
            result: ConstantIndex(u32(r.u()?)?),
        },
        1 => OpKind::Move { value: v(r)? },
        2 => OpKind::Load { address: v(r)? },
        3 => OpKind::Store {
            address: v(r)?,
            value: v(r)?,
        },
        4 => OpKind::LoadField {
            object: v(r)?,
            field: u32(r.u()?)?,
        },
        5 => OpKind::StoreField {
            object: v(r)?,
            field: u32(r.u()?)?,
            value: v(r)?,
        },
        6 => OpKind::Alloc {
            words: u32(r.u()?)?,
        },
        7 => OpKind::LoadArg { index: u8(r.u()?)? },
        8 => OpKind::Call {
            function: v(r)?,
            args: vals(r)?,
        },
        9 => OpKind::CallIndirect {
            callee: v(r)?,
            args: vals(r)?,
        },
        10 => OpKind::MakeClosure {
            entry: v(r)?,
            captures: vals(r)?,
        },
        11 => OpKind::CallClosure {
            closure: v(r)?,
            args: vals(r)?,
        },
        12 => OpKind::Builtin {
            name: r.s()?,
            args: vals(r)?,
        },
        13 => {
            let name = r.s()?;
            let args = vals(r)?;
            let condition = match r.u()? {
                u64::MAX => None,
                n => Some(BlockId(u32(n)?)),
            };
            OpKind::Prim {
                op: prim(&name)?,
                args,
                condition,
            }
        }
        14 => {
            let name = r.s()?;
            OpKind::Compare {
                op: compare(&name)?,
                left: v(r)?,
                right: v(r)?,
            }
        }
        15 => OpKind::Convert {
            op: convert(&r.s()?)?,
            value: v(r)?,
        },
        16 => OpKind::SetMultipleValues { values: vals(r)? },
        17 => OpKind::Safepoint,
        18 => OpKind::EnterHandler {
            region: HandlerRegionId(u32(r.u()?)?),
        },
        19 => OpKind::LeaveHandler {
            region: HandlerRegionId(u32(r.u()?)?),
        },
        _ => return Err(ParseError("bad operation".into())),
    })
}
fn prim(s: &str) -> Result<Prim, ParseError> {
    let names = [
        ("Car", Prim::Car),
        ("Cdr", Prim::Cdr),
        ("Rplaca", Prim::Rplaca),
        ("Rplacd", Prim::Rplacd),
        ("Svref", Prim::Svref),
        ("Aref", Prim::Aref),
        ("Aset", Prim::Aset),
        ("FixnumAdd", Prim::FixnumAdd),
        ("FixnumSub", Prim::FixnumSub),
        ("FixnumMul", Prim::FixnumMul),
        ("FixnumDiv", Prim::FixnumDiv),
        ("FixnumLt", Prim::FixnumLt),
        ("FixnumLe", Prim::FixnumLe),
        ("FixnumEq", Prim::FixnumEq),
        ("Eq", Prim::Eq),
        ("Eql", Prim::Eql),
        ("Typep", Prim::Typep),
    ];
    names
        .into_iter()
        .find(|(n, _)| *n == s)
        .map(|(_, p)| p)
        .ok_or_else(|| ParseError("bad primitive".into()))
}
fn compare(s: &str) -> Result<Compare, ParseError> {
    match s {
        "Eq" => Ok(Compare::Eq),
        "Ne" => Ok(Compare::Ne),
        "Lt" => Ok(Compare::Lt),
        "Le" => Ok(Compare::Le),
        "Gt" => Ok(Compare::Gt),
        "Ge" => Ok(Compare::Ge),
        _ => Err(ParseError("bad comparison".into())),
    }
}
fn convert(s: &str) -> Result<Convert, ParseError> {
    match s {
        "WordToI64" => Ok(Convert::WordToI64),
        "I64ToWord" => Ok(Convert::I64ToWord),
        "WordToF64" => Ok(Convert::WordToF64),
        "F64ToWord" => Ok(Convert::F64ToWord),
        "AddressToWord" => Ok(Convert::AddressToWord),
        "WordToAddress" => Ok(Convert::WordToAddress),
        _ => Err(ParseError("bad conversion".into())),
    }
}

fn term_read(r: &mut Reader<'_>) -> Result<Terminator, ParseError> {
    let tag = r.u()?;
    let val = |r: &mut Reader<'_>| Ok(ValueId(u32(r.u()?)?));
    Ok(match tag {
        0 => Terminator::Jump {
            target: BlockId(u32(r.u()?)?),
            args: vals(r)?,
        },
        1 => {
            let condition = val(r)?;
            let then_target = BlockId(u32(r.u()?)?);
            let then_args = vals(r)?;
            let else_target = BlockId(u32(r.u()?)?);
            let else_args = vals(r)?;
            Terminator::Branch {
                condition,
                then_target,
                then_args,
                else_target,
                else_args,
            }
        }
        2 => {
            let value = val(r)?;
            let cases = (0..r.u()?)
                .map(|_| Ok((r.i()?, BlockId(u32(r.u()?)?), vals(r)?)))
                .collect::<Result<Vec<_>, ParseError>>()?;
            let default = BlockId(u32(r.u()?)?);
            let default_args = vals(r)?;
            Terminator::Switch {
                value,
                cases,
                default,
                default_args,
            }
        }
        3 => Terminator::CallReturn {
            function: val(r)?,
            args: vals(r)?,
        },
        4 => Terminator::TailCall {
            function: val(r)?,
            args: vals(r)?,
        },
        5 => Terminator::Return { values: vals(r)? },
        6 => Terminator::Throw { condition: val(r)? },
        7 => Terminator::Unreachable,
        _ => return Err(ParseError("bad terminator".into())),
    })
}
fn handler_read(r: &mut Reader<'_>) -> Result<HandlerRegion, ParseError> {
    let id = HandlerRegionId(u32(r.u()?)?);
    let kind = match r.u()? {
        0 => HandlerKind::Catch,
        1 => HandlerKind::UnwindProtect,
        2 => HandlerKind::Progv,
        _ => return Err(ParseError("bad handler kind".into())),
    };
    let protected = (0..r.u()?)
        .map(|_| Ok(BlockId(u32(r.u()?)?)))
        .collect::<Result<Vec<_>, ParseError>>()?;
    let handler = BlockId(u32(r.u()?)?);
    let cleanup = if r.b()? {
        Some(BlockId(u32(r.u()?)?))
    } else {
        None
    };
    let catch_tag = if r.b()? {
        Some(ValueId(u32(r.u()?)?))
    } else {
        None
    };
    let binding_targets = (0..r.u()?)
        .map(|_| Ok(ValueId(u32(r.u()?)?)))
        .collect::<Result<Vec<_>, ParseError>>()?;
    Ok(HandlerRegion {
        id,
        kind,
        protected,
        handler,
        cleanup,
        catch_tag,
        binding_targets,
        depth: u32(r.u()?)?,
        parent: if r.b()? {
            Some(HandlerRegionId(u32(r.u()?)?))
        } else {
            None
        },
    })
}
