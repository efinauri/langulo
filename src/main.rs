use clap::Command;

mod emitter;
mod errors;
mod lexer;
mod parser;
mod repl;
mod tests;
mod typecheck;
mod vm;
pub mod word;

fn main() {
    let _matches = Command::new("langulo-rs")
        .version("1.0")
        .author("Edoardo Finauri")
        .about("REPL for the Langulo programming language")
        .get_matches();
    repl::serve_repl();
}
