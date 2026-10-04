fn pprint_logical_block(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let stream = args.required(0)?;
    let object = args.required(1)?;
    let prefix = args.required(2)?;
    let _per_line_prefix = args.required(3)?;
    let suffix = args.required(4)?;
    let thunk = FunctionObject::try_from(args.required(5)?).map_err(|_| ObjectError::TypeError)?;
    let options = PrintOptions::from_specials(ctx, runtime);
    let key = logical_block_key(ctx);
    let depth = logical_blocks()
        .lock()
        .map_err(|_| ObjectError::Layout)?
        .get(&key)
        .map_or(0, Vec::len);
    let tag = Word::fixnum(-1000 - i64::try_from(depth).map_err(|_| ObjectError::Layout)?);
    let frame = LogicalBlock {
        cursor: object,
        count: 0,
        limit: options.length().map(crate::options::NonNegative::get),
        tag,
        seen: HashSet::new(),
    };
    logical_blocks()
        .lock()
        .map_err(|_| ObjectError::Layout)?
        .entry(key)
        .or_default()
        .push(frame);
    let result = (|| {
        if prefix != Word::NIL {
            call_builtin(ctx, runtime, "WRITE-STRING", &[prefix, stream])?;
        }
        ctx.enter_catch(tag);
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        let body_result = caller.call_function(
            ctx,
            runtime,
            FunctionDesignator::Function(thunk),
            FunctionArguments::new(&[]),
            &mut values,
        );
        let was_exit = body_result == Err(ObjectError::NonLocalExit)
            && ctx.thread_mut().multiple_values().first().copied() == Some(tag);
        let leave_result = ctx.leave_catch();
        let _ = ctx.take_non_local_exit();
        leave_result?;
        if let Err(error) = body_result
            && !was_exit
        {
            return Err(error);
        }
        if suffix != Word::NIL {
            call_builtin(ctx, runtime, "WRITE-STRING", &[suffix, stream])?;
        }
        Ok(Word::NIL)
    })();
    let mut blocks = logical_blocks().lock().map_err(|_| ObjectError::Layout)?;
    if let Some(stack) = blocks.get_mut(&key) {
        stack.pop();
        if stack.is_empty() {
            blocks.remove(&key);
        }
    }
    result
}

fn pprint_pop(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let key = logical_block_key(ctx);
    let mut blocks = logical_blocks().lock().map_err(|_| ObjectError::Layout)?;
    let frame = blocks
        .get_mut(&key)
        .and_then(|stack| stack.last_mut())
        .ok_or(ObjectError::ControlError)?;
    if frame.cursor == Word::NIL
        || frame.limit.is_some_and(|limit| frame.count >= limit)
        || (frame.cursor.is_cons() && !frame.seen.insert(frame.cursor.address()))
    {
        let tag = frame.tag;
        drop(blocks);
        ctx.throw(tag, Word::NIL)?;
        return Err(ObjectError::NonLocalExit);
    }
    let cursor_is_cons = frame.cursor.is_cons();
    let value = if cursor_is_cons {
        car(ctx, frame.cursor)?
    } else {
        let value = frame.cursor;
        frame.cursor = Word::NIL;
        value
    };
    frame.count += 1;
    if cursor_is_cons {
        frame.cursor = cdr(ctx, frame.cursor)?;
    }
    let _ = runtime;
    Ok(value)
}

fn pprint_exit_if_list_exhausted(
    ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let key = logical_block_key(ctx);
    let blocks = logical_blocks().lock().map_err(|_| ObjectError::Layout)?;
    let frame = blocks
        .get(&key)
        .and_then(|stack| stack.last())
        .ok_or(ObjectError::ControlError)?;
    if frame.cursor == Word::NIL
        || frame.limit.is_some_and(|limit| frame.count >= limit)
        || (frame.cursor.is_cons() && frame.seen.contains(&frame.cursor.address()))
    {
        let tag = frame.tag;
        drop(blocks);
        ctx.throw(tag, Word::NIL)?;
        return Err(ObjectError::NonLocalExit);
    }
    Ok(Word::NIL)
}
