# NCL runtime FASL

NCL runtime FASL files use the `ncl-objfile` version-1 container. The
container has the fixed `NCLFASL\0` magic, a 64-byte little-endian header, a
pointer width of 8, a target architecture, and feature bits. The reader
rejects an incompatible architecture, feature set, invalid section ranges,
and invalid relocation offsets before a payload is executed.

The runtime compile unit is stored in the container's debug section until the
native code and relocation publication path is available for persistent code
objects. Its payload is:

| Field | Size | Meaning |
| --- | ---: | --- |
| `NCLRTFASL` | 9 | Runtime payload magic |
| version | 2 | Little-endian payload version, currently 1 |
| source length | 8 | Little-endian byte length |
| source hash | 8 | FNV-1a hash of the source bytes |
| source | variable | UTF-8 source forms |

`compile-file` evaluates the source through the normal native pipeline, then
writes `<source stem>.fasl`. `load` identifies the outer FASL magic, validates
the container and runtime payload, checks the source hash, and evaluates the
verified source forms. A source file is never overwritten by compilation.

This is an intermediate compatibility format. The future native FASL payload
will replace the source field with code blobs, constants, symbol bindings,
fixups, stack maps, and top-level execution records while retaining target and
integrity validation before publication.
