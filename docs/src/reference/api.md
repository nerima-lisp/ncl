# API reference

This page describes the current user-visible surface. Names are shown in
lowercase for readability; the reader and runtime use the corresponding Lisp
symbols. Behavior is intentionally bounded and may be narrower than the
Common Lisp specification.

## Command-line interface

| Option | Meaning |
| --- | --- |
| <code>--eval</code>, <code>-e</code> <em>source</em> | Read and evaluate source. The option may be repeated. |
| <code>--file</code>, <code>-f</code> <em>path</em> | Read and evaluate a Lisp file. |
| <code>--repl</code> | Start the interactive REPL. |
| <code>--compiled</code> | Use the stack-bytecode compiler and VM for evaluation. |
| <code>--compile</code> | Compile input and report bytecode artifact sizes without executing it. Requires <code>--eval</code> or <code>--file</code>. |
| <code>--quiet</code>, <code>-q</code> | Suppress normal value output and REPL prompts. |
| <code>--help</code>, <code>-h</code> | Print usage information. |
| <code>--version</code>, <code>-V</code> | Print the package version. |

If no expression, file, or explicit REPL option is supplied, the CLI starts a
REPL. Exit status 0 means success, 1 means an evaluation or file error, and 2
means a command-line usage error.

## Reader

The reader supports:

- symbols, keywords, integers, floating-point numbers, rational literals,
  strings, characters, booleans, lists, dotted lists, and vectors;
- quote, quasiquote, unquote, unquote-splicing, and function reader prefixes;
- line comments, nested block comments, and discarded forms with <code>#;</code>;
- bounded <code>read-from-string</code> parsing with EOF and character-position
  results.

An unquote outside a quasiquote is rejected. Reader forms carry source spans
through the Rust syntax crate.

## Special forms and definitions

### Reading and declarations

<code>quote</code>, <code>quasiquote</code>, <code>declare</code>,
<code>locally</code>, <code>eval-when</code>, <code>load-time-value</code>,
<code>declaim</code>, <code>proclaim</code>, and <code>the</code> are recognized.
Declaration and type-checking behavior is bounded.

### Conditionals and sequencing

<code>if</code>, <code>progn</code>, <code>prog1</code>, <code>prog2</code>,
<code>prog</code>, <code>prog*</code>, <code>and</code>, <code>or</code>,
<code>when</code>, <code>unless</code>, <code>cond</code>, <code>case</code>,
<code>ecase</code>, <code>typecase</code>, and <code>etypecase</code> provide
conditional and sequential evaluation.

### Bindings and iteration

<code>let</code>, <code>let*</code>, <code>flet</code>, <code>labels</code>,
<code>dotimes</code>, <code>dolist</code>, <code>do</code>, and <code>do*</code>
provide lexical variable, local function, and iteration bindings. The
ordinary lambda-list implementation supports required parameters and bounded
<code>&amp;optional</code>, <code>&amp;rest</code>, <code>&amp;key</code>,
<code>&amp;aux</code>, and <code>&amp;allow-other-keys</code> behavior.

### Functions and macros

<code>lambda</code>, <code>function</code>, <code>defun</code>,
<code>defmacro</code>, <code>macroexpand-1</code>, and <code>macroexpand</code>
define or inspect functions and macros. <code>destructuring-bind</code>
applies a lambda-list pattern to a value.

### Multiple values

<code>values</code>, <code>multiple-value-bind</code>,
<code>multiple-value-call</code>, <code>multiple-value-list</code>,
<code>multiple-value-prog1</code>, <code>multiple-value-setq</code>, and
<code>nth-value</code> create, consume, sequence, or assign multiple values.

### Conditions and non-local control

<code>error</code> (one message argument), <code>ignore-errors</code>,
<code>handler-case</code>,
<code>handler-bind</code>, <code>with-simple-restart</code>, <code>restart-bind</code>,
<code>restart-case</code>, <code>with-condition-restarts</code>,
<code>compute-restarts</code>, <code>find-restart</code>,
<code>restart-name</code>, and <code>invoke-restart</code> provide bounded
condition and restart handling. Restart introspection returns restart objects
and can filter them by an associated condition. <code>error</code> signals a
<code>SIMPLE-ERROR</code> that can be caught by the existing condition
handlers.

<code>catch</code>, <code>throw</code>, <code>block</code>,
<code>return-from</code>, <code>return</code>, <code>tagbody</code>,
<code>go</code>, <code>unwind-protect</code>, and <code>progv</code> provide
non-local control, dynamic variable binding, and cleanup.

### Packages and variables

<code>defpackage</code>, <code>in-package</code>, <code>define</code>,
<code>setq</code>, <code>psetq</code>, <code>defvar</code>,
<code>defparameter</code>, <code>defconstant</code>, <code>setf</code>,
<code>incf</code>, <code>decf</code>, <code>defsetf</code>,
<code>define-setf-expander</code>, <code>define-symbol-macro</code>,
<code>define-modify-macro</code>, and <code>defstruct</code> define packages,
variables, places, and basic structures.

### Object system

<code>defclass</code>, <code>defgeneric</code>, <code>defmethod</code>,
<code>make-instance</code>, <code>slot-value</code>, <code>slot-exists-p</code>,
<code>slot-boundp</code>, <code>slot-makunbound</code>, <code>class-of</code>,
<code>find-class</code>, <code>class-name</code>, <code>with-slots</code>, and
<code>with-accessors</code> provide the current bounded CLOS surface.
<code>subtypep</code>, <code>call-next-method</code>, and
<code>next-method-p</code> cover the corresponding type and method-dispatch
queries supported by the runtime.

<code>eval</code>, <code>funcall</code>, <code>apply</code>, and
<code>mapcar</code> are handled with access to the caller's runtime context.

## Numeric functions

The runtime currently provides integers, floating-point values, and rational
values. The numeric function surface includes:

<code>+</code>, <code>-</code>, <code>*</code>, <code>/</code>,
<code>expt</code>, <code>sqrt</code>, <code>signum</code>, <code>float</code>,
<code>rational</code>, <code>rationalize</code>, <code>=</code>,
<code>&lt;</code>, <code>&gt;</code>, <code>&lt;=</code>, <code>&gt;=</code>,
<code>zerop</code>, <code>plusp</code>, <code>minusp</code>,
<code>evenp</code>, <code>oddp</code>, <code>min</code>, <code>max</code>,
<code>abs</code>, <code>1+</code>, <code>1-</code>, <code>floor</code>,
<code>ceiling</code>, <code>truncate</code>, <code>round</code>,
<code>gcd</code>, <code>lcm</code>, <code>numerator</code>,
<code>denominator</code>, <code>mod</code>, <code>rem</code>, <code>ash</code>,
<code>logand</code>, <code>logior</code>, <code>logxor</code>,
<code>lognot</code>, <code>logtest</code>, <code>logcount</code>, and
<code>integer-length</code>.

## Lists, sequences, arrays, and hash tables

### Lists and sequences

<code>list</code>, <code>values-list</code>, <code>cons</code>, <code>car</code>,
<code>cdr</code>, <code>first</code>, <code>rest</code>, <code>append</code>,
<code>length</code>, <code>reverse</code>, <code>vector</code>, <code>nth</code>,
<code>elt</code>, <code>subseq</code>, <code>member</code>, <code>assoc</code>,
and <code>getf</code> operate on list and sequence values.

### Arrays

<code>make-array</code>, <code>aref</code>, <code>row-major-aref</code>,
<code>arrayp</code>, <code>array-rank</code>, <code>array-dimensions</code>,
<code>array-dimension</code>, and <code>array-total-size</code> provide simple
and multidimensional array operations.

### Hash tables

<code>make-hash-table</code>, <code>gethash</code>, <code>remhash</code>,
<code>clrhash</code>, <code>hash-table-p</code>, <code>hash-table-count</code>,
and <code>hash-table-test</code> provide hash-table creation, lookup,
mutation, and metadata. Supported tests are <code>eql</code>, <code>eq</code>,
<code>equal</code>, and <code>equalp</code>. <code>gethash</code> returns a
value and a found-status as multiple values.

## Characters, strings, streams, and I/O

### Characters and strings

<code>string</code>, <code>make-string</code>, <code>char</code>,
<code>char-code</code>, <code>code-char</code>, <code>characterp</code>,
<code>char=</code>, <code>char-equal</code>, <code>char&lt;</code>,
<code>char&gt;</code>, <code>char&lt;=</code>, <code>char&gt;=</code>,
<code>char-upcase</code>, and <code>char-downcase</code> operate on character
and string values.

<code>string=</code>, <code>string-equal</code>, <code>string&lt;</code>,
<code>string&gt;</code>, <code>string&lt;=</code>, <code>string&gt;=</code>,
<code>string-upcase</code>, and <code>string-downcase</code> provide bounded
string comparison and case conversion.

### Streams and output

<code>make-string-input-stream</code>, <code>make-string-output-stream</code>,
<code>get-output-stream-string</code>, <code>open</code>,
<code>with-open-file</code>, <code>read-char</code>, <code>peek-char</code>,
<code>unread-char</code>, <code>read-line</code>, <code>write-char</code>,
<code>write-string</code>, <code>terpri</code>, <code>fresh-line</code>,
<code>write-line</code>, <code>close</code>, <code>streamp</code>,
<code>input-stream-p</code>, and <code>output-stream-p</code> implement the
current string/file-stream and stream predicate surface. The pathname
operations <code>probe-file</code>, <code>delete-file</code>,
<code>rename-file</code>, <code>file-write-date</code>, and
<code>truename</code> cover the current file-management surface. File
streams currently provide character I/O and bounded <code>:direction</code>,
<code>:if-does-not-exist</code>, and <code>:if-exists</code> handling;
<code>:direction :io</code> supports duplex access with current-position
overwrites and append-mode writes.

<code>print</code>, <code>princ</code>, <code>prin1</code>,
<code>write-to-string</code>, and <code>format</code> provide output
operations. <code>format</code> currently supports <code>~A</code>,
<code>~S</code>, <code>~D</code>, <code>~B</code>, <code>~O</code>,
<code>~X</code>, <code>~C</code>, <code>~%</code>, <code>~&amp;</code>,
<code>~~</code>, <code>~*</code>, and <code>~?</code> with bounded
destinations and argument behavior.

## Predicates, equality, and types

The runtime provides:

<code>null</code>, <code>not</code>, <code>endp</code>, <code>atom</code>,
<code>consp</code>, <code>listp</code>, <code>numberp</code>,
<code>integerp</code>, <code>floatp</code>, <code>rationalp</code>,
<code>stringp</code>, <code>characterp</code>, <code>symbolp</code>,
<code>packagep</code>, <code>keywordp</code>, <code>vectorp</code>,
<code>functionp</code>, <code>eq</code>, <code>eql</code>, <code>equal</code>,
<code>equalp</code>, <code>identity</code>, <code>type-of</code>, and
<code>typep</code>.

## Symbols, packages, and variables

### Symbol operations

<code>symbol-name</code>, <code>symbol-package</code>, <code>make-symbol</code>,
<code>gensym</code>, <code>intern</code>, <code>find-symbol</code>, and
<code>find-package</code> inspect or create symbols and packages.
<code>intern</code> and <code>find-symbol</code> return status information as
multiple values.

### Package operations

<code>package-name</code>, <code>package-use-list</code>,
<code>list-all-packages</code>, <code>use-package</code>,
<code>unuse-package</code>, <code>export</code>, <code>unexport</code>,
<code>import</code>, <code>unintern</code>, <code>shadow</code>, and
<code>shadowing-import</code> provide the current package-management surface.

### Variable operations

<code>boundp</code>, <code>symbol-value</code>, <code>set</code>, and
<code>makunbound</code> inspect or mutate symbol values.

## Evaluation and mapping

<code>eval</code> evaluates a form in the caller's runtime environment.
<code>funcall</code> calls a function with already separated arguments, while
<code>apply</code> combines ordinary arguments with a final argument list.
<code>mapcar</code>, <code>mapc</code>, <code>mapl</code>,
<code>maplist</code>, <code>mapcan</code>, and <code>mapcon</code> provide the
currently registered mapping operations; list mapping stops at the shortest
proper input list where applicable.

## Runtime API boundary

The direct Common Lisp API is the `ncl` package and is documented in the
[Common Lisp core guide](../guide/common-lisp-core.md). The Rust API below is
the embedding surface for the production runtime; it is separate from the
direct SBCL-loaded core.

## Rust runtime API

The root Rust crate re-exports these convenience types:

- <code>CompiledForm</code>, <code>Environment</code>, <code>Function</code>, <code>Runtime</code>,
  <code>RuntimeError</code>, and <code>Value</code> from the runtime;
- <code>Form</code>, <code>FormKind</code>, <code>ReadError</code>,
  <code>ReadErrorKind</code>, <code>Span</code>, and <code>read</code> from the
  syntax crate.

The runtime crate additionally exposes <code>Rational</code> and
<code>Stream</code>. The compiler and runtime internals are not presented as a
stable external API; consult the source and tests when embedding the
workspace.

### Compilation API

<code>Runtime::compile</code> compiles one parsed <code>Form</code> and
<code>Runtime::compile_source</code> reads and compiles all forms in a source
string. Both return <code>CompiledForm</code> values without executing runtime
forms. A compiled form exposes its macro-expanded form through
<code>form()</code>, its bytecode program through <code>program()</code>, and
summary metrics through <code>function_count()</code> and
<code>instruction_count()</code>.

Compilation runs in order on one runtime, so compile-time macro and package
state can affect later forms. <code>eval_compiled</code> and
<code>eval_compiled_source</code> are the execution APIs; the source variant
compiles and executes each form in order so definitions and package operations
remain visible to subsequent forms.
