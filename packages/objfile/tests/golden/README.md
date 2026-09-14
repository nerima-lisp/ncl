# Object-file golden fixtures

The fixture commands are run during development with the LLVM toolchain:

```text
llvm-readobj --all fixture.o
llvm-objdump -r -d fixture.o
llvm-nm fixture.o
```

Runtime tests use the dependency-free readers and do not invoke external tools.

The checked-in summaries were captured with Apple LLVM 21.0.0. `elf-x86-64.txt`
and `macho-arm64.txt` record the fields consumed by the reader tests: machine,
section names, section sizes, relocation names, and symbol names.
