#![allow(clippy::wildcard_imports)]
use super::*;

impl Runtime {
    pub(super) fn apply_sequence_search_primitive(
        &self,
        name: &str,
        arguments: &[Value],
        environment: &Environment,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        Some(match name {
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN" if arguments.len() >= 2 => self
                .apply_list_membership(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                ),
            "ASSOC" | "ASSOC-IF" | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT"
                if arguments.len() >= 2 =>
            {
                self.apply_association_search(
                    name,
                    &arguments[0],
                    &arguments[1],
                    &arguments[2..],
                    environment,
                    span,
                )
            }
            "FIND" | "POSITION" | "COUNT" if arguments.len() >= 2 => self.apply_sequence_search(
                name,
                &arguments[0],
                &arguments[1],
                &arguments[2..],
                environment,
                span,
            ),
            "SEARCH" | "MISMATCH" if arguments.len() >= 2 => self.apply_sequence_pair_search(
                name,
                &arguments[0],
                &arguments[1],
                &arguments[2..],
                environment,
                span,
            ),
            "SORT" | "STABLE-SORT" if arguments.len() >= 2 => self.apply_sequence_sort(
                name,
                &arguments[0],
                &arguments[1],
                &arguments[2..],
                environment,
                span,
            ),
            "EVERY" | "SOME" | "NOTANY" | "NOTEVERY" if arguments.len() >= 2 => self
                .apply_sequence_quantifier(name, &arguments[0], &arguments[1..], environment, span),
            "MAPCAR" | "MAPC" | "MAPL" | "MAPLIST" | "MAPCAN" | "MAPCON"
                if arguments.len() >= 2 =>
            {
                self.apply_list_mapping(name, &arguments[0], &arguments[1..], environment, span)
            }
            "MEMBER" | "MEMBER-IF" | "MEMBER-IF-NOT" | "ADJOIN" | "ASSOC" | "ASSOC-IF"
            | "ASSOC-IF-NOT" | "RASSOC" | "RASSOC-IF" | "RASSOC-IF-NOT" | "FIND" | "POSITION"
            | "COUNT" | "SEARCH" | "MISMATCH" | "SORT" | "STABLE-SORT" | "EVERY" | "SOME"
            | "NOTANY" | "NOTEVERY" | "MAPCAR" | "MAPC" | "MAPL" | "MAPLIST" | "MAPCAN"
            | "MAPCON" => Err(Self::arity(
                &name.to_ascii_lowercase(),
                "at least two",
                arguments.len(),
            )),
            _ => return None,
        })
    }
}
