mod cli;

use clap::Parser;

fn main() {
    let args = cli::Cli::parse();
    privcrab::style::banner();
    privcrab::util::log::init(args.verbose);
    cli::ensure_privilege(&args.command);
    if let Err(err) = cli::run(args) {
        cli::report(&err);
        std::process::exit(1);
    }
}