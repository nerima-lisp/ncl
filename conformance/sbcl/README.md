# SBCL surface tables

Regenerate the tables from the repository root with:

```sh
nix develop --command sbcl --script conformance/sbcl/extract-surface.lisp
```

The extractor requires SBCL 2.6.0 and checks the runtime version, so the `sbcl` on `PATH` for the local command above must be that version. The Linux x86-64 workflow installs the official 2.6.0 binary and invokes its absolute path from inside `nix develop`, so the dev shell's SBCL version cannot take precedence. See [`symbols/README.md`](symbols/README.md) for the schema, per-platform counts, and provenance.
