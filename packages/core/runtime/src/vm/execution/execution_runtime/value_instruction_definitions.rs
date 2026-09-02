use ncl_compiler::Instruction;
use ncl_syntax::FormKind;

use crate::{Environment, Runtime, RuntimeError, Value};

pub(super) fn execute_definition_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
) -> Option<Result<bool, RuntimeError>> {
    let (form, operation) = match instruction {
        Instruction::Defstruct(form) => (form, "DEFSTRUCT"),
        Instruction::Deftype(form) => (form, "DEFTYPE"),
        Instruction::Defclass(form) => (form, "DEFCLASS"),
        Instruction::DefineCondition(form) => (form, "DEFINE-CONDITION"),
        Instruction::Defgeneric(form) => (form, "DEFGENERIC"),
        Instruction::Defmethod(form) => (form, "DEFMETHOD"),
        Instruction::Defsetf(form) => (form, "DEFSETF"),
        Instruction::Defconstant(form) => (form, "DEFCONSTANT"),
        Instruction::DefineSymbolMacro(form) => (form, "DEFINE-SYMBOL-MACRO"),
        Instruction::DefineModifyMacro(form) => (form, "DEFINE-MODIFY-MACRO"),
        Instruction::DefineSetfExpander(form) => (form, "DEFINE-SETF-EXPANDER"),
        Instruction::GetSetfExpansion(form) => (form, "GET-SETF-EXPANSION"),
        Instruction::Psetf(form) => (form, "PSETF"),
        Instruction::LoadTimeValue(form) => (form, "LOAD-TIME-VALUE"),
        _ => return None,
    };
    let FormKind::List(items) = &form.kind else {
        return Some(Err(RuntimeError::InvalidForm {
            message: format!("{operation} instruction requires a list"),
            span: Some(form.span),
        }));
    };
    let value = match operation {
        "DEFSTRUCT" => runtime.special_defstruct(items, environment),
        "DEFTYPE" => runtime.special_deftype(items, environment),
        "DEFCLASS" => Runtime::special_defclass(items, environment),
        "DEFINE-CONDITION" => Runtime::special_define_condition(items, environment),
        "DEFGENERIC" => Runtime::special_defgeneric(items, environment),
        "DEFMETHOD" => runtime.special_defmethod(items, environment),
        "DEFSETF" => runtime.special_defsetf(items, environment),
        "DEFCONSTANT" => runtime.special_defconstant(items, environment),
        "DEFINE-SYMBOL-MACRO" => Runtime::special_define_symbol_macro(items, environment),
        "DEFINE-MODIFY-MACRO" => runtime.special_define_modify_macro(items, environment),
        "DEFINE-SETF-EXPANDER" => Runtime::special_define_setf_expander(items, environment),
        "GET-SETF-EXPANSION" => runtime.special_get_setf_expansion(items, environment),
        "PSETF" => runtime.special_psetf(items, environment),
        "LOAD-TIME-VALUE" => runtime.special_load_time_value(items, environment),
        _ => unreachable!(),
    };
    Some(value.map(|value| {
        stack.push(value);
        true
    }))
}
