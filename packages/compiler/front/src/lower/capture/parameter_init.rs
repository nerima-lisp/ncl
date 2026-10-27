use super::{FreeNames, add_parameter_binding, collect_free};
use crate::ast::LambdaExpr;

pub(super) fn collect_parameter_init_free(lambda: &LambdaExpr, free: &mut FreeNames) {
    let list = &lambda.lambda_list;
    let mut bound_variables = Vec::new();
    let bound_functions = Vec::new();
    for name in &list.required {
        add_parameter_binding(name, &mut bound_variables);
    }
    for optional in &list.optional {
        if let Some(form) = &optional.default {
            collect_free(form, &mut bound_variables, &bound_functions, free);
        }
        add_parameter_binding(&optional.name, &mut bound_variables);
        if let Some(supplied_p) = &optional.supplied_p {
            add_parameter_binding(supplied_p, &mut bound_variables);
        }
    }
    if let Some(rest) = &list.rest {
        add_parameter_binding(rest, &mut bound_variables);
    }
    for key in &list.keys {
        if let Some(form) = &key.default {
            collect_free(form, &mut bound_variables, &bound_functions, free);
        }
        add_parameter_binding(&key.name, &mut bound_variables);
        if let Some(supplied_p) = &key.supplied_p {
            add_parameter_binding(supplied_p, &mut bound_variables);
        }
    }
    for aux in &list.aux {
        if let Some(form) = &aux.default {
            collect_free(form, &mut bound_variables, &bound_functions, free);
        }
        add_parameter_binding(&aux.name, &mut bound_variables);
    }
}
