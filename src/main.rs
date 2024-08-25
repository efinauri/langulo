use clap::Command;

mod lexer;
mod parser;
mod repl;
mod errors;
mod typecheck;

fn main() {
    let _matches = Command::new("langulo-rs")
        .version("1.0")
        .author("Edoardo Finauri")
        .about("REPL for the Langulo programming language")
        .get_matches();
    repl::serve_repl();
}
