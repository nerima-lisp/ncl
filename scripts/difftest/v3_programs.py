"""Generate and shrink deterministic Common Lisp expressions for V3."""

from __future__ import annotations

import random
from collections.abc import Iterator
from typing import TypeAlias


Form: TypeAlias = int | str | list["Form"]
FunctionSignature: TypeAlias = tuple[tuple[str, ...], str]


def _integer(rng: random.Random) -> int:
    return rng.randint(-20, 20)


def _name(rng: random.Random, prefix: str) -> str:
    return f"{prefix}{rng.randrange(1 << 16):04x}"


def _expression(
    rng: random.Random,
    depth: int,
    variables: tuple[tuple[str, str], ...],
    expected: str,
) -> Form:
    if depth <= 0 or rng.random() < 0.22:
        available = [
            name for name, value_type in variables
            if expected == "object" or value_type == "integer"
        ]
        if available and rng.random() < 0.35:
            return rng.choice(available)
        return _integer(rng)

    families = ["arithmetic", "conditional", "binding", "sequence", "array", "flet", "labels", "closure"]
    if expected == "object":
        families.append("cons")
    family = rng.choice(families)
    child_depth = depth - 1
    if family == "arithmetic":
        operator = rng.choice(("+", "-", "*", "mod"))
        left = _expression(rng, child_depth, variables, "integer")
        if operator == "mod":
            right: Form = rng.randint(1, 9)
        else:
            right = _expression(rng, child_depth, variables, "integer")
        return [operator, left, right]
    if family == "conditional":
        condition = [
            rng.choice(("<", "=", ">")),
            _expression(rng, child_depth, variables, "integer"),
            _expression(rng, child_depth, variables, "integer"),
        ]
        return [
            "if", condition,
            _expression(rng, child_depth, variables, expected),
            _expression(rng, child_depth, variables, expected),
        ]
    if family == "binding":
        variable = _name(rng, "v")
        value = _expression(rng, child_depth, variables, "integer")
        body = _expression(rng, child_depth, ((variable, "integer"), *variables), expected)
        return ["let", [[variable, value]], body]
    if family == "sequence":
        return [
            "progn",
            _expression(rng, child_depth, variables, "object"),
            _expression(rng, child_depth, variables, expected),
        ]
    if family == "cons":
        return [
            rng.choice(("cons", "list")),
            _expression(rng, child_depth, variables, "object"),
            _expression(rng, child_depth, variables, "object"),
        ]
    if family == "array":
        size = rng.randint(2, 4)
        element_type = "integer" if expected == "integer" else "object"
        values = [
            _expression(rng, child_depth, variables, element_type)
            for _ in range(size)
        ]
        if expected == "integer" or rng.random() < 0.5:
            return ["aref", ["vector", *values], rng.randrange(size)]
        return ["vector", *values]
    if family == "flet":
        function = _name(rng, "f")
        parameter = _name(rng, "x")
        body = _expression(rng, child_depth, ((parameter, "integer"), *variables), expected)
        argument = _expression(rng, child_depth, variables, "integer")
        return ["flet", [[function, [parameter], body]], [function, argument]]
    if family == "labels":
        function = _name(rng, "sum")
        parameter = _name(rng, "n")
        initial = rng.randint(0, 6)
        definition = [
            function,
            [parameter],
            [
                "if",
                ["=", parameter, 0],
                0,
                ["+", parameter, [function, ["-", parameter, 1]]],
            ],
        ]
        return ["labels", [definition], [function, initial]]

    captured = _name(rng, "c")
    function = _name(rng, "fn")
    parameter = _name(rng, "x")
    captured_value = _integer(rng)
    argument = _expression(rng, child_depth, variables, "integer")
    closure = ["lambda", [parameter], ["+", captured, parameter]]
    return [
        "let",
        [[captured, captured_value]],
        ["let", [[function, closure]], ["funcall", function, argument]],
    ]


def generate_expression(seed: int, *, depth: int = 3) -> Form:
    """Return one valid, bounded expression for a stable integer seed."""
    rng = random.Random(seed)
    expected = rng.choice(("integer", "object"))
    return _expression(rng, depth, (), expected)


def render_form(form: Form) -> str:
    if isinstance(form, int):
        return str(form)
    if isinstance(form, str):
        return form
    if not form:
        return "nil"
    return "(" + " ".join(render_form(item) for item in form) + ")"


def render_program(form: Form, policy: str) -> str:
    if policy not in {"off", "default", "speed3"}:
        raise ValueError(f"unsupported policy: {policy}")
    declarations = "(declaim (optimize (speed 3)))\n" if policy == "speed3" else ""
    return declarations + f'(format t "~S~%" {render_form(form)})\n'


def ast_size(form: Form) -> int:
    if isinstance(form, list):
        return 1 + sum(ast_size(item) for item in form)
    return 1


def form_cost(form: Form) -> tuple[int, int, int, str]:
    magnitude = 0

    def count_magnitude(node: Form) -> None:
        nonlocal magnitude
        if isinstance(node, int):
            magnitude += abs(node)
        elif isinstance(node, list):
            for item in node:
                count_magnitude(item)

    count_magnitude(form)
    rendered = render_form(form)
    return ast_size(form), magnitude, len(rendered), rendered


def is_valid_expression(form: Form) -> bool:
    return _expression_type(form, {}, {}, {}) is not None


def _expression_type(
    form: Form,
    variables: dict[str, str],
    functions: dict[str, FunctionSignature],
    function_variables: dict[str, FunctionSignature],
) -> str | None:
    if isinstance(form, int):
        return "integer"
    if isinstance(form, str):
        return variables.get(form, "object" if form in {"nil", "t"} else None)
    if not isinstance(form, list):
        return None
    if not form:
        return "object"
    if not isinstance(form[0], str):
        return None

    operator = form[0]
    if operator in {"+", "-", "*", "mod"}:
        if len(form) != 3:
            return None
        left_type = _expression_type(form[1], variables, functions, function_variables)
        right_type = _expression_type(form[2], variables, functions, function_variables)
        if left_type != "integer" or right_type != "integer":
            return None
        if operator == "mod" and (not isinstance(form[2], int) or form[2] <= 0):
            return None
        return "integer"
    if operator in {"<", "=", ">"}:
        if len(form) != 3:
            return None
        operand_types = [
            _expression_type(item, variables, functions, function_variables)
            for item in form[1:]
        ]
        return "boolean" if operand_types == ["integer", "integer"] else None
    if operator == "if":
        if len(form) != 4 or any(
            _expression_type(item, variables, functions, function_variables) is None
            for item in form[1:]
        ):
            return None
        then_type = _expression_type(form[2], variables, functions, function_variables)
        else_type = _expression_type(form[3], variables, functions, function_variables)
        return then_type if then_type == else_type else "object"
    if operator == "let":
        if len(form) != 3 or not isinstance(form[1], list):
            return None
        bindings = form[1]
        local_variables = dict(variables)
        local_function_variables = dict(function_variables)
        names: set[str] = set()
        for binding in bindings:
            if (
                not isinstance(binding, list)
                or len(binding) != 2
                or not isinstance(binding[0], str)
                or binding[0] in names
            ):
                return None
            binding_type = _expression_type(binding[1], variables, functions, function_variables)
            if binding_type is None:
                return None
            names.add(binding[0])
            local_variables[binding[0]] = binding_type
            if binding_type == "function":
                signature = _lambda_signature(binding[1], variables, functions, function_variables)
                if signature is None:
                    return None
                local_function_variables[binding[0]] = signature
        return _expression_type(form[2], local_variables, functions, local_function_variables)
    if operator in {"flet", "labels"}:
        if len(form) != 3 or not isinstance(form[1], list) or not form[1]:
            return None
        definitions: list[tuple[str, list[str], Form]] = []
        names: set[str] = set()
        for definition in form[1]:
            if (
                not isinstance(definition, list)
                or len(definition) != 3
                or not isinstance(definition[0], str)
                or not isinstance(definition[1], list)
                or any(not isinstance(parameter, str) for parameter in definition[1])
                or len(set(definition[1])) != len(definition[1])
                or definition[0] in names
            ):
                return None
            names.add(definition[0])
            definitions.append((definition[0], definition[1], definition[2]))
        local_functions = dict(functions)
        if operator == "labels":
            local_functions.update({
                name: (("integer",) * len(parameters), "integer")
                for name, parameters, _ in definitions
            })
        signatures: dict[str, FunctionSignature] = {}
        for name, parameters, body in definitions:
            definition_variables = dict(variables)
            definition_variables.update({parameter: "integer" for parameter in parameters})
            definition_functions = local_functions if operator == "labels" else functions
            body_type = _expression_type(body, definition_variables, definition_functions, function_variables)
            if body_type is None or (operator == "labels" and body_type != "integer"):
                return None
            signature = (("integer",) * len(parameters), body_type)
            signatures[name] = signature
        local_functions.update(signatures)
        return _expression_type(form[2], variables, local_functions, function_variables)
    if operator == "lambda":
        return "function" if _lambda_signature(form, variables, functions, function_variables) else None
    if operator == "funcall":
        if len(form) != 3 or not isinstance(form[1], str):
            return None
        signature = function_variables.get(form[1])
        if signature is None or len(signature[0]) != 1:
            return None
        argument_type = _expression_type(form[2], variables, functions, function_variables)
        if argument_type != signature[0][0]:
            return None
        return signature[1]
    if operator == "aref":
        if (
            len(form) != 3
            or not isinstance(form[1], list)
            or not form[1]
            or form[1][0] != "vector"
            or not isinstance(form[2], int)
            or not 0 <= form[2] < len(form[1]) - 1
        ):
            return None
        vector_type = _expression_type(form[1], variables, functions, function_variables)
        if vector_type != "vector":
            return None
        return _expression_type(form[1][form[2] + 1], variables, functions, function_variables)
    if operator == "vector":
        if len(form) < 2 or any(
            _expression_type(item, variables, functions, function_variables) is None
            for item in form[1:]
        ):
            return None
        return "vector"
    if operator == "progn":
        if len(form) < 2:
            return None
        result_type = None
        for item in form[1:]:
            result_type = _expression_type(item, variables, functions, function_variables)
            if result_type is None:
                return None
        return result_type
    if operator in {"cons", "list"}:
        if len(form) != 3 or any(
            _expression_type(item, variables, functions, function_variables) is None
            for item in form[1:]
        ):
            return None
        return "cons"
    if operator in functions:
        parameter_types, result_type = functions[operator]
        if len(form) - 1 != len(parameter_types):
            return None
        argument_types = [
            _expression_type(item, variables, functions, function_variables)
            for item in form[1:]
        ]
        return result_type if tuple(argument_types) == parameter_types else None
    return None


def _lambda_signature(
    form: Form,
    variables: dict[str, str],
    functions: dict[str, FunctionSignature],
    function_variables: dict[str, FunctionSignature],
) -> FunctionSignature | None:
    if (
        not isinstance(form, list)
        or len(form) != 3
        or form[0] != "lambda"
        or not isinstance(form[1], list)
        or any(not isinstance(parameter, str) for parameter in form[1])
        or len(set(form[1])) != len(form[1])
    ):
        return None
    local_variables = dict(variables)
    local_variables.update({parameter: "integer" for parameter in form[1]})
    result_type = _expression_type(form[2], local_variables, functions, function_variables)
    if result_type is None:
        return None
    return (("integer",) * len(form[1]), result_type)


def _expression_paths(form: Form, path: tuple[int, ...] = ()) -> Iterator[tuple[tuple[int, ...], Form]]:
    yield path, form
    if not isinstance(form, list) or not form or not isinstance(form[0], str):
        return
    operator = form[0]
    if operator == "let":
        for index, binding in enumerate(form[1]):
            yield from _expression_paths(binding[1], (*path, 1, index, 1))
        yield from _expression_paths(form[2], (*path, 2))
    elif operator in {"flet", "labels"}:
        for index, definition in enumerate(form[1]):
            yield from _expression_paths(definition[2], (*path, 1, index, 2))
        yield from _expression_paths(form[2], (*path, 2))
    elif operator == "lambda":
        yield from _expression_paths(form[2], (*path, 2))
    elif operator == "funcall":
        yield from _expression_paths(form[2], (*path, 2))
    elif operator == "aref":
        vector = form[1]
        for index, item in enumerate(vector[1:], start=1):
            yield from _expression_paths(item, (*path, 1, index))
        yield from _expression_paths(form[2], (*path, 2))
    elif operator == "vector":
        for index, item in enumerate(form[1:], start=1):
            yield from _expression_paths(item, (*path, index))
    else:
        for index, item in enumerate(form[1:], start=1):
            yield from _expression_paths(item, (*path, index))


def _replace(form: Form, path: tuple[int, ...], replacement: Form) -> Form:
    if not path:
        return replacement
    if not isinstance(form, list):
        raise ValueError("replacement path does not point into a list")
    index, *tail = path
    result = list(form)
    result[index] = _replace(result[index], tuple(tail), replacement)
    return result


def structural_replacements(form: Form) -> list[Form]:
    """Return smaller, well-formed expression candidates deterministically."""
    candidates: dict[str, Form] = {}
    original_cost = form_cost(form)
    for path, node in _expression_paths(form):
        replacements: list[Form] = []
        if isinstance(node, int):
            replacements.extend((0, 1, -1, node // 2))
        elif isinstance(node, str):
            replacements.extend((0, 1, -1))
        elif isinstance(node, list):
            if not node or node[0] != "lambda":
                replacements.extend((0, 1, -1))
            if node and node[0] in {"+", "-", "*", "mod", "<", "=", ">"}:
                replacements.extend(node[1:])
            elif node and node[0] == "progn":
                replacements.append(node[-1])
            elif node and node[0] == "aref":
                vector, index = node[1], node[2]
                if isinstance(vector, list) and vector and vector[0] == "vector" and isinstance(index, int):
                    replacements.append(vector[index + 1])
            elif node and node[0] == "funcall":
                replacements.append(node[2])
            elif node and node[0] == "if":
                replacements.extend(node[2:])
        else:
            continue
        for replacement in replacements:
            candidate = _replace(form, path, replacement)
            if form_cost(candidate) < original_cost and is_valid_expression(candidate):
                candidates[render_form(candidate)] = candidate
    return sorted(candidates.values(), key=form_cost)
