use crate::environment::{Environment, intern_name};

impl Environment {
    pub(crate) fn define_block(&self, name: impl AsRef<str>, target: u64) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().block_targets.insert(key, target);
    }

    pub(crate) fn lookup_block(&self, name: &str) -> Option<u64> {
        let key = intern_name(name);
        let (target, parent) = {
            let frame = self.0.borrow();
            (frame.block_targets.get(&key).copied(), frame.parent.clone())
        };
        target.or_else(|| parent.and_then(|environment| environment.lookup_block(name)))
    }

    pub(crate) fn define_tag(&self, name: impl AsRef<str>, target: u64) {
        let key = intern_name(name.as_ref());
        self.0.borrow_mut().tag_targets.insert(key, target);
    }

    pub(crate) fn lookup_tag(&self, name: &str) -> Option<u64> {
        let key = intern_name(name);
        let (target, parent) = {
            let frame = self.0.borrow();
            (frame.tag_targets.get(&key).copied(), frame.parent.clone())
        };
        target.or_else(|| parent.and_then(|environment| environment.lookup_tag(name)))
    }
}

#[cfg(test)]
mod tests {
    use crate::environment::{Environment, normalize_name};

    #[test]
    fn block_and_tag_targets_resolve_through_parent_scope() {
        let root = Environment::new();
        let child = root.child();

        root.define_block("done", 11);
        root.define_tag("again", 22);
        assert_eq!(child.lookup_block("DONE"), Some(11));
        assert_eq!(child.lookup_tag("AGAIN"), Some(22));
        assert_eq!(normalize_name("MiXeD"), "MIXED");
    }
}
