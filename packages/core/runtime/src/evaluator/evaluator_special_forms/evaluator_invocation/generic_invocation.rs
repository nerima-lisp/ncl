use super::{
    Environment, MethodContext, MethodContinuation, MethodDefinition, Runtime, RuntimeError, Span,
    Value,
};

impl Runtime {
    pub(super) fn invoke_method(
        &self,
        method: &MethodDefinition,
        arguments: &[Value],
        next: Option<MethodContinuation>,
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.method_context.borrow_mut().push(MethodContext {
            arguments: arguments.to_vec(),
            next,
        });
        let result = self.apply_in(&method.function, arguments, span, environment);
        self.method_context.borrow_mut().pop();
        result
    }

    fn invoke_core(
        &self,
        before: &[MethodDefinition],
        primary: &[MethodDefinition],
        after: &[MethodDefinition],
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        for method in before {
            self.invoke_method(method, arguments, None, span, environment)?;
        }
        let Some(method) = primary.first() else {
            return Err(Self::invalid("no primary method is applicable", span));
        };
        let next = (primary.len() > 1).then(|| MethodContinuation::Chain {
            methods: primary.to_vec(),
            index: 1,
            fallback: None,
        });
        let result = self.invoke_method(method, arguments, next, span, environment)?;
        for method in after {
            self.invoke_method(method, arguments, None, span, environment)?;
        }
        Ok(result)
    }

    pub(crate) fn invoke_continuation(
        &self,
        continuation: MethodContinuation,
        arguments: &[Value],
        span: Span,
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        match continuation {
            MethodContinuation::Chain {
                methods,
                index,
                fallback,
            } => {
                if index < methods.len() {
                    let method = methods[index].clone();
                    let next = if index + 1 < methods.len() || fallback.is_some() {
                        Some(MethodContinuation::Chain {
                            methods,
                            index: index + 1,
                            fallback,
                        })
                    } else {
                        None
                    };
                    self.invoke_method(&method, arguments, next, span, environment)
                } else if let Some(fallback) = fallback {
                    self.invoke_continuation(*fallback, arguments, span, environment)
                } else {
                    Err(Self::invalid("no next method is applicable", span))
                }
            }
            MethodContinuation::Core {
                before,
                primary,
                after,
            } => self.invoke_core(&before, &primary, &after, arguments, span, environment),
        }
    }
}
