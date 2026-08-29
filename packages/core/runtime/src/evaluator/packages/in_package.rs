use ncl_syntax::Form;

use crate::{Runtime, RuntimeError, Value};

impl Runtime {
    pub(in crate::evaluator) fn special_in_package(
        &self,
        items: &[Form],
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity(
                "in-package",
                "one",
                items.len().saturating_sub(1),
            ));
        }
        let name = Self::package_name_from_form(&items[1])?;
        let mut packages = self.packages.borrow_mut();
        if !packages.package_exists(&name) {
            return Err(Self::package_error(
                &format!("unknown package {name}"),
                items[1].span,
            ));
        }
        let canonical_name = packages.canonical_package_name(&name);
        packages.set_current(&canonical_name);
        Ok(Value::package(&canonical_name))
    }
}

#[cfg(test)]
mod tests {
    use ncl_syntax::{Form, Span};

    use crate::Runtime;

    const SPAN: Span = Span::new(0, 1);

    #[test]
    fn in_package_rejects_wrong_arity_and_unknown_packages() {
        let runtime = Runtime::new();
        let name = Form::atom("IN-PACKAGE", SPAN);

        assert!(
            runtime
                .special_in_package(std::slice::from_ref(&name))
                .is_err()
        );
        assert!(
            runtime
                .special_in_package(&[name.clone(), Form::atom("A", SPAN), Form::atom("B", SPAN)])
                .is_err()
        );
        assert!(
            runtime
                .special_in_package(&[name, Form::atom("NO-SUCH-PACKAGE", SPAN)])
                .is_err()
        );
    }
}
