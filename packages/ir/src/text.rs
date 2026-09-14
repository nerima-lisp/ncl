//! Stable, dependency-free textual serialization for IR.
#![allow(clippy::too_many_lines)]
#![allow(clippy::all)]

use crate::*;
use std::error::Error;
use std::fmt::{self, Display, Formatter, Write};

/// An error returned when parsing an IR dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError(pub String);
impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ParseError {}

fn esc(s: &str) -> String {
    s.bytes().fold(String::new(), |mut out, byte| {
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' {
            out.push(byte as char);
        } else {
            let _ = write!(out, "_{byte:02x}");
        }
        out
    })
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

impl Display for Function {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut w = Writer::default();
        encode_function(&mut w, self);
        writeln!(f, "fn @{} {{ {} }}", esc(&self.name), w.0)
    }
}

#[derive(Default)]
struct Writer(String);
impl Writer {
    fn u(&mut self, n: u64) {
        let _ = write!(self.0, "{n:x},");
    }
    fn i(&mut self, n: i64) {
        let _ = write!(self.0, "i{n:x},");
    }
    fn s(&mut self, s: &str) {
        self.0.push_str(&esc(s));
        self.0.push(',');
    }
    fn b(&mut self, b: bool) {
        self.u(u64::from(b));
    }
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
    fn done(&self) -> bool {
        self.at == self.fields.len()
    }
}
fn ty(w: &mut Writer, t: Ty) {
    w.u(match t {
        Ty::Word => 0,
        Ty::I64 => 1,
        Ty::F64 => 2,
        Ty::Address => 3,
        Ty::Bool => 4,
        Ty::Unit => 5,
    });
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
fn vec_len(w: &mut Writer, n: usize) {
    w.u(n as u64);
}
fn encode_function(w: &mut Writer, f: &Function) {
    w.u(f.id.0.into());
    w.s(&f.name);
    vec_len(w, f.params.len());
    for p in &f.params {
        w.s(&p.name);
        ty(w, p.ty);
    }
    vec_len(w, f.return_types.len());
    for t in &f.return_types {
        ty(w, *t);
    }
    vec_len(w, f.constants.len());
    for c in &f.constants {
        constant(w, c);
    }
    vec_len(w, f.blocks.len());
    for b in &f.blocks {
        block(w, b);
    }
    vec_len(w, f.locals.len());
    for l in &f.locals {
        w.u(l.id.0.into());
        w.s(&l.name);
        ty(w, l.ty);
    }
    vec_len(w, f.handler_regions.len());
    for h in &f.handler_regions {
        vec_len(w, h.protected.len());
        for b in &h.protected {
            w.u(b.0.into());
        }
        w.u(h.handler.0.into());
        w.b(h.cleanup.is_some());
        if let Some(b) = h.cleanup {
            w.u(b.0.into());
        }
        w.b(h.catch_tag.is_some());
        if let Some(v) = h.catch_tag {
            w.u(v.0.into());
        }
        w.u(h.depth.into());
    }
    vec_len(w, f.debug.len());
    for d in &f.debug {
        w.u(d.file.0.into());
        w.u(d.line.into());
        w.u(d.column.into());
        w.u(d.form.0.into());
    }
}
fn decode_function(r: &mut Reader<'_>) -> Result<Function, ParseError> {
    let id = FunctionId(r.u()? as u32);
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
                id: LocalId(r.u()? as u32),
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
                file: FileId(r.u()? as u32),
                line: r.u()? as u32,
                column: r.u()? as u32,
                form: FormId(r.u()? as u32),
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
fn constant(w: &mut Writer, c: &Constant) {
    match c {
        Constant::Fixnum(n) => {
            w.u(0);
            w.i(*n)
        }
        Constant::Character(n) => {
            w.u(1);
            w.u((*n).into())
        }
        Constant::SingleFloat(n) => {
            w.u(2);
            w.u(n.to_bits().into())
        }
        Constant::DoubleFloat(n) => {
            w.u(3);
            w.u(n.to_bits())
        }
        Constant::Symbol { package, name } => {
            w.u(4);
            w.s(package);
            w.s(name)
        }
        Constant::Object(i) => {
            w.u(5);
            w.u(i.0.into())
        }
        Constant::StringBytes(v) => {
            w.u(6);
            vec_len(w, v.len());
            for b in v {
                w.u((*b).into())
            }
        }
        Constant::Nil => w.u(7),
        Constant::T => w.u(8),
        Constant::Unbound => w.u(9),
    }
}
fn constant_read(r: &mut Reader<'_>) -> Result<Constant, ParseError> {
    Ok(match r.u()? {
        0 => Constant::Fixnum(r.i()?),
        1 => Constant::Character(r.u()? as u32),
        2 => Constant::SingleFloat(f32::from_bits(r.u()? as u32)),
        3 => Constant::DoubleFloat(f64::from_bits(r.u()?)),
        4 => Constant::Symbol {
            package: r.s()?,
            name: r.s()?,
        },
        5 => Constant::Object(ConstantIndex(r.u()? as u32)),
        6 => Constant::StringBytes(
            (0..r.u()?)
                .map(|_| Ok(r.u()? as u8))
                .collect::<Result<Vec<_>, ParseError>>()?,
        ),
        7 => Constant::Nil,
        8 => Constant::T,
        9 => Constant::Unbound,
        _ => return Err(ParseError("bad constant".into())),
    })
}
fn block(w: &mut Writer, b: &BasicBlock) {
    w.u(b.id.0.into());
    vec_len(w, b.params.len());
    for p in &b.params {
        w.u(p.value.0.into());
        ty(w, p.ty)
    }
    vec_len(w, b.ops.len());
    for o in &b.ops {
        w.u(o.results.len() as u64);
        for (v, t) in &o.results {
            w.u(v.0.into());
            ty(w, *t)
        }
        w.u(o.loc.map_or(u64::MAX, |v| v.0.into()));
        op(w, &o.kind)
    }
    term(w, &b.terminator)
}
fn block_read(r: &mut Reader<'_>) -> Result<BasicBlock, ParseError> {
    let id = BlockId(r.u()? as u32);
    let params = (0..r.u()?)
        .map(|_| {
            Ok(BlockParam {
                value: ValueId(r.u()? as u32),
                ty: read_ty(r)?,
            })
        })
        .collect::<Result<Vec<_>, ParseError>>()?;
    let ops = (0..r.u()?)
        .map(|_| {
            let results = (0..r.u()?)
                .map(|_| Ok((ValueId(r.u()? as u32), read_ty(r)?)))
                .collect::<Result<Vec<_>, ParseError>>()?;
            let loc = match r.u()? {
                u64::MAX => None,
                n => Some(DebugLocationId(n as u32)),
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
fn op(w: &mut Writer, o: &OpKind) {
    let tag = match o {
        OpKind::Const { .. } => 0,
        OpKind::Move { .. } => 1,
        OpKind::Load { .. } => 2,
        OpKind::Store { .. } => 3,
        OpKind::LoadField { .. } => 4,
        OpKind::StoreField { .. } => 5,
        OpKind::Alloc { .. } => 6,
        OpKind::LoadArg { .. } => 7,
        OpKind::Call { .. } => 8,
        OpKind::CallIndirect { .. } => 9,
        OpKind::Builtin { .. } => 10,
        OpKind::Prim { .. } => 11,
        OpKind::Compare { .. } => 12,
        OpKind::Convert { .. } => 13,
        OpKind::SetMultipleValues { .. } => 14,
        OpKind::Safepoint => 15,
    };
    w.u(tag);
    match o {
        OpKind::Const { result } => w.u(result.0.into()),
        OpKind::Move { value }
        | OpKind::Load { address: value }
        | OpKind::Convert { value, .. } => w.u(value.0.into()),
        OpKind::Store { address, value } => {
            w.u(address.0.into());
            w.u(value.0.into())
        }
        OpKind::LoadField { object, field } => {
            w.u(object.0.into());
            w.u((*field).into())
        }
        OpKind::StoreField {
            object,
            field,
            value,
        } => {
            w.u(object.0.into());
            w.u((*field).into());
            w.u(value.0.into())
        }
        OpKind::Alloc { words } => w.u((*words).into()),
        OpKind::LoadArg { index } => w.u((*index).into()),
        OpKind::Call { function, args }
        | OpKind::CallIndirect {
            callee: function,
            args,
        } => {
            w.u(function.0.into());
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into())
            }
        }
        OpKind::Builtin { name, args } => {
            w.s(name);
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into())
            }
        }
        OpKind::Compare { op, left, right } => {
            w.s(&format!("{op:?}"));
            w.u(left.0.into());
            w.u(right.0.into())
        }
        OpKind::Prim {
            op,
            args,
            condition,
        } => {
            w.s(&format!("{op:?}"));
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into())
            }
            w.u(condition.map_or(u64::MAX, |v| v.0.into()))
        }
        OpKind::SetMultipleValues { values } => {
            vec_len(w, values.len());
            for v in values {
                w.u(v.0.into())
            }
        }
        OpKind::Safepoint => {}
    }
}
fn vals(r: &mut Reader<'_>) -> Result<Vec<ValueId>, ParseError> {
    (0..r.u()?).map(|_| Ok(ValueId(r.u()? as u32))).collect()
}
fn op_read(r: &mut Reader<'_>) -> Result<OpKind, ParseError> {
    let tag = r.u()?;
    let v = |r: &mut Reader<'_>| Ok(ValueId(r.u()? as u32));
    Ok(match tag {
        0 => OpKind::Const {
            result: ConstantIndex(r.u()? as u32),
        },
        1 => OpKind::Move { value: v(r)? },
        2 => OpKind::Load { address: v(r)? },
        3 => OpKind::Store {
            address: v(r)?,
            value: v(r)?,
        },
        4 => OpKind::LoadField {
            object: v(r)?,
            field: r.u()? as u32,
        },
        5 => OpKind::StoreField {
            object: v(r)?,
            field: r.u()? as u32,
            value: v(r)?,
        },
        6 => OpKind::Alloc {
            words: r.u()? as u32,
        },
        7 => OpKind::LoadArg {
            index: r.u()? as u8,
        },
        8 => OpKind::Call {
            function: v(r)?,
            args: vals(r)?,
        },
        9 => OpKind::CallIndirect {
            callee: v(r)?,
            args: vals(r)?,
        },
        10 => OpKind::Builtin {
            name: r.s()?,
            args: vals(r)?,
        },
        11 => {
            let name = r.s()?;
            let args = vals(r)?;
            let condition = match r.u()? {
                u64::MAX => None,
                n => Some(BlockId(n as u32)),
            };
            OpKind::Prim {
                op: prim(&name)?,
                args,
                condition,
            }
        }
        12 => {
            let name = r.s()?;
            OpKind::Compare {
                op: compare(&name)?,
                left: v(r)?,
                right: v(r)?,
            }
        }
        13 => OpKind::Convert {
            op: convert(&r.s()?)?,
            value: v(r)?,
        },
        14 => OpKind::SetMultipleValues { values: vals(r)? },
        15 => OpKind::Safepoint,
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
fn term(w: &mut Writer, t: &Terminator) {
    match t {
        Terminator::Jump { target, args } => {
            w.u(0);
            w.u(target.0.into());
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into())
            }
        }
        Terminator::Branch {
            condition,
            then_target,
            then_args,
            else_target,
            else_args,
        } => {
            w.u(1);
            w.u(condition.0.into());
            w.u(then_target.0.into());
            vec_len(w, then_args.len());
            for v in then_args {
                w.u(v.0.into())
            }
            w.u(else_target.0.into());
            vec_len(w, else_args.len());
            for v in else_args {
                w.u(v.0.into())
            }
        }
        Terminator::Switch {
            value,
            cases,
            default,
            default_args,
        } => {
            w.u(2);
            w.u(value.0.into());
            vec_len(w, cases.len());
            for (k, b, a) in cases {
                w.i(*k);
                w.u(b.0.into());
                vec_len(w, a.len());
                for v in a {
                    w.u(v.0.into())
                }
            }
            w.u(default.0.into());
            vec_len(w, default_args.len());
            for v in default_args {
                w.u(v.0.into())
            }
        }
        Terminator::CallReturn { function, args } => {
            w.u(3);
            w.u(function.0.into());
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into())
            }
        }
        Terminator::TailCall { function, args } => {
            w.u(4);
            w.u(function.0.into());
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into())
            }
        }
        Terminator::Return { values } => {
            w.u(5);
            vec_len(w, values.len());
            for v in values {
                w.u(v.0.into())
            }
        }
        Terminator::Throw { condition } => {
            w.u(6);
            w.u(condition.0.into())
        }
        Terminator::Unreachable => w.u(7),
    }
}
fn term_read(r: &mut Reader<'_>) -> Result<Terminator, ParseError> {
    let tag = r.u()?;
    let val = |r: &mut Reader<'_>| Ok(ValueId(r.u()? as u32));
    Ok(match tag {
        0 => Terminator::Jump {
            target: BlockId(r.u()? as u32),
            args: vals(r)?,
        },
        1 => {
            let condition = val(r)?;
            let then_target = BlockId(r.u()? as u32);
            let then_args = vals(r)?;
            let else_target = BlockId(r.u()? as u32);
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
                .map(|_| Ok((r.i()?, BlockId(r.u()? as u32), vals(r)?)))
                .collect::<Result<Vec<_>, ParseError>>()?;
            let default = BlockId(r.u()? as u32);
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
    let protected = (0..r.u()?)
        .map(|_| Ok(BlockId(r.u()? as u32)))
        .collect::<Result<Vec<_>, ParseError>>()?;
    let handler = BlockId(r.u()? as u32);
    let cleanup = if r.b()? {
        Some(BlockId(r.u()? as u32))
    } else {
        None
    };
    let catch_tag = if r.b()? {
        Some(ValueId(r.u()? as u32))
    } else {
        None
    };
    Ok(HandlerRegion {
        protected,
        handler,
        cleanup,
        catch_tag,
        depth: r.u()? as u32,
    })
}
