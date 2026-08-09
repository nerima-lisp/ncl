# Recipes

These examples use only the currently implemented runtime surface.

## Keep definitions across evaluations

Pass multiple expressions to one CLI invocation. They share the same runtime:

~~~sh
cargo run -- --eval '(define twice (lambda (x) (* 2 x)))' --eval '(twice 21)'
~~~

The same pattern works with <code>--compiled</code>:

~~~sh
cargo run -- --compiled --eval '(defun cube (x) (* x x x))' --eval '(cube 3)'
~~~

## Work with multiple values

Bind results returned by <code>values</code>:

~~~lisp
(multiple-value-bind (left right)
    (values 10 20)
  (+ left right))
~~~

<code>multiple-value-call</code>, <code>multiple-value-list</code>, and
<code>multiple-value-prog1</code> provide the corresponding collection and
sequencing operations.

## Define a small macro

Use quasiquote and unquote in a macro definition:

~~~lisp
(defmacro twice (form)
  `(progn ,form ,form))

(twice (+ 20 1))
~~~

The same macro can be used in interpreted and compiled evaluation.

## Capture printed output

String output streams are useful for composing output without writing a file:

~~~lisp
(let ((stream (make-string-output-stream)))
  (write-string "hello" stream)
  (terpri stream)
  (get-output-stream-string stream))
~~~

Use <code>make-string-input-stream</code>, <code>read-char</code>, and
<code>read-line</code> for the corresponding bounded input operations.

## Update a place

The current <code>setf</code> implementation supports common places such as
variables, list accessors, array elements, property lists, and hash-table
values:

~~~lisp
(let ((items (list 1 2 3)))
  (setf (car items) 10)
  items)
~~~

## Inspect the available API

The [API reference](../reference/api.md) is the source of truth for the
current forms, builtins, CLI flags, and Rust exports.
