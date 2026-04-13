# askic — the aski compiler, written in aski

Self-hosting compiler. Stage1 (aski-rs-bootstrap) compiles these
.aski sources to Rust. rustc compiles that Rust + ffi/ crate into
the askic binary. That binary can then compile itself.

## Structure

```
source/         .aski modules (the compiler in aski)
ffi/            Rust crate providing FFI implementations
flake.nix       build pipeline: stage1 → .sema → .rs → rustc + ffi → binary
```

## Pipeline

```
source/*.aski → stage1 askic compile → .sema + .aski-table.sema
                stage1 askic rust .sema → .rs
                rustc .rs + ffi/ crate → askic binary
```

## VCS

Jujutsu (`jj`) mandatory. Branch: `main`. Push after every change.
