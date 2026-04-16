# askic — The Aski Frontend

askic is a self-contained binary that reads .aski source and
produces .sema binary. It is one frontend for the sema format.

Sema is the thing — the universal typed binary format. Aski is
one text notation for specifying sema. askic turns that notation
into the canonical binary form. Eventually aski may be replaced
by better ways to represent sema; askic would be replaced too.
semac (the sema backend) is permanent and independent.

## Self-Contained

askic has no runtime dependencies. One binary, one input format
(.aski), one output format (.sema). Everything else is built in.

Internally, three layers:
```
cc      (aski-core crate)  — .aski → Rust types (language anatomy)
askicc  (askicc crate)     — uses cc + .synth → scoped types + dialects
askic   (this crate)       — uses askicc → parser, data-tree, .sema
```

askic's own types are compiled enum variants from askicc — zero
strings. User types are read as strings during parsing, then
generated as scoped enums.

## Current State

Not yet built. Stubs in src/.
v015_reference/ — 14 .aski modules from v0.15 (see TERMINOLOGY.md).

## VCS

Jujutsu (`jj`) mandatory. Always pass `-m`.
Domain = any data def (enum + struct + newtype).
Tests in separate files.
