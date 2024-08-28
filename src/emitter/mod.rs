use crate::errors::err::LanguloErr;
use crate::parser::ast::lang::LanguloSyntaxNode;
use crate::parser::ast::node::AstNode;
use crate::parser::Parser;
use crate::typecheck::TypeChecker;
use crate::vm::instruction::{Instruction, OpCode};

macro_rules! cast_node {
    ($node:expr, $typ:ident) => {
        $node.text().to_string().parse::<$typ>()
           .map_err(|_|
            LanguloErr::semantic("expected node to be of type")
            )
    }
}

pub struct Emitter {
    bytecode: Vec<Instruction>,
    type_checker: TypeChecker,
    ast_root: LanguloSyntaxNode,
}

impl Emitter {
    pub fn new(input: &str) -> Result<Self, LanguloErr> {
        let mut parser = Parser::new(input);
        parser.parse()?;
        let ast_root = parser.to_ast();
        let mut type_checker = TypeChecker::new();
        type_checker.typecheck(&ast_root)?;

        Ok(Self {
            ast_root,
            type_checker,
            bytecode: Vec::new(),
        })
    }

    pub fn emit(&mut self) -> Result<(), LanguloErr> {
        for child in self.ast_root.children() { self.emit_node(&child)? }
        self.bytecode.push(Instruction::new(OpCode::Return, 0));
        Ok(())
    }

    fn emit_node(&mut self, node: &LanguloSyntaxNode) -> Result<(), LanguloErr> {
        // opcodes are laid out in a "vm-friendly" order, where when an operator comes up,
        // all the needed operands are already on the stack.
        match node.kind() {
            AstNode::Int => {
                self.bytecode.push(Instruction::new(OpCode::Constant, cast_node!(node, i32)?));
                Ok(())
            }
            AstNode::Add => {
                let children: Vec<_> = node.children().collect();
                self.emit_node(&children[0])?;
                self.emit_node(&children[1])?;
                self.bytecode.push(Instruction::new(OpCode::Add, 0));
                Ok(())
            },
            AstNode::Print => {
                self.emit_node(&node.first_child().unwrap())?;
                self.bytecode.push(Instruction::new(OpCode::Print, 0));
                Ok(())
            }
            _ => unimplemented!("todo: emit node type {:?}", node),
        }
    }

    pub fn to_bytecode(self) -> Vec<Instruction> { self.bytecode }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_emit(input: &str, expected_bytecode: Vec<Instruction>) {
        let mut emitter = Emitter::new(input).unwrap();
        emitter.emit().unwrap();
        let instructions = emitter.to_bytecode();
        for (obtained, expected) in instructions.into_iter().zip(expected_bytecode) {
            assert_eq!(obtained, expected);
        }
    }

    #[test]
    fn simple_addition() {
        expect_emit("3;", vec![Instruction::new(OpCode::Constant, 3)]);
        expect_emit("3+2;", vec![
            Instruction::new(OpCode::Constant, 3),
            Instruction::new(OpCode::Constant, 2),
            Instruction::new(OpCode::Add, 0)]);
    }
}