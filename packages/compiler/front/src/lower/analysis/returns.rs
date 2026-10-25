use crate::ast::{Expr, FunctionDesignator, Operator, TagbodyItem};
use crate::symbols::SymbolRef;

pub(super) fn contains_return(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::ReturnFrom {
            name: target,
            value,
        } => {
            target == name
                || value
                    .as_deref()
                    .is_some_and(|form| contains_return(form, name))
        }
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            forms.iter().any(|form| contains_return(form, name))
        }
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .filter_map(|binding| binding.value.as_ref())
                .any(|form| contains_return(form, name))
                || body.iter().any(|form| contains_return(form, name))
        }
        Expr::UnwindProtect { protected, cleanup } => {
            contains_return(protected, name)
                || cleanup.iter().any(|form| contains_return(form, name))
        }
        Expr::Catch { tag, body } => {
            contains_return(tag, name) || body.iter().any(|form| contains_return(form, name))
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            contains_return(symbols, name)
                || contains_return(values, name)
                || body.iter().any(|form| contains_return(form, name))
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            contains_return(test, name)
                || contains_return(then, name)
                || otherwise
                    .as_deref()
                    .is_some_and(|form| contains_return(form, name))
        }
        Expr::Block { body, .. } => body.iter().any(|form| contains_return(form, name)),
        Expr::Tagbody(items) => items.iter().any(|item| match item {
            TagbodyItem::Tag(_) => false,
            TagbodyItem::Form(form) => contains_return(form, name),
        }),
        Expr::Call {
            operator,
            arguments,
        } => {
            let in_lambda = matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(|form| contains_return(form, name)));
            in_lambda || arguments.iter().any(|form| contains_return(form, name))
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => {
            nested_return(function, name)
                || contains_return(function, name)
                || arguments.iter().any(|form| contains_return(form, name))
        }
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Lambda(_)
        | Expr::Function(_)
        | Expr::Go { .. }
        | Expr::Throw { .. }
        | Expr::Setq(_)
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. }
        | Expr::Flet { .. }
        | Expr::Labels { .. } => false,
    }
}
pub(super) fn nested_return(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::Lambda(lambda) => lambda.body.iter().any(|form| contains_return(form, name)),
        Expr::Call {
            operator,
            arguments,
        } => {
            matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(|form| contains_return(form, name)))
                || arguments.iter().any(|form| nested_return(form, name))
        }
        Expr::Function(FunctionDesignator::Lambda(lambda)) => {
            lambda.body.iter().any(|form| contains_return(form, name))
        }
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .filter_map(|binding| binding.value.as_ref())
                .any(|form| nested_return(form, name))
                || body.iter().any(|form| nested_return(form, name))
        }
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            forms.iter().any(|form| nested_return(form, name))
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            nested_return(test, name)
                || nested_return(then, name)
                || otherwise
                    .as_deref()
                    .is_some_and(|form| nested_return(form, name))
        }
        Expr::Block { body, .. } => body.iter().any(|form| nested_return(form, name)),
        Expr::Catch { tag, body } => {
            nested_return(tag, name) || body.iter().any(|form| nested_return(form, name))
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            nested_return(symbols, name)
                || nested_return(values, name)
                || body.iter().any(|form| nested_return(form, name))
        }
        Expr::Tagbody(items) => items.iter().any(|item| match item {
            TagbodyItem::Tag(_) => false,
            TagbodyItem::Form(form) => nested_return(form, name),
        }),
        Expr::Setq(bindings) => bindings.iter().any(|(_, form)| nested_return(form, name)),
        Expr::Flet {
            definitions, body, ..
        }
        | Expr::Labels {
            definitions, body, ..
        } => {
            definitions.iter().any(|definition| {
                definition
                    .lambda
                    .body
                    .iter()
                    .any(|form| contains_return(form, name) || nested_return(form, name))
            }) || body.iter().any(|form| nested_return(form, name))
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => {
            nested_return(function, name) || arguments.iter().any(|form| nested_return(form, name))
        }
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Function(_)
        | Expr::ReturnFrom { .. }
        | Expr::Go { .. }
        | Expr::Throw { .. }
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. } => false,
        Expr::UnwindProtect { protected, cleanup } => {
            nested_return(protected, name) || cleanup.iter().any(|form| nested_return(form, name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{contains_return, nested_return};
    use crate::ast::{Expr, LambdaExpr, Operator};
    use crate::lambda_list::LambdaList;
    use crate::symbols::SymbolRef;

    fn return_from(name: &SymbolRef) -> Expr {
        Expr::ReturnFrom {
            name: name.clone(),
            value: None,
        }
    }

    fn lambda_returning(name: &SymbolRef) -> Expr {
        Expr::Lambda(Box::new(LambdaExpr {
            lambda_list: LambdaList::new(),
            declarations: Vec::new(),
            docstring: None,
            body: vec![return_from(name)],
        }))
    }

    #[test]
    fn multiple_value_call_return_analysis_scans_function_and_arguments() {
        let target = SymbolRef::interned("COMMON-LISP", "NIL");
        let function_return = Expr::MultipleValueCall {
            function: Box::new(lambda_returning(&target)),
            arguments: Vec::new(),
        };
        assert!(nested_return(&function_return, &target));

        let argument_return = Expr::MultipleValueCall {
            function: Box::new(Expr::Variable(SymbolRef::interned(
                "COMMON-LISP-USER",
                "FUNCTION",
            ))),
            arguments: vec![lambda_returning(&target)],
        };
        assert!(nested_return(&argument_return, &target));

        let direct_return_in_function = Expr::MultipleValueCall {
            function: Box::new(Expr::Call {
                operator: Operator::Name(SymbolRef::interned("COMMON-LISP-USER", "FUNCTION")),
                arguments: vec![return_from(&target)],
            }),
            arguments: Vec::new(),
        };
        assert!(contains_return(&direct_return_in_function, &target));

        let direct_return_in_argument = Expr::MultipleValueCall {
            function: Box::new(Expr::Variable(SymbolRef::interned(
                "COMMON-LISP-USER",
                "FUNCTION",
            ))),
            arguments: vec![return_from(&target)],
        };
        assert!(contains_return(&direct_return_in_argument, &target));
    }
}
