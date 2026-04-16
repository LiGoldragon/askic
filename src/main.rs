/// askic — the aski frontend.
///
/// Reads .aski source files, produces .sema binary.
/// One binary, one input format, one output format.

use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: askic <file.aski>");
        std::process::exit(1);
    }

    let path = PathBuf::from(&args[1]);
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|e| {
            eprintln!("askic: failed to read {}: {}", path.display(), e);
            std::process::exit(1);
        });

    let program = askic::parse::parse_source(&source)
        .unwrap_or_else(|e| {
            eprintln!("askic: parse error in {}: {}", path.display(), e);
            std::process::exit(1);
        });

    eprintln!("askic: parsed {} → {} root children", path.display(), program.children.len());

    // serialize to .sema
    let sema_bytes = program.to_sema_bytes();
    let sema_path = path.with_extension("sema");
    fs::write(&sema_path, &sema_bytes)
        .unwrap_or_else(|e| {
            eprintln!("askic: failed to write {}: {}", sema_path.display(), e);
            std::process::exit(1);
        });

    eprintln!("askic: wrote {} ({} bytes)", sema_path.display(), sema_bytes.len());
}
