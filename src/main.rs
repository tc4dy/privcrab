mod cli;

use clap::Parser;

fn main() {
    let args = cli::Cli::parse();
    privcrab::style::banner();
    let _ = privcrab::util::log::init_with_file(args.verbose, args.log_file.clone());
    cli::ensure_privilege(&args.command);
    if let Err(err) = cli::run(args) {
        cli::report(&err);
        std::process::exit(1);
    }
}
