#![allow(clippy::wildcard_imports)]
use super::*;
use crate::environment::special_form_name;

impl Runtime {
    pub(super) fn eval_special_form_mutation(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match special_form_name(name).unwrap_or(name) {
            "MACROLET" => Some(self.special_macrolet(items, environment)?),
            "SYMBOL-MACROLET" => Some(self.special_symbol_macrolet(items, environment)?),
            "DEFPACKAGE" => Some(self.special_defpackage(items)?),
            "IN-PACKAGE" => Some(self.special_in_package(items)?),
            "DEFINE" => Some(self.special_define(items, environment)?),
            "DEFINE-SYMBOL-MACRO" => Some(Self::special_define_symbol_macro(items, environment)?),
            "SETQ" => Some(self.special_setq(items, environment)?),
            "PSETQ" => Some(self.special_psetq(items, environment)?),
            "MULTIPLE-VALUE-SETQ" => Some(self.special_multiple_value_setq(items, environment)?),
            "SETF" => Some(self.special_setf(items, environment)?),
            "PSETF" => Some(self.special_psetf(items, environment)?),
            "PUSH" => Some(self.special_push(items, environment)?),
            "POP" => Some(self.special_pop(items, environment)?),
            "PUSHNEW" => Some(self.special_pushnew(items, environment)?),
            "ROTATEF" => Some(self.special_rotatef(items, environment)?),
            "SHIFTF" => Some(self.special_shiftf(items, environment)?),
            "INCF" => Some(self.special_modify_symbol(items, environment, "INCF", "+")?),
            "DECF" => Some(self.special_modify_symbol(items, environment, "DECF", "-")?),
            "REMF" => Some(self.special_remf(items, environment)?),
            "DEFSTRUCT" => Some(self.special_defstruct(items, environment)?),
            "DEFCLASS" => Some(Self::special_defclass(items, environment)?),
            "DEFINE-CONDITION" => Some(Self::special_define_condition(items, environment)?),
            "DEFGENERIC" => Some(Self::special_defgeneric(items, environment)?),
            "DEFMETHOD" => Some(Self::special_defmethod(items, environment)?),
            "DEFSETF" => Some(self.special_defsetf(items, environment)?),
            "DEFINE-SETF-EXPANDER" => Some(Self::special_define_setf_expander(items, environment)?),
            "GET-SETF-EXPANSION" => Some(self.special_get_setf_expansion(items, environment)?),
            "DEFVAR" => Some(self.special_defvar(items, environment, false)?),
            "DEFPARAMETER" => Some(self.special_defvar(items, environment, true)?),
            "DEFCONSTANT" => Some(self.special_defconstant(items, environment)?),
            "EVAL" => Some(self.special_eval(items, environment)?),
            "FUNCALL" => Some(self.special_funcall(items, environment)?),
            "APPLY" => Some(self.special_apply(items, environment)?),
            "MAP-INTO" => Some(self.special_map_into(items, environment)?),
            "MAPCAR" => Some(self.special_mapcar(items, environment)?),
            "MAPHASH" => Some(self.special_maphash(items, environment)?),
            _ => None,
        };
        Ok(value)
    }

    pub(super) fn eval_special_form_expansion(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match special_form_name(name).unwrap_or(name) {
            "MACROEXPAND-1" => Some(self.special_macroexpand_1(items, environment)?),
            "MACROEXPAND" => Some(self.special_macroexpand(items, environment)?),
            _ => None,
        };
        Ok(value)
    }

    pub(super) fn eval_special_form_macros(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match special_form_name(name).unwrap_or(name) {
            "DEFMACRO" => Some(self.special_defmacro(items, environment)?),
            "DEFINE-MODIFY-MACRO" => Some(self.special_define_modify_macro(items, environment)?),
            _ => None,
        };
        Ok(value)
    }

    pub(super) fn eval_special_form_functions(
        &self,
        items: &[Form],
        name: &str,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        let value = match special_form_name(name).unwrap_or(name) {
            "LAMBDA" => Some(Self::special_lambda(items, environment)?),
            "FUNCTION" => Some(self.special_function(items, environment)?),
            "DEFUN" => Some(self.special_defun(items, environment)?),
            _ => None,
        };
        Ok(value)
    }
}
