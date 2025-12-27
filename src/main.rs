use clap::Parser;
use languria2::repl::Repl;

#[derive(Parser)]
#[command(name = "languria")]
#[command(about = "a programming language")]
struct Cli {
    #[arg(short, long)]
    show_transpiled_python: bool,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = Repl::new(cli.show_transpiled_python).run() {
        eprintln!("Fatal error: {:?}", e);
        std::process::exit(1);
    }
}
