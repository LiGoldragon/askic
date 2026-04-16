/// askic — the aski frontend.
///
/// Dialect state machine driven by embedded aski-core rkyv data.
/// Reads .aski source → rkyv parse tree (sema-core types).
///
/// Usage: askic <file.aski>

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: askic <file.aski>");
        std::process::exit(1);
    }

    // TODO: dialect engine reads embedded rkyv, parses source,
    // serializes parse tree as rkyv using sema-core types
    eprintln!("askic: not yet implemented — dialect engine pending");
    std::process::exit(1);
}
