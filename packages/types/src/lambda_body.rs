//! Shared parsing rules for the declaration and documentation prefix of a lambda body.

/// The parser's classification of one leading lambda body form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LambdaBodyElement {
    /// A `(declare ...)` form.
    Declaration,
    /// A string object that may be the lambda body's documentation string.
    String,
    /// Any other executable form.
    Form,
}

/// The declaration and documentation prefix of a lambda body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LambdaBodyPrefix {
    prefix_len: usize,
    docstring_index: Option<usize>,
}

impl LambdaBodyPrefix {
    /// Number of forms to preserve before the executable body.
    #[must_use]
    pub const fn len(self) -> usize {
        self.prefix_len
    }

    /// Whether the body has no leading declarations or documentation string.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.prefix_len == 0
    }

    /// Index of the documentation string within the original body, if present.
    #[must_use]
    pub const fn docstring_index(self) -> Option<usize> {
        self.docstring_index
    }
}

/// Split leading declarations and an optional documentation string from a body.
///
/// Declarations may appear before or after the documentation string. A string
/// with no following body item remains an executable form.
#[must_use]
pub fn split_lambda_body_prefix(elements: &[LambdaBodyElement]) -> LambdaBodyPrefix {
    let mut prefix_len = 0;
    let mut docstring_index = None;

    for (index, element) in elements.iter().enumerate() {
        match element {
            LambdaBodyElement::Declaration => prefix_len += 1,
            LambdaBodyElement::String
                if docstring_index.is_none() && index + 1 < elements.len() =>
            {
                docstring_index = Some(index);
                prefix_len += 1;
            }
            LambdaBodyElement::String | LambdaBodyElement::Form => break,
        }
    }

    LambdaBodyPrefix {
        prefix_len,
        docstring_index,
    }
}

#[cfg(test)]
mod tests {
    use super::{LambdaBodyElement, split_lambda_body_prefix};

    #[test]
    fn a_terminal_string_remains_an_executable_form() {
        let prefix = split_lambda_body_prefix(&[LambdaBodyElement::String]);

        assert_eq!(prefix.len(), 0);
        assert!(prefix.is_empty());
        assert_eq!(prefix.docstring_index(), None);
    }

    #[test]
    fn declarations_can_follow_the_documentation_string() {
        let prefix = split_lambda_body_prefix(&[
            LambdaBodyElement::String,
            LambdaBodyElement::Declaration,
            LambdaBodyElement::Form,
        ]);

        assert_eq!(prefix.len(), 2);
        assert!(!prefix.is_empty());
        assert_eq!(prefix.docstring_index(), Some(0));
    }

    #[test]
    fn documentation_can_follow_declarations() {
        let prefix = split_lambda_body_prefix(&[
            LambdaBodyElement::Declaration,
            LambdaBodyElement::String,
            LambdaBodyElement::Form,
        ]);

        assert_eq!(prefix.len(), 2);
        assert_eq!(prefix.docstring_index(), Some(1));
    }

    #[test]
    fn a_terminal_string_after_declarations_remains_an_executable_form() {
        let prefix =
            split_lambda_body_prefix(&[LambdaBodyElement::Declaration, LambdaBodyElement::String]);

        assert_eq!(prefix.len(), 1);
        assert_eq!(prefix.docstring_index(), None);
    }

    #[test]
    fn a_documentation_string_followed_by_a_declaration_is_preserved() {
        let prefix =
            split_lambda_body_prefix(&[LambdaBodyElement::String, LambdaBodyElement::Declaration]);

        assert_eq!(prefix.len(), 2);
        assert_eq!(prefix.docstring_index(), Some(0));
    }
}
