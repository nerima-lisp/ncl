#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(super) fn compile_destructuring_lambda_list(
        &mut self,
        form: &Form,
    ) -> Result<DestructureLambdaList, CompileError> {
        let FormKind::List(parameters) = &form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "destructuring lambda list".to_string(),
                },
                form.span,
            ));
        };
        let mut lambda_list = DestructureLambdaList {
            whole: None,
            required: Vec::new(),
            optional: Vec::new(),
            keywords: Vec::new(),
            has_keyword_section: false,
            allow_other_keys: false,
            rest: None,
            auxiliary: Vec::new(),
        };
        let mut seen = HashSet::new();
        let mut section = DestructureLambdaListSection::Required;
        let mut index = 0;
        while index < parameters.len() {
            let parameter = &parameters[index];
            if let FormKind::Atom(name) = &parameter.kind {
                let marker = normalize_name(name);
                if marker.starts_with('&') {
                    index = Self::compile_destructuring_apply_marker(
                        &marker,
                        parameter,
                        parameters,
                        index,
                        &mut lambda_list,
                        &mut seen,
                        &mut section,
                    )?;
                    continue;
                }
            }
            self.compile_destructuring_regular_parameter(
                parameter,
                section,
                &mut lambda_list,
                &mut seen,
            )?;
            index += 1;
        }

        Ok(lambda_list)
    }
}
