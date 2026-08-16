use super::{Environment, normalize_name};

impl Environment {
    pub(crate) fn define_block(&self, name: impl AsRef<str>, target: u64) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().block_targets.insert(key, target);
    }

    pub(crate) fn lookup_block(&self, name: &str) -> Option<u64> {
        let key = normalize_name(name);
        let (target, parent) = {
            let frame = self.0.borrow();
            (frame.block_targets.get(&key).copied(), frame.parent.clone())
        };
        target.or_else(|| parent.and_then(|environment| environment.lookup_block(name)))
    }

    pub(crate) fn define_tag(&self, name: impl AsRef<str>, target: u64) {
        let key = normalize_name(name.as_ref());
        self.0.borrow_mut().tag_targets.insert(key, target);
    }

    pub(crate) fn lookup_tag(&self, name: &str) -> Option<u64> {
        let key = normalize_name(name);
        let (target, parent) = {
            let frame = self.0.borrow();
            (frame.tag_targets.get(&key).copied(), frame.parent.clone())
        };
        target.or_else(|| parent.and_then(|environment| environment.lookup_tag(name)))
    }
}
