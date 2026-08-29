use ncl_syntax::Form;

use crate::evaluator::helpers::control_tag;
use crate::{Environment, Runtime, RuntimeError, Value};

impl Runtime {
    pub(crate) fn special_tagbody(
        &self,
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        self.eval_tagbody_forms(&items[1..], environment)
    }

    pub(in crate::evaluator::evaluator_special_forms) fn eval_tagbody_forms(
        &self,
        forms: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        let mut tags: Vec<(String, usize)> = Vec::new();
        for (position, item) in forms.iter().enumerate() {
            if let Some(tag) = control_tag(item) {
                if tags.iter().any(|(known_tag, _)| known_tag == &tag) {
                    return Err(Self::invalid("tagbody contains duplicate tag", item.span));
                }
                tags.push((tag, position));
            }
        }

        let target = self.fresh_block_target();
        let tag_environment = environment.child();
        for (tag, _) in &tags {
            tag_environment.define_tag(tag, target);
        }

        let mut position = 0;
        while position < forms.len() {
            let item = &forms[position];
            if control_tag(item).is_some() {
                position += 1;
                continue;
            }
            match self.eval_values_in(item, &tag_environment) {
                Ok(_) => position += 1,
                Err(RuntimeError::Go {
                    tag,
                    target: Some(go_target),
                    ..
                }) if go_target == target => {
                    position = tags
                        .iter()
                        .find(|(known_tag, _)| known_tag == &tag)
                        .map(|(_, tag_position)| *tag_position)
                        .ok_or_else(|| {
                            Self::invalid("GO target is missing from TAGBODY", item.span)
                        })?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(Value::Nil)
    }

    pub(crate) fn special_go(
        items: &[Form],
        environment: &Environment,
    ) -> Result<Value, RuntimeError> {
        if items.len() != 2 {
            return Err(Self::arity("go", "one", items.len().saturating_sub(1)));
        }
        let tag = control_tag(&items[1])
            .ok_or_else(|| Self::invalid("go tag must be a symbol or integer", items[1].span))?;
        Err(RuntimeError::Go {
            target: environment.lookup_tag(&tag),
            tag,
            span: Some(items[1].span),
        })
    }
}
