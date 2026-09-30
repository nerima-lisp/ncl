#[allow(clippy::too_many_lines)]
fn defstruct_macro_builtin(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = ncl_object::Scope::new(ctx);
    let form = scope.root(ncl_object::Local::from_word(args.required(0)?));
    let parts = macro_list_to_handles(&mut scope, form)?;
    let name_form = *parts.as_slice().get(1).ok_or(ObjectError::TypeError)?;
    let mut include: ncl_object::Handle<'_, Word> =
        scope.root(ncl_object::Local::from_word(Word::NIL));
    let mut include_overrides = Vec::new();
    let mut conc_name = None;
    let mut conc_name_specified = false;
    let mut constructors = Vec::new();
    let mut constructor_specified = false;
    let mut predicate_name = None;
    let mut copier_name = None;
    let mut print_function = None;
    let mut predicate_disabled = false;
    let mut copier_disabled = false;
    let (name, mut option_index) = if scope.get(name_form).as_word().is_cons() {
        let header = macro_list_to_handles(&mut scope, name_form)?;
        let name = *header.as_slice().first().ok_or(ObjectError::TypeError)?;
        for option in header.as_slice().iter().copied().skip(1) {
            let fields = macro_list_to_handles(&mut scope, option)?;
            let key = *fields.as_slice().first().ok_or(ObjectError::TypeError)?;
            let value = *fields.as_slice().get(1).ok_or(ObjectError::TypeError)?;
            let key_name = symbol_name_string(scope.context(), scope.get(key).as_word())?;
            if key_name == ":INCLUDE" || key_name == "INCLUDE" {
                include = scope.root(ncl_object::Local::from_word(scope.get(value).as_word()));
                let include_word = scope.get(include).as_word();
                if include_word != Word::NIL && include_word.is_cons() {
                    let include_fields = macro_list_to_handles(&mut scope, value)?;
                    include = scope.root(ncl_object::Local::from_word(
                        scope
                            .get(
                                *include_fields
                                    .as_slice()
                                    .first()
                                    .ok_or(ObjectError::TypeError)?,
                            )
                            .as_word(),
                    ));
                    for override_form in include_fields.as_slice().iter().skip(1).copied() {
                        let fields = macro_list_to_handles(&mut scope, override_form)?;
                        include_overrides.push(defstruct_slot_spec(&mut scope, &fields)?);
                    }
                } else {
                    for override_form in fields.as_slice().iter().skip(2).copied() {
                        let fields = macro_list_to_handles(&mut scope, override_form)?;
                        include_overrides.push(defstruct_slot_spec(&mut scope, &fields)?);
                    }
                }
            } else if key_name == ":PREDICATE" || key_name == "PREDICATE" {
                predicate_name = if defstruct_is_nil(scope.context(), scope.get(value).as_word())? {
                    predicate_disabled = true;
                    None
                } else {
                    Some(symbol_name_string(
                        scope.context(),
                        scope.get(value).as_word(),
                    )?)
                };
            } else if key_name == ":COPIER" || key_name == "COPIER" {
                copier_name = if defstruct_is_nil(scope.context(), scope.get(value).as_word())? {
                    copier_disabled = true;
                    None
                } else {
                    Some(symbol_name_string(
                        scope.context(),
                        scope.get(value).as_word(),
                    )?)
                };
            } else if key_name == ":CONC-NAME" || key_name == "CONC-NAME" {
                let value_word = scope.get(value).as_word();
                conc_name = defstruct_name_or_nil(&scope, value_word)?;
                conc_name_specified = true;
            } else if key_name == ":CONSTRUCTOR" || key_name == "CONSTRUCTOR" {
                let value_word = scope.get(value).as_word();
                let constructor_name = defstruct_name_or_nil(&scope, value_word)?;
                let lambda = fields.as_slice().get(2).copied().map(|value| {
                    scope.root(ncl_object::Local::from_word(scope.get(value).as_word()))
                });
                constructors.push((constructor_name, lambda));
                constructor_specified = true;
            } else if key_name == ":PRINT-FUNCTION" || key_name == "PRINT-FUNCTION" {
                print_function =
                    Some(scope.root(ncl_object::Local::from_word(scope.get(value).as_word())));
            } else {
                return Err(ObjectError::TypeError);
            }
        }
        (name, 2)
    } else {
        (name_form, 2)
    };
    let name_text = symbol_name_string(scope.context(), scope.get(name).as_word())?;
    let definition_package = {
        let package = ncl_object::symbol_package(scope.context(), scope.get(name).as_word())?;
        let package_name = ncl_object::Package::from_word(package).name(scope.context())?;
        let length = ncl_object::string_length(scope.context(), package_name)?;
        (0..length)
            .map(|index| ncl_object::string_ref(scope.context(), package_name, index))
            .collect::<Result<String, _>>()?
    };
    let mut slot_specs = Vec::new();
    while let Some(slot) = parts.as_slice().get(option_index).copied() {
        let slot_word = scope.get(slot).as_word();
        let is_option = matches!(
            ncl_object::classify_object(scope.context(), slot_word),
            ObjectRef::Symbol(_)
        ) && symbol_name_string(scope.context(), slot_word)?.starts_with(':');
        if is_option {
            break;
        }
        if slot_word.is_cons() {
            let fields = macro_list_to_handles(&mut scope, slot)?;
            slot_specs.push(defstruct_slot_spec(&mut scope, &fields)?);
        } else {
            slot_specs.push((
                scope.root(ncl_object::Local::from_word(scope.get(slot).as_word())),
                scope.root(ncl_object::Local::from_word(Word::NIL)),
                false,
            ));
        }
        option_index += 1;
    }
    if !conc_name_specified {
        conc_name = Some(format!("{name_text}-"));
    }
    if predicate_name.is_none() {
        predicate_name = Some(format!("{name_text}-P"));
    }
    if copier_name.is_none() {
        copier_name = Some(format!("COPY-{name_text}"));
    }
    let mut predicate_name = predicate_name.ok_or(ObjectError::Layout)?;
    let mut copier_name = copier_name.ok_or(ObjectError::Layout)?;
    let mut index = option_index;
    while let Some(option) = parts.as_slice().get(index).copied() {
        let option_name = symbol_name_string(scope.context(), scope.get(option).as_word())?;
        let value = parts
            .as_slice()
            .get(index + 1)
            .copied()
            .ok_or(ObjectError::TypeError)?;
        match option_name.as_str() {
            ":CONC-NAME" | "CONC-NAME" => {
                let value_word = scope.get(value).as_word();
                conc_name = defstruct_name_or_nil(&scope, value_word)?;
            }
            ":CONSTRUCTOR" | "CONSTRUCTOR" => {
                let value_word = scope.get(value).as_word();
                let constructor_name = defstruct_name_or_nil(&scope, value_word)?;
                let lambda = parts.as_slice().get(index + 2).copied().map(|value| {
                    scope.root(ncl_object::Local::from_word(scope.get(value).as_word()))
                });
                constructors.push((constructor_name, lambda));
                constructor_specified = true;
                if lambda.is_some() {
                    index += 1;
                }
            }
            ":PRINT-FUNCTION" | "PRINT-FUNCTION" => {
                print_function =
                    Some(scope.root(ncl_object::Local::from_word(scope.get(value).as_word())));
            }
            ":PREDICATE" | "PREDICATE" => {
                if defstruct_is_nil(scope.context(), scope.get(value).as_word())? {
                    predicate_disabled = true;
                } else {
                    predicate_name =
                        symbol_name_string(scope.context(), scope.get(value).as_word())?;
                }
            }
            ":COPIER" | "COPIER" => {
                if defstruct_is_nil(scope.context(), scope.get(value).as_word())? {
                    copier_disabled = true;
                } else {
                    copier_name = symbol_name_string(scope.context(), scope.get(value).as_word())?;
                }
            }
            ":INCLUDE" | "INCLUDE" => {
                include = scope.root(ncl_object::Local::from_word(scope.get(value).as_word()));
                let include_word = scope.get(include).as_word();
                if include_word != Word::NIL && include_word.is_cons() {
                    let include_fields = macro_list_to_handles(&mut scope, value)?;
                    include = scope.root(ncl_object::Local::from_word(
                        scope
                            .get(
                                *include_fields
                                    .as_slice()
                                    .first()
                                    .ok_or(ObjectError::TypeError)?,
                            )
                            .as_word(),
                    ));
                    for override_form in include_fields.as_slice().iter().skip(1).copied() {
                        let fields = macro_list_to_handles(&mut scope, override_form)?;
                        include_overrides.push(defstruct_slot_spec(&mut scope, &fields)?);
                    }
                }
            }
            ":TYPE" | "TYPE" => {
                let type_name = symbol_name_string(scope.context(), scope.get(value).as_word())?;
                if type_name != "STRUCTURE" {
                    return Err(ObjectError::TypeError);
                }
            }
            _ => return Err(ObjectError::TypeError), // check-added-lines: allow(wildcard) reject unknown options
        }
        index += 2;
    }
    let conc_name = conc_name.unwrap_or_default();
    let include_word = scope.get(include).as_word();
    let include_name = if include_word == Word::NIL {
        None
    } else {
        Some(symbol_name_string(scope.context(), include_word)?)
    };
    let parent_layout = if include_word == Word::NIL {
        None
    } else {
        runtime.structure_layout_for_symbol(scope.context(), include_word)
    };
    let superclass = if let Some(include_name) = include_name.as_deref() {
        let qualified_name = runtime
            .structure_class_name(scope.context(), include_word)
            .ok();
        qualified_name
            .as_deref()
            .and_then(|name| runtime.class(scope.context_mut(), name))
            .or_else(|| runtime.class(scope.context_mut(), include_name))
            .ok_or(ObjectError::TypeError)?
    } else {
        runtime
            .class(scope.context_mut(), "STRUCTURE-OBJECT")
            .ok_or(ObjectError::Layout)?
    };
    let superclass: ncl_object::Handle<'_, Word> =
        scope.root(ncl_object::Local::from_word(superclass));
    let mut direct_specs = slot_specs.clone();
    direct_specs.extend(include_overrides);
    let mut descriptors = Vec::new();
    for (slot, initform, read_only) in &direct_specs {
        let values = scope.root_many(&[
            ncl_object::Local::from_word(scope.get(*slot).as_word()),
            ncl_object::Local::from_word(scope.get(*initform).as_word()),
            ncl_object::Local::from_word(if *read_only { Word::TRUE } else { Word::NIL }),
        ]);
        descriptors.push(scope.make_simple_vector(runtime, &values)?);
    }
    let descriptor_values = descriptors
        .iter()
        .map(|handle| ncl_object::Local::from_word(scope.get(*handle).as_word()))
        .collect::<Vec<_>>();
    let rooted_descriptors = scope.root_many(&descriptor_values);
    let direct_slots = scope.make_simple_vector(runtime, &rooted_descriptors)?;
    let direct_slots_word = scope.get(direct_slots).as_word();
    let name_word = scope.get(name).as_word();
    let superclass_word = scope.get(superclass).as_word();
    let class = make_class(
        scope.context_mut(),
        runtime,
        name_word,
        superclass_word,
        direct_slots_word,
        Word::fixnum(1),
    )?;
    let class: ncl_object::Handle<'_, Word> = scope.root(ncl_object::Local::from_word(class));
    let class_word = scope.get(class).as_word();
    runtime.define_class(scope.context_mut(), name_text.clone(), class_word)?;
    let name_word = scope.get(name).as_word();
    let qualified_class_name = runtime.structure_class_name(scope.context(), name_word)?;
    let class_word = scope.get(class).as_word();
    runtime.define_class(scope.context_mut(), qualified_class_name, class_word)?;
    if let Some(print_function) = print_function {
        let key = scope.intern(runtime, "NCL", "%STRUCTURE-PRINT-FUNCTION")?;
        let old_plist = scope.root(ncl_object::Local::from_word(ncl_object::symbol_plist(
            scope.context(),
            scope.get(name).as_word(),
        )?));
        let property = scope.make_cons(runtime, key, print_function)?;
        let plist = scope.make_cons(runtime, property, old_plist)?;
        let plist_word = scope.get(plist).as_word();
        let name_word = scope.get(name).as_word();
        ncl_object::set_symbol_plist(scope.context_mut(), name_word, plist_word)?;
    }
    let effective: ncl_object::Handle<'_, Word> =
        scope.root(ncl_object::Local::from_word(ncl_object::simple_vector_ref(
            scope.context(),
            scope.get(class).as_word(),
            CLASS_EFFECTIVE_SLOTS,
        )?));
    let effective_count =
        ncl_object::simple_vector_length(scope.context(), scope.get(effective).as_word())?;
    let layout = runtime.register_structure_layout(effective_count)?;
    runtime.register_structure_class_with_parent(
        scope.context(),
        layout,
        parent_layout,
        scope.get(name).as_word(),
    )?;
    let layout_word = Word::fixnum(i64::from(layout.as_u32()));

    let defun = scope.intern(runtime, COMMON_LISP, "DEFUN")?;
    let defsetf = scope.intern(runtime, COMMON_LISP, "DEFSETF")?;
    let key_marker = scope.intern(runtime, COMMON_LISP, "&KEY")?;
    let make = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-MAKE")?;
    let predicate = (!predicate_disabled)
        .then(|| scope.intern(runtime, &definition_package, &predicate_name))
        .transpose()?;
    let copier = (!copier_disabled)
        .then(|| scope.intern(runtime, &definition_package, &copier_name))
        .transpose()?;
    let mut effective_names: ncl_object::HandleVec<'_, Word> = scope.root_many(&[]);
    for slot in 0..effective_count {
        let descriptor =
            ncl_object::simple_vector_ref(scope.context(), scope.get(effective).as_word(), slot)?;
        let name = ncl_object::simple_vector_ref(scope.context(), descriptor, 0)?;
        effective_names.push(&mut scope, ncl_object::Local::from_word(name));
    }
    let mut forms = scope.root_many(&[]);
    if !constructor_specified {
        constructors.push((Some(format!("MAKE-{name_text}")), None));
    }
    for (constructor_name, boa_lambda) in constructors {
        let Some(constructor_name) = constructor_name else {
            continue;
        };
        let constructor = scope.intern(runtime, &definition_package, &constructor_name)?;
        let lambda_form = if let Some(boa_lambda) = boa_lambda {
            boa_lambda
        } else {
            let mut lambda = scope.root_many(&[ncl_object::Local::from_word(
                scope.get(key_marker).as_word(),
            )]);
            for slot in effective_names.iter().copied() {
                let slot_word = scope.get(slot).as_word();
                let descriptor = ncl_object::simple_vector_ref(
                    scope.context(),
                    scope.get(effective).as_word(),
                    effective_names
                        .iter()
                        .position(|name| scope.get(*name).as_word() == slot_word)
                        .ok_or(ObjectError::Layout)?,
                )?;
                let initform = ncl_object::simple_vector_ref(scope.context(), descriptor, 1)?;
                if initform == Word::NIL {
                    lambda.push(&mut scope, ncl_object::Local::from_word(slot_word));
                } else {
                    let binding_values = scope.root_many(&[
                        ncl_object::Local::from_word(slot_word),
                        ncl_object::Local::from_word(initform),
                    ]);
                    let binding = scope.make_list(runtime, &binding_values)?;
                    macro_push_handle(&mut scope, &mut lambda, binding);
                }
            }
            scope.make_list(runtime, &lambda)?
        };
        let parameters = if boa_lambda.is_some() {
            let lambda_word = scope.get(lambda_form).as_word();
            defstruct_boa_slot_parameters(&mut scope, lambda_word)?
        } else {
            Vec::new()
        };
        let mut body = scope.root_many(&[
            ncl_object::Local::from_word(scope.get(make).as_word()),
            ncl_object::Local::from_word(layout_word),
        ]);
        for slot in effective_names.iter().copied() {
            let slot_word = scope.get(slot).as_word();
            let slot_name = symbol_name_string(scope.context(), slot_word)?;
            let value = if boa_lambda.is_none() {
                slot_word
            } else {
                parameters
                    .iter()
                    .find(|(name, _)| name == &slot_name)
                    .map_or_else(
                        || {
                            let index = effective_names
                                .iter()
                                .position(|name| scope.get(*name).as_word() == slot_word)
                                .ok_or(ObjectError::Layout)?;
                            let descriptor = ncl_object::simple_vector_ref(
                                scope.context(),
                                scope.get(effective).as_word(),
                                index,
                            )?;
                            ncl_object::simple_vector_ref(scope.context(), descriptor, 1)
                        },
                        |(_, value)| Ok(scope.get(*value).as_word()),
                    )?
            };
            body.push(&mut scope, ncl_object::Local::from_word(value));
        }
        let body_form = scope.make_list(runtime, &body)?;
        let definition = make_form(
            &mut scope,
            runtime,
            &[defun, constructor, lambda_form, body_form],
        )?;
        macro_push_handle(&mut scope, &mut forms, definition);
    }
    let object = scope.intern(runtime, "NCL", "OBJECT")?;
    let structure_p = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-P")?;
    let layout_handle = scope.root(ncl_object::Local::from_word(layout_word));
    if let Some(predicate) = predicate {
        let predicate_args =
            scope.root_many(&[ncl_object::Local::from_word(scope.get(object).as_word())]);
        let predicate_lambda = scope.make_list(runtime, &predicate_args)?;
        let predicate_body = make_form(&mut scope, runtime, &[structure_p, object, layout_handle])?;
        let predicate_definition = make_form(
            &mut scope,
            runtime,
            &[defun, predicate, predicate_lambda, predicate_body],
        )?;
        macro_push_handle(&mut scope, &mut forms, predicate_definition);
    }
    if let Some(copier) = copier {
        let copier_args =
            scope.root_many(&[ncl_object::Local::from_word(scope.get(object).as_word())]);
        let copier_lambda = scope.make_list(runtime, &copier_args)?;
        let structure_copy = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-COPY")?;
        let copier_body = make_form(&mut scope, runtime, &[structure_copy, object])?;
        let copier_definition = make_form(
            &mut scope,
            runtime,
            &[defun, copier, copier_lambda, copier_body],
        )?;
        macro_push_handle(&mut scope, &mut forms, copier_definition);
    }
    for (index, slot) in effective_names.iter().copied().enumerate() {
        let slot_name = symbol_name_string(scope.context(), scope.get(slot).as_word())?;
        let accessor = scope.intern(
            runtime,
            &definition_package,
            &format!("{conc_name}{slot_name}"),
        )?;
        let object = scope.intern(runtime, "NCL", "OBJECT")?;
        let structure_ref = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-REF")?;
        let index_value = i64::try_from(index).map_err(|_| ObjectError::TypeError)?;
        let index_handle = scope.root(ncl_object::Local::from_word(Word::fixnum(index_value)));
        let accessor_body = make_form(&mut scope, runtime, &[structure_ref, object, index_handle])?;
        let accessor_args =
            scope.root_many(&[ncl_object::Local::from_word(scope.get(object).as_word())]);
        let accessor_lambda = scope.make_list(runtime, &accessor_args)?;
        let accessor_definition = make_form(
            &mut scope,
            runtime,
            &[defun, accessor, accessor_lambda, accessor_body],
        )?;
        macro_push_handle(&mut scope, &mut forms, accessor_definition);
        let descriptor =
            ncl_object::simple_vector_ref(scope.context(), scope.get(effective).as_word(), index)?;
        let read_only = ncl_object::simple_vector_ref(scope.context(), descriptor, 2)? != Word::NIL;
        if read_only {
            continue;
        }
        let store = scope.intern(runtime, "NCL", "STORE")?;
        let place_args =
            scope.root_many(&[ncl_object::Local::from_word(scope.get(object).as_word())]);
        let place_lambda = scope.make_list(runtime, &place_args)?;
        let store_word = scope.get(store).as_word();
        let store_args = scope.root_many(&[ncl_object::Local::from_word(store_word)]);
        let store_lambda = scope.make_list(runtime, &store_args)?;
        let structure_set = scope.intern(runtime, COMMON_LISP, "%STRUCTURE-SET")?;
        let set_index = scope.root(ncl_object::Local::from_word(Word::fixnum(index_value)));
        let set_body = make_form(
            &mut scope,
            runtime,
            &[structure_set, object, set_index, store],
        )?;
        let set_definition = make_form(
            &mut scope,
            runtime,
            &[defsetf, accessor, place_lambda, store_lambda, set_body],
        )?;
        macro_push_handle(&mut scope, &mut forms, set_definition);
    }
    // The trailing element is this `progn`'s (and so `defstruct`'s own)
    // result: CLHS requires the structure *name* back, so it must be the
    // quoted symbol, not a bare one (which would compile as a read of the
    // variable of that name, which `defstruct` never binds).
    let quoted_name = quoted(&mut scope, runtime, name)?;
    macro_push_handle(&mut scope, &mut forms, quoted_name);
    make_progn(&mut scope, runtime, &forms).map(|result| scope.get(result).as_word())
}
