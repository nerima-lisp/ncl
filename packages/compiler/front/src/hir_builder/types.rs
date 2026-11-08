use ncl_hir::cl_type::numeric::{NumericBound, NumericDomain, NumericValue};
use ncl_hir::cl_type::{BuiltinType, MemberValue, OpaqueOperand};
use ncl_hir::{TypeId, TypeSpecConstId};

use crate::literal::{Literal, NumberLiteral};
use crate::types::TypeSpecifier;

use super::HirBuildError;
use super::context::BuildContext;

impl BuildContext<'_> {
    pub(super) fn type_specifier(
        &mut self,
        specifier: &TypeSpecifier,
    ) -> Result<(TypeId, TypeSpecConstId), HirBuildError> {
        let literal = self.literal(specifier.form())?;
        let constant = self.form.add_type_spec_constant(literal)?;
        let ty = self.type_from_literal(specifier.form(), constant)?;
        Ok((ty, constant))
    }

    pub(super) fn type_from_literal(
        &mut self,
        specifier: &Literal,
        source_constant: TypeSpecConstId,
    ) -> Result<TypeId, HirBuildError> {
        if let Literal::Symbol(symbol) = specifier {
            if symbol.is_common_lisp("T") {
                return Ok(self.form.types_mut().any());
            }
            if symbol.is_common_lisp("NIL") {
                return Ok(self.form.types_mut().empty());
            }
            if symbol.is_common_lisp(&symbol.name)
                && let Some(builtin) = BuiltinType::from_name(&symbol.name)
            {
                return self
                    .form
                    .types_mut()
                    .named(builtin)
                    .map_err(HirBuildError::Type);
            }
            return self.opaque_type(symbol, source_constant);
        }

        let Some(elements) = specifier.list_elements() else {
            return Err(HirBuildError::Unrepresentable(
                super::UnsupportedHirForm::TypeSpecifier,
            ));
        };
        let Some((Literal::Symbol(operator), operands)) = elements.split_first() else {
            return Err(HirBuildError::Unrepresentable(
                super::UnsupportedHirForm::TypeSpecifier,
            ));
        };

        if !operator.is_common_lisp(&operator.name) {
            return self.opaque_type(operator, source_constant);
        }
        let name = operator.name.to_ascii_uppercase();
        match name.as_str() {
            "OR" => {
                let types = operands
                    .iter()
                    .map(|item| {
                        let literal = self.literal(item)?;
                        let id = self.form.add_type_spec_constant(literal)?;
                        self.type_from_literal(item, id)
                    })
                    .collect::<Result<Vec<_>, HirBuildError>>()?;
                self.form
                    .types_mut()
                    .union(types)
                    .map_err(HirBuildError::Type)
            }
            "AND" => {
                let types = operands
                    .iter()
                    .map(|item| {
                        let literal = self.literal(item)?;
                        let id = self.form.add_type_spec_constant(literal)?;
                        self.type_from_literal(item, id)
                    })
                    .collect::<Result<Vec<_>, HirBuildError>>()?;
                self.form
                    .types_mut()
                    .intersection(types)
                    .map_err(HirBuildError::Type)
            }
            "NOT" => {
                let [operand] = operands else {
                    return self.opaque_type(operator, source_constant);
                };
                let literal = self.literal(operand)?;
                let constant = self.form.add_type_spec_constant(literal)?;
                let ty = self.type_from_literal(operand, constant)?;
                self.form
                    .types_mut()
                    .complement(ty)
                    .map_err(HirBuildError::Type)
            }
            "MEMBER" => self.member_type(operands, operator, source_constant),
            "EQL" if operands.len() == 1 => self.member_type(operands, operator, source_constant),
            "INTEGER" => self.integer_type(operands, operator, source_constant),
            "CONS" if operands.len() <= 2 => {
                let mut ids = [self.form.types_mut().any(); 2];
                for (slot, operand) in ids.iter_mut().zip(operands) {
                    let literal = self.literal(operand)?;
                    let constant = self.form.add_type_spec_constant(literal)?;
                    *slot = self.type_from_literal(operand, constant)?;
                }
                let [car, cdr] = ids;
                self.form
                    .types_mut()
                    .cons(car, cdr)
                    .map_err(HirBuildError::Type)
            }
            _ => self.opaque_type(operator, source_constant), // check-added-lines: allow(wildcard) unmodeled operators retain opaque specifiers
        }
    }

    fn member_type(
        &mut self,
        operands: &[&Literal],
        operator: &crate::symbols::SymbolRef,
        source_constant: TypeSpecConstId,
    ) -> Result<TypeId, HirBuildError> {
        let mut values = Vec::with_capacity(operands.len());
        for operand in operands {
            let Some(value) = self.member_value(operand)? else {
                return self.opaque_type(operator, source_constant);
            };
            values.push(value);
        }
        self.form
            .types_mut()
            .member(values)
            .map_err(HirBuildError::Type)
    }

    fn member_value(&mut self, value: &Literal) -> Result<Option<MemberValue>, HirBuildError> {
        Ok(Some(match value {
            Literal::Nil => MemberValue::Nil,
            Literal::T => MemberValue::True,
            Literal::Symbol(symbol) if symbol.is_common_lisp("NIL") => MemberValue::Nil,
            Literal::Symbol(symbol) if symbol.is_common_lisp("T") => MemberValue::True,
            Literal::Symbol(symbol) => MemberValue::Symbol(self.intern(symbol)?),
            Literal::Number(NumberLiteral::Fixnum(value)) => MemberValue::Integer(*value),
            Literal::Number(NumberLiteral::SingleFloat(value)) => {
                MemberValue::SingleFloat(value.to_bits())
            }
            Literal::Number(NumberLiteral::DoubleFloat(value)) => {
                MemberValue::DoubleFloat(value.to_bits())
            }
            Literal::Character(value) => {
                let Some(value) = char::from_u32(*value) else {
                    return Ok(None);
                };
                MemberValue::Character(value)
            }
            _ => return Ok(None), // check-added-lines: allow(wildcard) nonmember literals use the opaque fallback
        }))
    }

    fn integer_type(
        &mut self,
        operands: &[&Literal],
        operator: &crate::symbols::SymbolRef,
        source_constant: TypeSpecConstId,
    ) -> Result<TypeId, HirBuildError> {
        if operands.is_empty() {
            return self
                .form
                .types_mut()
                .named(BuiltinType::Integer)
                .map_err(HirBuildError::Type);
        }
        let [lower_operand, upper_operand] = operands else {
            return self.opaque_type(operator, source_constant);
        };
        let Some(lower) = integer_bound(lower_operand) else {
            return self.opaque_type(operator, source_constant);
        };
        let Some(upper) = integer_bound(upper_operand) else {
            return self.opaque_type(operator, source_constant);
        };
        let range = ncl_hir::cl_type::numeric::NumericRange::new(
            NumericDomain::Integer,
            lower.into_numeric_bound(),
            upper.into_numeric_bound(),
        );
        match range {
            Ok(range) => self
                .form
                .types_mut()
                .numeric_range(range)
                .map_err(HirBuildError::Type),
            Err(_) => self.opaque_type(operator, source_constant),
        }
    }

    fn opaque_type(
        &mut self,
        specifier: &crate::symbols::SymbolRef,
        source_constant: TypeSpecConstId,
    ) -> Result<TypeId, HirBuildError> {
        let symbol = self.intern(specifier)?;
        self.form
            .types_mut()
            .opaque(symbol, [OpaqueOperand::Constant(source_constant)])
            .map_err(HirBuildError::Type)
    }
}

enum IntegerBound {
    Unbounded,
    Inclusive(NumericBound),
}

impl IntegerBound {
    const fn into_numeric_bound(self) -> Option<NumericBound> {
        match self {
            Self::Unbounded => None,
            Self::Inclusive(bound) => Some(bound),
        }
    }
}

fn integer_bound(literal: &Literal) -> Option<IntegerBound> {
    if matches!(literal, Literal::Symbol(symbol) if symbol.is_common_lisp("*")) {
        return Some(IntegerBound::Unbounded);
    }
    let Literal::Number(NumberLiteral::Fixnum(value)) = literal else {
        return None;
    };
    let value = NumericValue::Integer(*value);
    Some(IntegerBound::Inclusive(NumericBound::inclusive(value)))
}
