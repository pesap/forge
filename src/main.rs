use clap::Parser;
use forge::cli::Cli;

fn main() {
    if let Err(error) = forge::run(Cli::parse()) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
