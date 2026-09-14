# Object-file golden fixtures

The fixture commands are run during development with the LLVM toolchain:

```text
llvm-readobj --all fixture.o
llvm-objdump -r -d fixture.o
llvm-nm fixture.o
```

Runtime tests use the dependency-free readers and do not invoke external tools.
