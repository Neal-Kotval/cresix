use std::{env, fs, process::ExitCode};

use c6_core::ProjectManifest;

fn main() -> ExitCode {
    let paths = env::args().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!("usage: c6-example-manifest-validator <c6.toml>...");
        return ExitCode::FAILURE;
    }

    let mut failed = false;
    for path in paths {
        match fs::read_to_string(&path) {
            Ok(source) => match ProjectManifest::parse(&source) {
                Ok(_) => println!("valid: {path}"),
                Err(error) => {
                    eprintln!("invalid: {path}: {error}");
                    failed = true;
                }
            },
            Err(error) => {
                eprintln!("unreadable: {path}: {error}");
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
