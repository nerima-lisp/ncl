# ncl-image

`ncl-image` saves the object graph reachable from caller-supplied roots, stores
supplied code images separately, and records runtime feature names as metadata
that `load` does not apply. It depends on the `ncl-object`, `ncl-objfile`, and
`ncl-sys` crates.

## Public API

```rust
pub fn register(runtime: &Runtime) -> Result<(), ObjectError>;

pub struct CodeImage { /* opaque */ }
impl CodeImage {
    pub fn capture(code: &CodePtr, entry_offset: usize, frame_words: u16, function_name: &str)
        -> Result<Self, ImageError>;
    pub fn from_raw(bytes: Vec<u8>, entry_offset: usize, frame_words: u16, function_name: String)
        -> Result<Self, ImageError>;
    pub fn bytes(&self) -> &[u8];
    pub fn entry_offset(&self) -> usize;
    pub fn frame_words(&self) -> u16;
    pub fn function_name(&self) -> &str;
    pub fn publish(&self) -> Result<CodePtr, ImageError>;
}

pub fn save(runtime: &Runtime, ctx: &mut ThreadContext, roots: &[Word], code: &[CodeImage])
    -> Result<Vec<u8>, ImageError>;

pub struct LoadedImage { pub roots: Vec<Word>, pub code: Vec<CodePtr> }
pub fn load(bytes: &[u8], runtime: &Runtime, ctx: &mut ThreadContext)
    -> Result<LoadedImage, ImageError>;

pub const FORMAT_VERSION: u16 = 2;
```

`ncl_image::register(runtime)` installs the Lisp-visible `NCL-IMAGE` symbols:
the special variable `*POSIX-ARGV*`, `EXIT` as both a class and function, and
the functions `QUIT`, `SAVE-LISP-AND-DIE`, `OS-COLD-INIT-OR-REINIT`, `OS-DEINIT`,
and `OS-EXIT`. `ncl-stdlib` calls it during initialization; Rust callers do not
need it unless they use those symbols. Separately, call `ctx.register(runtime)`
to attach the thread context to the runtime heap before `save` or `load`.

`CodeImage::publish()` copies one blob into a new published code allocation in
the current process. `load` uses it to republish every supplied blob.

With caller-created source and destination runtimes, roots, and code images, the
save/load call sequence is:

```rust
let mut save_ctx = ThreadContext::new();
save_ctx.register(&source_runtime)?;
let bytes = save(&source_runtime, &mut save_ctx, &roots, &code)?;

let mut load_ctx = ThreadContext::new();
load_ctx.register(&target_runtime)?;
let loaded = load(&bytes, &target_runtime, &mut load_ctx)?;
```

## Image format overview (version 2)

The container is little-endian. This overview lists the header, shared
encodings, and notable record fields; it does not specify every per-kind field.
A fixed 64-byte header is followed by one payload holding object records, roots,
code blobs, and feature strings in that order.

### Header

| offset | size | field |
| ---: | ---: | --- |
| 0 | 8 | magic `NCLIMAGE` |
| 8 | 2 | version (2; version 1 is also accepted) |
| 10 | 1 | architecture, 1 x86-64, 2 AArch64 |
| 11 | 1 | pointer width (8) |
| 12 | 1 | endianness (1 little) |
| 13 | 1 | header size (64) |
| 14 | 2 | reserved (written as 0; ignored when loading) |
| 16 | 4 | object record count |
| 20 | 4 | root count |
| 24 | 4 | code blob count |
| 28 | 4 | feature count |
| 32 | 8 | heap collection epoch at save time |
| 40 | 4 | payload offset (64) |
| 44 | 4 | payload size |
| 48 | 16 | reserved (written as 0; ignored when loading) |

The saved collection epoch is parsed but not used by `load`. `load` rejects an
image written for another architecture.

### Records

One record per reachable object, in index order. A record is a kind byte
followed by kind-specific fields. Kinds are cons (0), symbol (1), package (2),
string (3), simple vector (4), specialized array (5), hash table (6), structure
(7), instance (8), function (9), code object (10), bignum (11), ratio (12),
double float (13), and complex (14).

A reference is one tag byte: `0` followed by a `u64` of raw tagged `Word` bits
for an immediate, or `1` followed by a `u32` record index for a heap object.
Lengths and counts are `u32`; strings are a `u32` length followed by UTF-8
bytes.

Symbols are stored as `(package name, symbol name, flags, value, function,
plist)`, not as heap copies, so loading re-interns them into their home
packages. Packages store a name and a nickname list; their internal and
external tables are rebuilt by symbol interning. Hash tables store their test,
weakness, and live entries, and are rebuilt through `HashTable::insert`.
Kind 9 is a function record for either a simple function or a closure; its
closure flag distinguishes them. Its code reference points to a code-object
record in the object table, not to a separate code blob. Kind 10 stores a code
object's entry address, size, constants, stack map, and debug references. `load`
restores those fields as recorded, so the entry address is usable only if the
same code remains mapped there in the target process. The supplied code blobs
do not rebase it.

### Code blobs

Each supplied `CodeImage` becomes one code blob containing a length-prefixed
UTF-8 function name, `u32` entry byte offset within the code allocation, `u16`
frame-word count, `u32` byte length, and raw bytes. `load` republishes each blob
with `alloc_code` → `write_code` → `publish_code` and returns a `CodePtr` to the
allocation base in `LoadedImage.code`. The entry offset is relative to that base
and is used when invoking the code. Use the returned pointer and saved offset to
invoke a blob directly with `ncl_sys::invoke_entry`. Supplying a `CodeImage`
does not set the entry address of a function or code-object record. Returned
pointers correspond to the saved code blobs in order, but `LoadedImage` does
not return their names, offsets, or frame sizes; retain that metadata separately
for invocation.
`CodeImage::from_raw` accepts entry offsets from zero through the byte length
when the value fits `u32`, but does not check instruction boundaries. An offset
equal to the byte length is accepted but does not designate a byte to execute.
`CodeImage::capture` copies the entire published allocation and applies the same
offset checks; its offset is measured from that allocation's base.
`frame_words` is the caller-supplied native frame size in words. Both
constructors store it as-is without checking it against the code. Code blobs
carry no relocations. A simple-function record without a builtin identity
retains its saved entry address and remains callable only if the same native
function is mapped at that address in the target process; this is not guaranteed
across restarts.

Feature strings follow the code blobs and use the string encoding above. They
are names added through `Runtime::add_feature`, which the Lisp source reader
uses for its `#+` and `#-` feature conditionals. `load` parses but discards them.
Callers that need those names in the destination runtime must obtain them from
another source and pass them to `Runtime::add_feature`.

Version 2 adds an optional package/name identity to simple-function records for
registered builtins. Kind 9 stores a closure flag, entry address, name, lambda
list, code reference, and captured-value reference list in that order. Each
version 2 function record then has a one-byte identity tag: closures always use
`0`; for a simple function, `0` means absent and `1` is followed by the package
and name strings. When an identity is present, the function record's
entry-address field is saved as zero, and `load` resolves it to the most
recently registered matching builtin. It returns
`ImageError::UnregisteredBuiltin` if the target runtime has no matching
registration. Version 1 records have no builtin identity, so `load` retains a
builtin's serialized native entry address and does not rebind it. The
raw-address constraint above applies. Version 1 uses the same header and other
record and code-blob encodings; its kind 9 records end after the captured-value
references, without the version 2 identity tag.

## Save and load semantics

- `save` walks the graph reachable from `roots`, records each object once, and
  stores references as record indices. It performs no managed-heap allocation,
  so the roots need no `RootToken`.
- `load` runs four passes in the supplied destination `Runtime`: create packages,
  intern symbols, allocate placeholder objects, then write every payload slot
  and hash-table entry. It reuses packages by name and symbols returned by
  `Package::intern`; restored symbol flags, value, function, and property list
  replace those fields on reused symbols. It does not reset the destination
  runtime. Register matching builtins in that runtime before loading an image
  that contains them, using `ncl-stdlib` initialization or
  `Runtime::register_builtin`. The object table is held in a `Vec<Word>` rooted
  for the whole rebuild, so allocation cannot lose a partially built object.
- A failed `load` can leave the destination runtime changed because
  reconstruction has no rollback. Use a disposable runtime when a failed load
  must not affect existing state.
- Each serialized reference resolves to `Word::from_bits` for immediates and to
  the restored object for indices. Every reference store goes through
  `ncl_sys::write_object_word` and `ncl_sys::write_barrier`.

## Supported and rejected kinds

Supported: cons, symbol, package, string, simple vector, specialized array,
hash table, structure, instance, simple function, closure, code object, bignum,
ratio, double float, and complex.

`save` returns `ImageError::UnsupportedKind` when traversal reaches a
non-simple array, readtable, or stream. General arrays created with
`ncl_object::make_array` use the rejected non-simple array kind; the separate
`SpecializedArray` object type is supported.

## ncl-sys / ncl-object API gaps

`save` handles only the graph reachable from supplied roots. The public API
cannot enumerate every heap object or stop the world without collecting, so it
cannot produce a whole-heap snapshot. Preserving runtime registries and code
metadata is also beyond the current APIs. The following are possible API
additions, not current functionality or committed plans.

1. Heap enumeration. `State.objects` and `Heap`'s page accessors are
   `pub(crate)`, so no caller can list live objects or read an object's total
   word count. A public read-only object-iteration API would be required to dump
   the whole heap instead of a caller-rooted graph.

2. Stop-the-world without collecting. The only public stop-the-world entry
   point is `ncl_sys::collect`, which runs a collection. A whole-heap snapshot
   that must not collect needs the `stw` begin/end pair exposed, or a callback
   API that runs code while the world is stopped.

3. Runtime registry access. `Runtime` exposes no `&Heap` accessor and no
   enumeration of its function, class, or package registries. Preserving those
   registries in an image needs a public `&Heap` accessor and package
   enumeration; the function and class registries would need analogous
   enumeration APIs.

4. Code metadata and relocation. There is no public way to enumerate the heap's
   `CodeRegistry`, and `CodeObjectMetadata` cannot be rebuilt from a `CodeImage`
   alone (`SafepointMap` has no public constructor). A code image preserves its
   bytes, but safepoint maps, debug tables, constant-slot values, and heap
   references inside code are not preserved.

## Suggested verification commands

```text
nix develop --command cargo test --locked -p ncl-image
nix develop --command cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps'
python3 scripts/check_standards.py
python3 scripts/reachability.py
```

Tests: `tests/coverage.rs` checks registration of the `NCL-IMAGE` symbols;
`tests/round_trip.rs` saves a cons/symbol/string/vector/hash-table/function
graph, loads it into a fresh runtime, checks isomorphism, and runs a full
collection on the loaded objects; `tests/kinds.rs` round-trips specialized
arrays, bignums, ratios, double floats, complex numbers, structures, instances,
and closures; `tests/code_image.rs` publishes machine code, saves it, reloads
it, and invokes the republished entry.
