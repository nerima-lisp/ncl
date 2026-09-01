use ncl_syntax::{Form, FormKind};

use crate::{Runtime, RuntimeError};

impl Runtime {
    pub(super) fn expand_builtin_loop(form: &Form) -> Result<Form, RuntimeError> {
        let FormKind::List(items) = &form.kind else {
            return Ok(form.clone());
        };
        let tag = Form::atom(format!("#:NCL-LOOP-{}", form.span.start), form.span);
        let mut tagbody = vec![Form::atom("TAGBODY", form.span), tag.clone()];
        tagbody.extend(items[1..].iter().cloned());
        tagbody.push(Form::list(
            vec![Form::atom("GO", form.span), tag],
            form.span,
        ));

        Ok(Form::list(
            vec![
                Form::atom("BLOCK", form.span),
                Form::atom("NIL", form.span),
                Form::list(tagbody, form.span),
            ],
            form.span,
        ))
    }
}
