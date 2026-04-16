# askic — The Aski Frontend

askic is a self-contained binary that reads .aski source and
produces an rkyv-serialized parse tree. It is one frontend for
the sema pipeline.

Sema is the thing — the universal typed binary format. Aski is
one text notation for specifying sema. askic turns that notation
into a parse tree. semac then produces true sema from that tree.
Eventually aski may be replaced; askic would be replaced too.
semac (the sema backend) is permanent and independent.

## Dialect-Based State Machine

askic contains NO language-specific parsing logic. It is a
generic dialect engine. askicc's rkyv domain-data-tree is
embedded in the askic binary at build time, giving it the
ability to read that version of aski's grammar. The engine
executes the embedded grammar as a state machine against
the token stream.

askic depends on corec's generated Rust types from aski-core
to deserialize the embedded rkyv data. These are the same
types askicc used to serialize it — aski-core is the input
contract. askic also depends on corec's generated Rust types
from sema-core to serialize its parse tree output — sema-core
is the output contract that semac reads.

Adding new syntax = adding .synth files + .aski domain
definitions in aski-core, then rebuilding askicc and askic.
No askic code changes.

## The Pipeline

```
corec     — .aski → Rust with rkyv derives (the bootstrap tool)
aski-core — grammar .aski + corec → Rust rkyv types (askicc↔askic contract)
sema-core — parse tree .aski + corec → Rust rkyv types (askic↔semac contract)
askicc    — uses aski-core types → rkyv dialect-data-tree
askic     — uses aski-core (input) + sema-core (output), embeds askicc's rkyv
semac     — uses sema-core types only, independent of aski
```

Six repos. They communicate through files.

**Only semac produces sema.** askic's output is rkyv — it has
strings (user names, literals). Sema has no unsized data.

**askic does NOT generate Rust.** Only corec and semac generate
Rust. askic reads rkyv data (aski-core types) and produces
rkyv data (sema-core types).

## Rust Style

**No free functions — methods on types always.** All Rust
will eventually be rewritten in aski, which uses methods
(traits + impls). `main` is the only exception.

Names describe WHAT IT IS structurally — not semantic meaning.
Small files. Tests in separate files.

## VCS

Jujutsu (`jj`) mandatory. Always pass `-m`.
Domain = any data def (enum + struct + newtype).
