/// askic — the aski frontend.
///
/// Dialect state machine driven by aski-core rkyv data.
/// Reads .aski source → rkyv parse tree (sema-core types).
///
/// Usage: askic <file.aski> [output.rkyv]

use std::fs;
use std::path::PathBuf;

use askic::lexer::lex;
use askic::engine::Engine;

struct Askic {
    engine: Engine,
}

impl Askic {
    fn new() -> Self {
        let dialect_data = Self::load_dialect_data();
        Askic { engine: Engine::new(dialect_data) }
    }

    fn compile(&self, source_path: &str, output_path: &str) -> Result<(), String> {
        let source = fs::read_to_string(source_path)
            .map_err(|e| format!("failed to read {}: {}", source_path, e))?;

        let tokens = lex(&source)
            .map_err(|errs| format!("lex errors: {}", errs.iter()
                .map(|e| e.to_string()).collect::<Vec<_>>().join(", ")))?;

        let root_children = self.engine.parse(&tokens)?;

        eprintln!("askic: parsed {} → {} root children",
            source_path, root_children.len());

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&root_children)
            .map_err(|e| format!("serialization failed: {}", e))?;

        fs::write(output_path, bytes.as_ref())
            .map_err(|e| format!("failed to write {}: {}", output_path, e))?;

        eprintln!("askic: wrote {} ({} bytes)", output_path, bytes.len());
        Ok(())
    }

    fn load_dialect_data() -> &'static [u8] {
        // Try DIALECT_DATA env var (set by nix build or manually)
        if let Ok(path) = std::env::var("DIALECT_DATA") {
            let bytes = fs::read(&path)
                .unwrap_or_else(|e| panic!("failed to read DIALECT_DATA {}: {}", path, e));
            return Box::leak(bytes.into_boxed_slice());
        }

        // Local dev fallback
        let fallback = "../askicc/generated/dialects.rkyv";
        if std::path::Path::new(fallback).exists() {
            let bytes = fs::read(fallback).expect("failed to read dialect data");
            return Box::leak(bytes.into_boxed_slice());
        }

        eprintln!("askic: no dialect data. Set DIALECT_DATA or run askicc first.");
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: askic <file.aski> [output.rkyv]");
        std::process::exit(1);
    }

    let source_path = &args[1];
    let output_path = if args.len() > 2 {
        args[2].clone()
    } else {
        PathBuf::from(source_path).with_extension("rkyv")
            .to_string_lossy().to_string()
    };

    let compiler = Askic::new();
    compiler.compile(source_path, &output_path)
        .unwrap_or_else(|e| {
            eprintln!("askic: {}", e);
            std::process::exit(1);
        });
}
