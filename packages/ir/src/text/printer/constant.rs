use crate::Constant;

use super::{Writer, vec_len};

pub(super) fn constant(w: &mut Writer, c: &Constant) {
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
        Constant::LiteralObject { identity, value } => {
            w.u(16);
            w.u((*identity).into());
            w.u(value.0.into());
        }
        Constant::StringBytes(v) => {
            w.u(6);
            vec_len(w, v.len());
            for b in v {
                w.u((*b).into());
            }
        }
        Constant::Structure { kind, elements } => {
            w.u(11);
            structure_kind(w, kind);
            vec_len(w, elements.len());
            for element in elements {
                w.u(element.0.into());
            }
        }
        Constant::Nil => w.u(7),
        Constant::T => w.u(8),
        Constant::Unbound => w.u(9),
        Constant::FunctionEntry(id) => {
            w.u(10);
            w.u(id.0.into());
        }
        Constant::Bignum { negative, limbs } => {
            w.u(12);
            w.b(*negative);
            vec_len(w, limbs.len());
            for limb in limbs {
                w.u((*limb).into());
            }
        }
        Constant::Ratio {
            numerator,
            denominator,
        } => {
            w.u(13);
            w.u(numerator.0.into());
            w.u(denominator.0.into());
        }
        Constant::Complex { real, imaginary } => {
            w.u(14);
            w.u(real.0.into());
            w.u(imaginary.0.into());
        }
        Constant::UninternedSymbol { identity, name } => {
            w.u(15);
            w.u((*identity).into());
            w.s(name);
        }
    }
}

fn structure_kind(w: &mut Writer, kind: &crate::StructureKind) {
    match kind {
        crate::StructureKind::Cons => w.u(0),
        crate::StructureKind::SimpleVector => w.u(1),
        crate::StructureKind::SpecializedArray { element_type } => {
            w.u(2);
            w.u(match element_type {
                crate::SpecializedArrayElementType::Bit => 0,
                crate::SpecializedArrayElementType::Character => 1,
                crate::SpecializedArrayElementType::BaseChar => 2,
                crate::SpecializedArrayElementType::Fixnum => 3,
                crate::SpecializedArrayElementType::Signed => 4,
                crate::SpecializedArrayElementType::Unsigned => 5,
                crate::SpecializedArrayElementType::SingleFloat => 6,
                crate::SpecializedArrayElementType::DoubleFloat => 7,
            });
        }
        crate::StructureKind::Array {
            dimensions,
            element_type,
        } => {
            w.u(3);
            w.u(match element_type {
                crate::ArrayElementType::T => 0,
                crate::ArrayElementType::Bit => 1,
                crate::ArrayElementType::Character => 2,
                crate::ArrayElementType::BaseChar => 3,
                crate::ArrayElementType::Fixnum => 4,
                crate::ArrayElementType::Signed => 5,
                crate::ArrayElementType::Unsigned => 6,
                crate::ArrayElementType::SingleFloat => 7,
                crate::ArrayElementType::DoubleFloat => 8,
            });
            vec_len(w, dimensions.len());
            for dimension in dimensions {
                vec_len(w, *dimension);
            }
        }
    }
}
