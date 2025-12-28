use clap::Parser;
use langulo::repl::Repl;

#[derive(Parser)]
#[command(name = "langulo")]
#[command(about = "a programming language")]
struct Cli {
    #[arg(short, long)]
    debug: bool,
}

fn main() {
    let cli = Cli::parse();
    let show_python = cli.debug || cfg!(debug_assertions);
    if let Err(e) = Repl::new(show_python).run() {
        eprintln!("Fatal error: {:?}", e);
        std::process::exit(1);
    }
}
