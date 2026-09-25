//! Text serialization printer.
use crate::{BasicBlock, Constant, Function, OpKind, Terminator, Ty};
use std::fmt::{self, Display, Formatter, Write};

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
        w.u(h.id.0.into());
        w.u(match h.kind {
            crate::HandlerKind::Catch => 0,
            crate::HandlerKind::UnwindProtect => 1,
            crate::HandlerKind::Progv => 2,
        });
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
        vec_len(w, h.binding_targets.len());
        for v in &h.binding_targets {
            w.u(v.0.into());
        }
        w.u(h.depth.into());
        w.b(h.parent.is_some());
        if let Some(parent) = h.parent { w.u(parent.0.into()); }
    }
    vec_len(w, f.debug.len());
    for d in &f.debug {
        w.u(d.file.0.into());
        w.u(d.line.into());
        w.u(d.column.into());
        w.u(d.form.0.into());
    }
}
fn constant(w: &mut Writer, c: &Constant) {
    match c {
        Constant::Fixnum(n) => {
            w.u(0);
            w.i(*n);
        }
        Constant::Character(n) => {
            w.u(1);
            w.u((*n).into());
        }
        Constant::SingleFloat(n) => {
            w.u(2);
            w.u(n.to_bits().into());
        }
        Constant::DoubleFloat(n) => {
            w.u(3);
            w.u(n.to_bits());
        }
        Constant::Symbol { package, name } => {
            w.u(4);
            w.s(package);
            w.s(name);
        }
        Constant::Object(i) => {
            w.u(5);
            w.u(i.0.into());
        }
        Constant::StringBytes(v) => {
            w.u(6);
            vec_len(w, v.len());
            for b in v {
                w.u((*b).into());
            }
        }
        Constant::Nil => w.u(7),
        Constant::T => w.u(8),
        Constant::Unbound => w.u(9),
        Constant::FunctionEntry(id) => { w.u(10); w.u(id.0.into()); }
    }
}
fn block(w: &mut Writer, b: &BasicBlock) {
    w.u(b.id.0.into());
    vec_len(w, b.params.len());
    for p in &b.params {
        w.u(p.value.0.into());
        ty(w, p.ty);
    }
    vec_len(w, b.ops.len());
    for o in &b.ops {
        w.u(o.results.len() as u64);
        for (v, t) in &o.results {
            w.u(v.0.into());
            ty(w, *t);
        }
        w.u(o.loc.map_or(u64::MAX, |v| v.0.into()));
        op(w, &o.kind);
    }
    term(w, &b.terminator);
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
        OpKind::MakeClosure { .. } => 10,
        OpKind::CallClosure { .. } => 11,
        OpKind::Builtin { .. } => 12,
        OpKind::Prim { .. } => 13,
        OpKind::Compare { .. } => 14,
        OpKind::Convert { .. } => 15,
        OpKind::SetMultipleValues { .. } => 16,
        OpKind::Safepoint => 17,
        OpKind::EnterHandler { .. } => 18,
        OpKind::LeaveHandler { .. } => 19,
    };
    w.u(tag);
    match o {
        OpKind::Const { result } => w.u(result.0.into()),
        OpKind::Move { value }
        | OpKind::Load { address: value }
        | OpKind::Convert { value, .. } => w.u(value.0.into()),
        OpKind::Store { address, value } => {
            w.u(address.0.into());
            w.u(value.0.into());
        }
        OpKind::LoadField { object, field } => {
            w.u(object.0.into());
            w.u((*field).into());
        }
        OpKind::StoreField {
            object,
            field,
            value,
        } => {
            w.u(object.0.into());
            w.u((*field).into());
            w.u(value.0.into());
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
                w.u(v.0.into());
            }
        }
        OpKind::MakeClosure { entry, captures } => {
            w.u(entry.0.into());
            vec_len(w, captures.len());
            for v in captures { w.u(v.0.into()); }
        }
        OpKind::CallClosure { closure, args } => {
            w.u(closure.0.into());
            vec_len(w, args.len());
            for v in args { w.u(v.0.into()); }
        }
        OpKind::Builtin { name, args } => {
            w.s(name);
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into());
            }
        }
        OpKind::Compare { op, left, right } => {
            w.s(&format!("{op:?}"));
            w.u(left.0.into());
            w.u(right.0.into());
        }
        OpKind::Prim {
            op,
            args,
            condition,
        } => {
            w.s(&format!("{op:?}"));
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into());
            }
            w.u(condition.map_or(u64::MAX, |v| v.0.into()));
        }
        OpKind::SetMultipleValues { values } => {
            vec_len(w, values.len());
            for v in values {
                w.u(v.0.into());
            }
        }
        OpKind::Safepoint => {}
        OpKind::EnterHandler { region } | OpKind::LeaveHandler { region } => w.u(region.0.into()),
    }
}
fn term(w: &mut Writer, t: &Terminator) {
    match t {
        Terminator::Jump { target, args } => {
            w.u(0);
            w.u(target.0.into());
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into());
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
                w.u(v.0.into());
            }
            w.u(else_target.0.into());
            vec_len(w, else_args.len());
            for v in else_args {
                w.u(v.0.into());
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
                    w.u(v.0.into());
                }
            }
            w.u(default.0.into());
            vec_len(w, default_args.len());
            for v in default_args {
                w.u(v.0.into());
            }
        }
        Terminator::CallReturn { function, args } => {
            w.u(3);
            w.u(function.0.into());
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into());
            }
        }
        Terminator::TailCall { function, args } => {
            w.u(4);
            w.u(function.0.into());
            vec_len(w, args.len());
            for v in args {
                w.u(v.0.into());
            }
        }
        Terminator::Return { values } => {
            w.u(5);
            vec_len(w, values.len());
            for v in values {
                w.u(v.0.into());
            }
        }
        Terminator::Throw { condition } => {
            w.u(6);
            w.u(condition.0.into());
        }
        Terminator::Unreachable => w.u(7),
    }
}
