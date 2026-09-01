#[allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(super) fn compile_native_remf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<Option<()>, CompileError> {
        if items.len() != 3 {
            return Ok(None);
        }
        if let FormKind::List(place_items) = &items[1].kind {
            if place_items.len() == 3
                && Self::symbol_name_info(&place_items[0], "REMF place operator")
                    .is_ok_and(|(name, _)| name == "GET")
            {
                self.compile_expression(function, &place_items[1])?;
                self.compile_expression(function, &place_items[2])?;
                self.compile_expression(function, &items[2])?;
                self.emit(function, Instruction::RemfGetDynamic, span)?;
                return Ok(Some(()));
            }
        }
        let Some((name, escaped)) = Self::symbol_name_info(&items[1], "REMF place").ok() else {
            return Ok(None);
        };
        self.compile_expression(function, &items[1])?;
        self.compile_expression(function, &items[2])?;
        self.compile_expression(function, &items[2])?;
        self.emit(function, Instruction::Remf { name, escaped }, span)?;
        Ok(Some(()))
    }

    pub(crate) fn compile_defsetf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() != 3 {
            return Err(Self::arity_error(items, "DEFSETF", "two", span));
        }
        self.emit(
            function,
            Instruction::Defsetf(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_define_setf_expander(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(
                items,
                "DEFINE-SETF-EXPANDER",
                "at least three",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::DefineSetfExpander(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_define_modify_macro(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(Self::arity_error(
                items,
                "DEFINE-MODIFY-MACRO",
                "at least three",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::DefineModifyMacro(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_get_setf_expansion(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if !(2..=3).contains(&items.len()) {
            return Err(Self::arity_error(
                items,
                "GET-SETF-EXPANSION",
                "one or two",
                span,
            ));
        }
        self.emit(
            function,
            Instruction::GetSetfExpansion(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }

    pub(crate) fn compile_psetf(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 || items.len().is_multiple_of(2) {
            return Err(Self::arity_error(
                items,
                "PSETF",
                "one or more place/value pairs",
                span,
            ));
        }
        if items.len() == 3 {
            return self.compile_setf(function, span, items);
        }
        let places = items[1..]
            .chunks_exact(2)
            .map(|pair| {
                if matches!(pair[0].kind, FormKind::Atom(_)) {
                    return Self::symbol_name_info(&pair[0], "PSETF place")
                        .ok()
                        .map(|(name, escaped)| crate::PsetfPlace::Symbol(name, escaped));
                }
                let mut accessors = Vec::new();
                let mut target = &pair[0];
                if let FormKind::List(place_items) = &pair[0].kind {
                    if place_items.len() == 2
                        && Self::symbol_name_info(&place_items[0], "PSETF place")
                            .is_ok_and(|(name, _)| name == "SYMBOL-PLIST")
                    {
                        return Some(crate::PsetfPlace::SymbolPlist);
                    }
                }
                while let Some((accessor, next_target)) =
                    crate::helpers::list_accessor_target(target)
                {
                    accessors.push(accessor);
                    target = next_target;
                }
                if accessors.is_empty() {
                    return None;
                }
                let (name, escaped) = Self::symbol_name_info(target, "PSETF list target").ok()?;
                accessors.reverse();
                Some(crate::PsetfPlace::List(accessors, name, escaped))
            })
            .collect::<Option<Vec<_>>>();
        if let Some(places) = places {
            for pair in items[1..].chunks_exact(2) {
                self.compile_expression(function, &pair[1])?;
            }
            for (pair, place) in items[1..].chunks_exact(2).zip(&places) {
                if matches!(place, crate::PsetfPlace::SymbolPlist) {
                    let FormKind::List(place_items) = &pair[0].kind else {
                        unreachable!("validated SYMBOL-PLIST place");
                    };
                    self.compile_expression(function, &place_items[1])?;
                }
            }
            if places
                .iter()
                .all(|place| matches!(place, crate::PsetfPlace::Symbol(_, _)))
            {
                let names = places
                    .into_iter()
                    .map(|place| match place {
                        crate::PsetfPlace::Symbol(name, escaped) => (name, escaped),
                        crate::PsetfPlace::List(..) | crate::PsetfPlace::SymbolPlist => {
                            unreachable!()
                        }
                    })
                    .collect();
                self.emit(function, Instruction::PsetfSymbols(names), span)?;
            } else if places
                .iter()
                .all(|place| matches!(place, crate::PsetfPlace::List(..)))
            {
                let list_places = places
                    .into_iter()
                    .map(|place| match place {
                        crate::PsetfPlace::List(accessors, name, escaped) => {
                            (accessors, name, escaped)
                        }
                        crate::PsetfPlace::Symbol(..) | crate::PsetfPlace::SymbolPlist => {
                            unreachable!()
                        }
                    })
                    .collect();
                self.emit(function, Instruction::PsetfList(list_places), span)?;
            } else {
                self.emit(function, Instruction::PsetfPlaces(places), span)?;
            }
            return Ok(());
        }
        self.emit(
            function,
            Instruction::Psetf(Form::list(items.to_vec(), span)),
            span,
        )?;
        Ok(())
    }
}
