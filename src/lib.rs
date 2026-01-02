// because of https://github.com/zkat/miette/issues/458
// to disable the warnings all the error enum payloads start with underscore
#![allow(non_snake_case)]
pub mod errors;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod transpile;
pub mod repl;
