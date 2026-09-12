#[allow(clippy::wildcard_imports)]
use super::*;
use ncl_syntax::Form;

mod array_places;
mod bit_place;
pub(super) mod list;
#[path = "list_places.rs"]
mod list_places;
mod nth_places;
mod property_places;
mod subseq_places;

#[allow(clippy::too_many_lines)]
pub(super) fn execute_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Set(name) | Instruction::SetExact(name) => {
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("setq has no value on the stack", span))?
                .primary_value();
            if matches!(instruction, Instruction::Set(_)) {
                runtime.set_or_define_in(name, value.clone(), environment, span)?;
            } else {
                runtime.set_or_define_exact_in(name, value.clone(), environment, span)?;
            }
            if name.eq_ignore_ascii_case("*RANDOM-STATE*") {
                crate::builtins::set_dynamic_random_state(&value);
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("setq has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::Setf(place) | Instruction::MapIntoSetf(place) => {
            let map_into = matches!(instruction, Instruction::MapIntoSetf(_));
            let value = stack
                .last()
                .cloned()
                .ok_or_else(|| {
                    invalid(
                        if map_into {
                            "map-into has no value on the stack"
                        } else {
                            "setf has no value on the stack"
                        },
                        span,
                    )
                })?
                .primary_value();
            if map_into {
                runtime.set_map_into_destination(place, value.clone(), environment)?;
            } else {
                runtime.set_place(place, value.clone(), environment)?;
            }
            *stack
                .last_mut()
                .ok_or_else(|| invalid("setf has no value on the stack", span))? = value;
            *program_counter += 1;
            Ok(true)
        }
        Instruction::SetfListPlace { operator, place } => list_places::execute_setf_list_place(
            runtime,
            operator,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfNthPlace { place } => nth_places::execute_setf_nth_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfArefVectorPlace { place } => array_places::execute_setf_aref_vector_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::MapIntoArefVectorPlace {
            sequence_count,
            place,
        } => array_places::execute_map_into_aref_vector_place(
            runtime,
            *sequence_count,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::MapIntoListPlace {
            operator,
            place,
            sequence_count,
        } => array_places::MapIntoListPlaceExecution {
            runtime,
            operator,
            place,
            sequence_count: *sequence_count,
            stack,
            environment,
            program_counter,
            span,
        }
        .execute(),
        Instruction::MapIntoNthPlace {
            sequence_count,
            place,
        } => nth_places::execute_map_into_nth_place(
            runtime,
            *sequence_count,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfArefArrayPlace { index_count, place } => {
            array_places::execute_setf_aref_array_place(
                runtime,
                *index_count,
                place,
                stack,
                environment,
                program_counter,
                span,
            )
        }
        Instruction::SetfBitPlace { index_count, place } => bit_place::execute_setf_bit_place(
            runtime,
            *index_count,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfGethashPlace { place } => {
            property_places::execute_setf_gethash_place(place, stack, program_counter, span)
        }
        Instruction::SetfGetfPlace { place } => property_places::execute_setf_getf_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfRowMajorArefPlace { place } => {
            array_places::execute_setf_row_major_aref_place(
                runtime,
                place,
                stack,
                environment,
                program_counter,
                span,
            )
        }
        Instruction::SetfSvrefPlace { place } => array_places::execute_setf_svref_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfStringCharPlace { operator, place } => {
            array_places::execute_setf_string_char_place(
                runtime,
                operator,
                place,
                stack,
                environment,
                program_counter,
                span,
            )
        }
        Instruction::SetfEltPlace { place } => array_places::execute_setf_elt_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::SetfSubseqPlace { place } => subseq_places::execute_setf_subseq_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PopListPlace { operator, place } => list_places::execute_pop_list_place(
            runtime,
            operator,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PopNthPlace { place } => nth_places::execute_pop_nth_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushNthPlace { place } => nth_places::execute_push_nth_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushnewNthPlace { place } => nth_places::execute_pushnew_nth_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushListPlace { operator, place } => list_places::execute_push_list_place(
            runtime,
            operator,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushGetfPlace { place } => property_places::execute_push_getf_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushnewGetfPlace { place } => property_places::execute_pushnew_getf_place(
            runtime,
            place,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::PushnewListPlace {
            operator,
            place,
            options,
        } => list_places::execute_pushnew_list_place(
            runtime,
            operator,
            place,
            options,
            stack,
            environment,
            program_counter,
            span,
        ),
        _ => Ok(false),
    }
}

pub(super) fn execute_parallel_set_instruction(
    runtime: &Runtime,
    instruction: &Instruction,
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    match instruction {
        Instruction::Psetq(names) => {
            if stack.len() < names.len() {
                return Err(invalid("psetq has fewer values than targets", span));
            }
            let values = stack.split_off(stack.len() - names.len());
            for (name, value) in names.iter().zip(values) {
                let value = value.primary_value();
                runtime.set_or_define_in(name, value, environment, span)?;
            }
            stack.push(Value::Nil);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PsetqExact(names) => {
            if stack.len() < names.len() {
                return Err(invalid("psetq has fewer values than targets", span));
            }
            let values = stack.split_off(stack.len() - names.len());
            for ((name, escaped), value) in names.iter().zip(values) {
                let value = value.primary_value();
                if *escaped {
                    runtime.set_or_define_exact_in(name, value, environment, span)?;
                } else {
                    runtime.set_or_define_in(name, value, environment, span)?;
                }
            }
            stack.push(Value::Nil);
            *program_counter += 1;
            Ok(true)
        }
        Instruction::PsetfListPlaces(places) => list_places::execute_psetf_list_places(
            runtime,
            places,
            stack,
            environment,
            program_counter,
            span,
        ),
        Instruction::Shiftf(names) => {
            if stack.len() < names.len() + 1 {
                return Err(invalid("shiftf has fewer values than places", span));
            }
            let values = stack.split_off(stack.len() - names.len() - 1);
            let old_values = &values[..names.len()];
            let new_value = values.last().cloned().unwrap_or(Value::Nil).primary_value();
            for (index, (name, escaped)) in names.iter().enumerate() {
                let value = old_values
                    .get(index + 1)
                    .cloned()
                    .unwrap_or_else(|| new_value.clone())
                    .primary_value();
                if *escaped {
                    runtime.set_or_define_exact_in(name, value, environment, span)?;
                } else {
                    runtime.set_or_define_in(name, value, environment, span)?;
                }
            }
            stack.push(
                old_values
                    .first()
                    .cloned()
                    .unwrap_or(Value::Nil)
                    .primary_value(),
            );
            *program_counter += 1;
            Ok(true)
        }
        Instruction::ShiftfListPlaces(places) => {
            execute_shiftf_list_places(runtime, places, stack, environment, program_counter, span)
        }
        Instruction::RotatefListPlaces(places) => {
            execute_rotatef_list_places(runtime, places, stack, environment, program_counter, span)
        }
        Instruction::MultipleValueSetq(names) => {
            let source = pop_value(stack, span, "multiple-value-setq")?;
            let values = source.multiple_values();
            for (index, name) in names.iter().enumerate() {
                let value = values.get(index).cloned().unwrap_or(Value::Nil);
                runtime.set_or_define_in(name, value, environment, span)?;
            }
            stack.push(source.primary_value());
            *program_counter += 1;
            Ok(true)
        }
        Instruction::MultipleValueSetqExact(names) => {
            let source = pop_value(stack, span, "multiple-value-setq")?;
            let values = source.multiple_values();
            for (index, (name, escaped)) in names.iter().enumerate() {
                let value = values.get(index).cloned().unwrap_or(Value::Nil);
                if *escaped {
                    runtime.set_or_define_exact_in(name, value, environment, span)?;
                } else {
                    runtime.set_or_define_in(name, value, environment, span)?;
                }
            }
            stack.push(source.primary_value());
            *program_counter += 1;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn execute_shiftf_list_places(
    runtime: &Runtime,
    places: &[(String, Form)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() + 1 {
        return Err(invalid("shiftf has fewer values than places", span));
    }
    let values = stack.split_off(stack.len() - places.len() - 1);
    let old_values = &values[..places.len()];
    let new_value = values.last().cloned().unwrap_or(Value::Nil).primary_value();
    for (index, (operator, place)) in places.iter().enumerate() {
        let value = old_values
            .get(index + 1)
            .cloned()
            .unwrap_or_else(|| new_value.clone())
            .primary_value();
        runtime.set_list_place_value(
            operator,
            place,
            &old_values[index],
            value,
            environment,
            span,
        )?;
    }
    stack.push(
        old_values
            .first()
            .cloned()
            .unwrap_or(Value::Nil)
            .primary_value(),
    );
    *program_counter += 1;
    Ok(true)
}

fn execute_rotatef_list_places(
    runtime: &Runtime,
    places: &[(String, Form)],
    stack: &mut Vec<Value>,
    environment: &Environment,
    program_counter: &mut usize,
    span: Span,
) -> Result<bool, RuntimeError> {
    if stack.len() < places.len() {
        return Err(invalid("rotatef has fewer values than places", span));
    }
    let old_values = stack.split_off(stack.len() - places.len());
    let old_slot_values = places
        .iter()
        .zip(&old_values)
        .map(|((operator, place), current)| {
            let elements = current.list_items().ok_or_else(|| RuntimeError::Type {
                expected: "LIST".to_string(),
                actual: current.type_name().to_string(),
                span: Some(place.span),
            })?;
            if elements.is_empty() {
                return Err(RuntimeError::Type {
                    expected: "non-empty LIST".to_string(),
                    actual: "NIL".to_string(),
                    span: Some(place.span),
                });
            }
            match operator.as_str() {
                "CAR" | "FIRST" => Ok(elements[0].clone()),
                "CDR" | "REST" => Ok(Value::list(elements[1..].to_vec())),
                _ => Err(RuntimeError::Type {
                    expected: "supported list place".to_string(),
                    actual: operator.clone(),
                    span: Some(place.span),
                }),
            }
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    let replacement = old_slot_values
        .last()
        .cloned()
        .unwrap_or(Value::Nil)
        .primary_value();
    for (index, (operator, place)) in places.iter().enumerate() {
        let value = if index == 0 {
            replacement.clone()
        } else {
            old_slot_values[index - 1].clone().primary_value()
        };
        runtime.set_list_place_value(
            operator,
            place,
            &old_values[index],
            value,
            environment,
            span,
        )?;
    }
    stack.push(Value::Nil);
    *program_counter += 1;
    Ok(true)
}
