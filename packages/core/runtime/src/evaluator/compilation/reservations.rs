use super::{Form, FormKind, Runtime, resolved_symbol};

pub(super) struct CompilationReservations<'a> {
    runtime: &'a Runtime,
    added: Vec<String>,
}

impl<'a> CompilationReservations<'a> {
    pub(super) const fn new(runtime: &'a Runtime) -> Self {
        Self {
            runtime,
            added: Vec::new(),
        }
    }

    pub(super) fn reserve(&mut self, form: &Form) {
        match &form.kind {
            FormKind::Atom(atom) => {
                let (name, _) = resolved_symbol(atom);
                let name = name.to_ascii_uppercase();
                if self
                    .runtime
                    .compilation_reserved_names
                    .borrow_mut()
                    .insert(name.clone())
                {
                    self.added.push(name);
                }
            }
            FormKind::List(items) | FormKind::Vector(items) => {
                for item in items {
                    self.reserve(item);
                }
            }
            FormKind::DottedList { items, tail } => {
                for item in items {
                    self.reserve(item);
                }
                self.reserve(tail);
            }
            FormKind::Complex { real, imaginary } => {
                self.reserve(real);
                self.reserve(imaginary);
            }
            FormKind::String(_)
            | FormKind::Character(_)
            | FormKind::Literal(_)
            | FormKind::CircularReference => {}
        }
    }
}

impl Drop for CompilationReservations<'_> {
    fn drop(&mut self) {
        let mut names = self.runtime.compilation_reserved_names.borrow_mut();
        for name in &self.added {
            names.remove(name);
        }
    }
}
