# ncl-objfile

`ncl-objfile` is the dependency-free binary-format layer for NCL. It only
returns byte vectors and does not perform file or operating-system I/O.

## Public API

Format-independent codegen types:

- `Relocation` and `RelocKind` describe fixups without importing an assembler
  crate.
- `Section` and `SectionId` identify named byte sections.
- `SymbolRef` identifies a local symbol index or an external symbol name.

FASL:

- `FaslWriter::write` serializes a `Fasl` value.
- `FaslReader::read` validates and reads a FASL for an expected architecture
  and feature bitmap.
- `Architecture`, `Fasl`, `FaslHeader`, `FaslSection`, and `ObjectError` are
  the associated public types.

Relocatable object files:

- `ElfObject::write` writes ELF64 objects for `x86-64` and `AArch64`.
- `ElfReader::validate` and `validate_elf` validate ELF object structure.
- `MachObject::write` writes 64-bit Mach-O objects for `x86_64` and `arm64`.
- `MachReader::validate` and `validate_macho` validate Mach-O object structure.
- `sections_from_generic` converts generic `Section` values to ELF sections.

Executable envelopes:

- `write_elf_executable` writes an ELF image with executable code and NCL
  metadata.
- `write_mach_executable` writes a Mach-O image with a native entry and NCL
  metadata.
- `validate_elf_executable` and `validate_mach_executable` reject images that
  lack the required NCL metadata.
- `ExecutableImage` is the input shared by both executable writers.

The caller owns filesystem output and macOS ad-hoc signing. In particular,
the crate does not invoke `codesign`.
