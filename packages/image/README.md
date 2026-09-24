# ncl-image

`ncl-image` saves and loads NCL runtime images: a rooted object graph, the
symbol table, the runtime feature list, and published code blocks. It depends
on `ncl-object`, `ncl-objfile`, and `ncl-sys`.

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

pub const FORMAT_VERSION: u16 = 1;
```

## Image format (version 1)

The container is little-endian. A fixed 64-byte header is followed by a single
payload holding the object records, roots, code blobs, and feature strings in
that order.

### Header

| offset | size | field |
| ---: | ---: | --- |
| 0 | 8 | magic `NCLIMAGE` |
| 8 | 2 | version (1) |
| 10 | 1 | architecture, 1 x86-64, 2 AArch64 |
| 11 | 1 | pointer width (8) |
| 12 | 1 | endianness (1 little) |
| 13 | 1 | header size (64) |
| 14 | 2 | reserved (0) |
| 16 | 4 | object record count |
| 20 | 4 | root count |
| 24 | 4 | code blob count |
| 28 | 4 | feature count |
| 32 | 8 | heap collection epoch at save time |
| 40 | 4 | payload offset (64) |
| 44 | 4 | payload size |
| 48 | 16 | reserved (0) |

The architecture field reuses `ncl_objfile::Architecture`. `load` rejects an
image written for another architecture.

### Records

One record per reachable object, in index order. A record is a kind byte
followed by kind-specific fields. Kinds are cons (0), symbol (1), package (2),
string (3), simple vector (4), specialized array (5), hash table (6), structure
(7), instance (8), function (9), code object (10), bignum (11), ratio (12),
double float (13), and complex (14).

A reference is one tag byte: `0` followed by a `u64` raw tagged word for an
immediate, or `1` followed by a `u32` record index for a heap object. Lengths
and counts are `u32`; strings are a `u32` length followed by UTF-8 bytes.

Symbols are stored as `(package name, symbol name, flags, value, function,
plist)`, not as heap copies, so loading re-interns them into their home
packages. Packages store a name and a nickname list; their internal and
external tables are rebuilt by symbol interning. Hash tables store their test,
weakness, and live entries, and are rebuilt through `HashTable::insert`. All
other kinds store the payload words that their object-layer accessors expose.

### Code blobs

Each code blob stores the function name, entry offset, frame size, byte length,
and raw bytes. `load` republishes each blob with `alloc_code` → `write_code` →
`publish_code` and returns the new `CodePtr`. Code blobs carry no relocations:
a raw entry address in a code object or function object is stored verbatim and
is not rewritten to the republished allocation.

## Save and load semantics

- `save` walks the graph reachable from `roots`, records each object once, and
  stores references as record indices. It performs no allocation, so the roots
  need no `RootToken`.
- `load` runs four passes in a fresh `Runtime`: create packages, intern symbols,
  allocate placeholder objects, then write every payload slot and hash-table
  entry. The object table is held in a `Vec<Word>` rooted for the whole rebuild,
  so allocation cannot lose a partially built object.
- Rooted references resolve to `Word::from_bits` for immediates and to the
  restored object for indices. Every reference store goes through
  `ncl_sys::write_object_word` and `ncl_sys::write_barrier`.

## Supported and rejected kinds

Supported: cons, symbol, package, string, simple vector, specialized array,
hash table, structure, instance, simple function, closure, code object, bignum,
ratio, double float, and complex.

Rejected with `ImageError::UnsupportedKind`: non-simple arrays, readtables, and
streams.

## ncl-sys / ncl-object API gaps

A whole-heap dump and a stop-the-world save are not possible with the current
public API. The following additions would close the gap; each is stated as a
requirement, not as implemented behavior.

1. Heap enumeration. `State.objects` and `Heap`'s page accessors are
   `pub(crate)`, so no caller can list live objects or read an object's total
   word count. A read-only snapshot API is required, for example:

   ```rust
   impl Heap {
       pub fn used_bytes(&self) -> usize;
       pub fn for_each_object(&self, visit: impl FnMut(Word, u8 /* widetag */, &[Word]));
   }
   ```

   With it, an image could dump the whole heap instead of a caller-rooted graph.

2. Stop-the-world without collecting. The only public stop-the-world entry
   point is `ncl_sys::collect`, which runs a collection. A save that must not
   collect needs the `stw` begin/end pair exposed, or a callback form:

   ```rust
   pub fn with_world_stopped(thread: &mut Thread, body: impl FnOnce());
   ```

3. Runtime registry access. `Runtime` exposes no `&Heap` accessor and no
   enumeration of its function, class, or package registries. Restoring those
   registries, rather than only the feature list, needs accessors such as:

   ```rust
   impl Runtime {
       pub fn heap(&self) -> &Heap;
       pub fn for_each_package(&self, ctx: &ThreadContext, visit: impl FnMut(Word));
   }
   ```

4. Code metadata and relocation. There is no public way to enumerate the heap's
   `CodeRegistry`, and `CodeObjectMetadata` cannot be rebuilt from bytes alone
   (`SafepointMap` has no public constructor). A code image therefore stores
   only bytes and an entry offset; safepoint maps, debug tables, constant-slot
   values, and heap references inside code are not preserved.

## Verification

```text
nix develop --command cargo test --locked -p ncl-image
nix develop --command cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps'
python3 scripts/check_standards.py
python3 scripts/reachability.py
```

Tests: `tests/coverage.rs` checks the seven owned symbols;
`tests/round_trip.rs` saves a cons/symbol/string/vector/hash-table/function
graph, loads it into a fresh runtime, checks isomorphism, and runs a full
collection on the loaded objects; `tests/kinds.rs` round-trips specialized
arrays, bignums, ratios, double floats, complex numbers, structures, instances,
and closures; `tests/code_image.rs` publishes machine code, saves it, reloads
it, and invokes the republished entry.
