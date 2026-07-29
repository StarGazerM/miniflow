use std::env;
use std::fs;
use std::process::ExitCode;

use miniflow_core::extract_dataflow_core;

fn main() -> ExitCode {
    let mut args = env::args_os();
    let program = args.next().unwrap_or_default();
    let Some(path) = args.next() else {
        eprintln!("usage: {} <generated-rust-file>", program.to_string_lossy());
        return ExitCode::FAILURE;
    };
    if args.next().is_some() {
        eprintln!("expected exactly one generated Rust file");
        return ExitCode::FAILURE;
    }

    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("failed to read {}: {error}", path.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };
    match extract_dataflow_core(&source) {
        Ok(core) => {
            print!("{core}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to extract dataflow core from {}: {error}",
                path.to_string_lossy()
            );
            ExitCode::FAILURE
        }
    }
}
