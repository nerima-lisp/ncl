use core::fmt;
use std::collections::HashMap;
use crate::model::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub enum FixupKind { Rel32, Abs64 }
#[derive(Clone, Copy, Debug, Eq, PartialEq)] pub struct Fixup { pub offset: usize, pub kind: FixupKind, pub target: Label }
#[derive(Clone, Debug, Eq, PartialEq)] pub struct CodeBlob { pub bytes: Vec<u8>, pub fixups: Vec<Fixup> }
#[derive(Clone, Debug, Eq, PartialEq)] pub enum EncodeError { UnboundLabel(Label), Rel32OutOfRange, InvalidOperand(&'static str), InvalidNopLength, InvalidRegister(u8), BufferTooLarge }
impl fmt::Display for EncodeError { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { match self {Self::UnboundLabel(l)=>write!(f,"unbound label {:?}",l),Self::Rel32OutOfRange=>write!(f,"rel32 out of range"),Self::InvalidOperand(s)=>f.write_str(s),Self::InvalidNopLength=>f.write_str("nop length must be 1..=9"),Self::InvalidRegister(n)=>write!(f,"invalid register {n}"),Self::BufferTooLarge=>f.write_str("buffer too large")} } }
impl std::error::Error for EncodeError {}
#[derive(Clone, Debug, Eq, PartialEq)] pub enum DecodeError { Truncated, Unsupported, Invalid }
impl fmt::Display for DecodeError { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result { f.write_str(match self {Self::Truncated=>"truncated instruction",Self::Unsupported=>"unsupported instruction",Self::Invalid=>"invalid instruction"}) } }
impl std::error::Error for DecodeError {}

#[derive(Debug, Default)] pub struct Assembler { bytes:Vec<u8>, labels:HashMap<Label,usize>, fixups:Vec<Fixup>, next:u32 }
impl Assembler {
 pub fn new()->Self { Self::default() }
 pub fn new_label(&mut self)->Label { let l=Label(self.next); self.next=self.next.wrapping_add(1); l }
 pub fn bind(&mut self,l:Label) { self.labels.insert(l,self.bytes.len()); }
 pub fn emit(&mut self,i:&Inst)->Result<(),EncodeError> { encode_inst(i,&mut self.bytes,&mut self.fixups) }
 pub fn finish(mut self)->Result<CodeBlob,EncodeError> { for f in &self.fixups { let Some(&at)=self.labels.get(&f.target) else { return Err(EncodeError::UnboundLabel(f.target)); }; let end=f.offset+4; let d=at as i64-end as i64; if d < i32::MIN as i64 || d > i32::MAX as i64 {return Err(EncodeError::Rel32OutOfRange)} self.bytes[f.offset..end].copy_from_slice(&(d as i32).to_le_bytes()); } Ok(CodeBlob{bytes:self.bytes,fixups:self.fixups}) }
 pub fn bytes(&self)->&[u8] { &self.bytes }
}
fn rex(b:&mut Vec<u8>,w:bool,r:u8,x:u8,bb:u8) { let v=0x40|(w as u8)<<3|((r>>3)&1)<<2|((x>>3)&1)<<1|((bb>>3)&1); if v!=0x40 {b.push(v)} }
fn modrm(b:&mut Vec<u8>,m:u8,r:u8,rm:u8){b.push((m<<6)|((r&7)<<3)|(rm&7));}
fn imm32(b:&mut Vec<u8>,v:i32){b.extend(v.to_le_bytes())} fn imm64(b:&mut Vec<u8>,v:i64){b.extend(v.to_le_bytes())}
fn mem(b:&mut Vec<u8>, reg:u8, m:Mem, w:bool)->Result<(),EncodeError>{
 let base=m.base.map(Reg::code); let idx=m.index.map(Reg::code); if base.is_none() && idx.is_some(){return Err(EncodeError::InvalidOperand("index without base is unsupported"));}
 if base.is_none(){modrm(b,0,reg,4);b.push((m.scale.bits()<<6)|((idx.unwrap_or(4)&7)<<3)|5);imm32(b,m.disp);return Ok(())}
 let bc=base.unwrap(); let need_sib=bc&7==4||idx.is_some(); let md=if m.disp==0 && bc&7!=5 {0} else if i8::try_from(m.disp).is_ok(){1}else{2}; modrm(b,md,reg,if need_sib{4}else{bc}); if need_sib {b.push((m.scale.bits()<<6)|((idx.unwrap_or(4)&7)<<3)|(bc&7));} if md==1 {b.push(m.disp as i8 as u8)} else if md==2 || (md==0 && bc&7==5){imm32(b,m.disp)} Ok(()) }
fn encode_inst(i:&Inst,b:&mut Vec<u8>,fx:&mut Vec<Fixup>)->Result<(),EncodeError>{
 match *i {
  Inst::MovRR(d,s)=>{rex(b,true,s.code(),4,d.code());b.push(0x89);modrm(b,3,s.code(),d.code())},
  Inst::MovRI(r,Imm::I64(v))=>{rex(b,true,4,4,r.code());b.push(0xb8+(r.code()&7));imm64(b,v)},
  Inst::MovRI(r,Imm::I32(v))=>{rex(b,true,4,4,r.code());b.push(0xc7);modrm(b,3,0,r.code());imm32(b,v)},
  Inst::MovRI(r,Imm::I8(v))=>{rex(b,true,4,4,r.code());b.push(0xc7);modrm(b,3,0,r.code());imm32(b,v as i32)},
  Inst::MovRM(r,m)=>{rex(b,true,r.code(),m.index.map_or(4,Reg::code),m.base.map_or(0,Reg::code));b.push(0x8b);mem(b,r.code(),m,true)?}, Inst::MovMR(m,r)=>{rex(b,true,r.code(),m.index.map_or(4,Reg::code),m.base.map_or(0,Reg::code));b.push(0x89);mem(b,r.code(),m,true)?},
  Inst::MovMI(m,v)=>{b.push(0xc7);mem(b,0,m,true)?;imm32(b,v)},
  Inst::Lea(r,m)=>{rex(b,true,r.code(),m.index.map_or(4,Reg::code),m.base.map_or(0,Reg::code));b.push(0x8d);mem(b,r.code(),m,true)?},
  Inst::BinRR(op,d,s)=>{let (o,ext)=bin(op);b.push(o);rex(b,true,s.code(),4,d.code());modrm(b,3,s.code(),d.code()); let _=ext;},
  Inst::BinRI(op,r,v)=>{let (_,ext)=bin(op); if (-128..=127).contains(&v) {b.push(0x83)} else {b.push(0x81)} rex(b,true,0,4,r.code());modrm(b,3,ext,r.code());if b.last()==Some(&0x83){b.push(v as i8 as u8)}else{imm32(b,v)}},
  Inst::BinRM(op,r,m)=>{let (o,_)=bin(op);b.push(o);mem(b,r.code(),m,true)?}, Inst::BinMR(op,m,r)=>{let (o,_)=bin(op);b.push(o);mem(b,r.code(),m,true)?},
  Inst::CmpRR(d,s)=>{b.push(0x39);rex(b,true,s.code(),4,d.code());modrm(b,3,s.code(),d.code())}, Inst::CmpRI(r,v)=>{b.push(if (-128..=127).contains(&v){0x83}else{0x81});rex(b,true,0,4,r.code());modrm(b,3,7,r.code());if (-128..=127).contains(&v){b.push(v as i8 as u8)}else{imm32(b,v)}}, Inst::CmpRM(r,m)=>{b.push(0x3b);mem(b,r.code(),m,true)?}, Inst::CmpMR(m,r)=>{b.push(0x39);mem(b,r.code(),m,true)?},
  Inst::TestRR(d,s)=>{b.push(0x85);rex(b,true,s.code(),4,d.code());modrm(b,3,s.code(),d.code())},
  Inst::ImulRR(d,s)=>{b.extend([0x0f,0xaf]);rex(b,true,d.code(),4,s.code());modrm(b,3,d.code(),s.code())}, Inst::ImulRRI(d,s,v)=>{b.push(0x69);rex(b,true,d.code(),4,s.code());modrm(b,3,d.code(),s.code());imm32(b,v)},
  Inst::Neg(r)=>unary(b,3,r),Inst::Not(r)=>unary(b,2,r),Inst::Inc(r)=>unary(b,0,r),Inst::Dec(r)=>unary(b,1,r), Inst::Cqo=>b.push(0x99), Inst::Idiv(r)=>unary(b,7,r),
  Inst::ShiftImm(s,r,v)=>{b.push(if v<=127{0xc1}else{0xc1});rex(b,true,0,4,r.code());modrm(b,3,shift(s),r.code());b.push(v)}, Inst::ShiftCl(s,r)=>{b.push(0xd3);rex(b,true,0,4,r.code());modrm(b,3,shift(s),r.code())},
  Inst::Setcc(c,r)=>{b.extend([0x0f,0x90|c.code()]);rex(b,false,0,4,r.code());modrm(b,3,0,r.code())}, Inst::Cmovcc(c,d,s)=>{b.extend([0x0f,0x40|c.code()]);rex(b,true,d.code(),4,s.code());modrm(b,3,d.code(),s.code())},
  Inst::Jmp(l)=>rel(b,fx,0xe9,l),Inst::Jcc(c,l)=>{b.push(0x0f);b.push(0x80|c.code());rel_at(b,fx,l)},Inst::Call(l)=>rel(b,fx,0xe8,l), Inst::JmpReg(r)=>{unary(b,4,r)}, Inst::CallReg(r)=>{unary(b,2,r)}, Inst::JmpMem(m)=>{b.push(0xff);mem(b,4,m,false)?},Inst::CallMem(m)=>{b.push(0xff);mem(b,2,m,false)?},
  Inst::Ret=>b.push(0xc3), Inst::Push(r)=>{if r.code()>=8{b.push(0x41)}b.push(0x50+(r.code()&7))}, Inst::Pop(r)=>{if r.code()>=8{b.push(0x41)}b.push(0x58+(r.code()&7))}, Inst::Nop(n)=>nop(b,n)?, Inst::Ud2=>b.extend([0x0f,0x0b]),Inst::Int3=>b.push(0xcc),
  Inst::Xchg(d,s)=>{b.push(0x87);rex(b,true,s.code(),4,d.code());modrm(b,3,s.code(),d.code())}, Inst::Bt(r,v)=>{b.extend([0x0f,0xba]);rex(b,true,0,4,r.code());modrm(b,3,4,r.code());b.push(v)},
  Inst::Movzx(d,s,w)=>{b.extend(if w==8{[0x0f,0xb6]}else{[0x0f,0xb7]});rex(b,true,d.code(),4,s.code());modrm(b,3,d.code(),s.code())},
  Inst::Movsx(d,s,w)=>{b.extend(if w==8{[0x0f,0xbe]}else if w==16{[0x0f,0xbf]}else{[0x63,0x00]});if w==32{b.pop();}rex(b,true,d.code(),4,s.code());modrm(b,3,d.code(),s.code())},
  Inst::MovsdRM(x,m)=>{b.extend([0xf2,0x0f,0x10]);mem(b,x.0,m,false)?}, Inst::MovsdMR(m,x)=>{b.extend([0xf2,0x0f,0x11]);mem(b,x.0,m,false)?},
  Inst::MovqXR(x,r)=>{b.extend([0x66,0x0f,0x6e]);rex(b,false,x.0,4,r.code());modrm(b,3,x.0,r.code())}, Inst::MovqRX(r,x)=>{b.extend([0x66,0x0f,0x7e]);rex(b,false,x.0,4,r.code());modrm(b,3,x.0,r.code())},
  Inst::Sse(op,d,s)=>sse(b,op,d,s), Inst::Xorpd(d,s)=>{b.extend([0x66,0x0f,0x57]);rex(b,false,d.0,4,s.0);modrm(b,3,d.0,s.0)},
  Inst::Cvtsi2sd(x,r)=>{b.extend([0xf2,0x0f,0x2a]);rex(b,true,x.0,4,r.code());modrm(b,3,x.0,r.code())}, Inst::Cvttsd2si(r,x)=>{b.extend([0xf2,0x0f,0x2c]);rex(b,true,r.code(),4,x.0);modrm(b,3,r.code(),x.0)},
 } Ok(()) }
fn bin(o:BinOp)->(u8,u8){match o{BinOp::Add=>(0x01,0),BinOp::Or=>(0x09,1),BinOp::And=>(0x21,4),BinOp::Sub=>(0x29,5),BinOp::Xor=>(0x31,6)}} fn shift(s:Shift)->u8{match s{Shift::Shl=>4,Shift::Shr=>5,Shift::Sar=>7}}
fn unary(b:&mut Vec<u8>,e:u8,r:Reg){b.push(0xf7);rex(b,true,0,4,r.code());modrm(b,3,e,r.code())}
fn rel(b:&mut Vec<u8>,f:&mut Vec<Fixup>,op:u8,l:Label){b.push(op);rel_at(b,f,l)} fn rel_at(b:&mut Vec<u8>,f:&mut Vec<Fixup>,l:Label){let o=b.len();b.extend([0;4]);f.push(Fixup{offset:o,kind:FixupKind::Rel32,target:l})}
fn nop(b:&mut Vec<u8>,n:u8)->Result<(),EncodeError>{if !(1..=9).contains(&n){return Err(EncodeError::InvalidNopLength)};b.extend(std::iter::repeat_n(0x90,n as usize));Ok(())}
fn sse(b:&mut Vec<u8>,op:SseOp,d:Xmm,s:Xmm){let code=match op{SseOp::Addsd=>0x58,SseOp::Subsd=>0x5c,SseOp::Mulsd=>0x59,SseOp::Divsd=>0x5e,SseOp::Ucomisd=>0x2e,SseOp::Sqrtsd=>0x51};b.extend([0xf2,0x0f,code]);rex(b,false,d.0,4,s.0);modrm(b,3,d.0,s.0)}

pub fn decode(bytes:&[u8])->Result<Vec<Inst>,DecodeError>{let mut out=Vec::new();let mut p=0;while p<bytes.len(){match bytes[p]{0x90=>{out.push(Inst::Nop(1));p+=1},0xc3=>{out.push(Inst::Ret);p+=1},0xcc=>{out.push(Inst::Int3);p+=1},0x0f if bytes.get(p+1)==Some(&0x0b)=>{out.push(Inst::Ud2);p+=2},_=>return Err(DecodeError::Unsupported)}}Ok(out)}
pub fn display(i:&Inst)->String{format!("{i:?}")}
