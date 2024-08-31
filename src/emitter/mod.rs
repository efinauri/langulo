use crate::errors::err::LanguloErr;
use crate::parser::ast::lang::LanguloSyntaxNode;
use crate::parser::ast::node::AstNode;
use crate::parser::Parser;
use crate::typecheck::TypeChecker;
use crate::word::heap::{encode_table, Table};
use crate::word::structure::{OpCode, Word};

macro_rules! cast_node {
    ($node:expr, $typ:ident) => {
        $node.text().to_string().parse::<$typ>()
           .map_err(|_|
            LanguloErr::semantic("expected node to be of type")
            )
    }
}

macro_rules! emit_binary {
    ($self:expr, $node:expr, $opcode:expr) => {{

        let lhs = $self.emit_node(&$node.first_child().unwrap())?;
        $self.bytecode.push(lhs);
        let mut rhs = $self.emit_node(&$node.last_child().unwrap())?;
        rhs.change_opcode($opcode);
        Ok(rhs)
    }};
}

macro_rules! emit_unary {
    ($self:expr, $node:expr, $opcode:expr) => {{
        let mut operand = $self.emit_node(&$node.first_child().unwrap())?;
        operand.change_opcode($opcode);
        Ok(operand)
    }};
}
pub struct Emitter {
    type_checker: TypeChecker,
    ast_root: LanguloSyntaxNode,
    bytecode: Vec<Word>,

    /// the VM operates on these values by reading/writing a pointer to their location to the heap.
    /// in order to put these in a Word at compilation time, we would need to heap allocate them now.
    /// however, this would tightly couple the compilation and execution phases, which is undesirable.
    /// instead, the emitter will serialize these values in a file along with the bytecode, and allow
    /// the VM to load them to its runtime.
    heap_floats: Vec<f32>,
    heap_strings: Vec<String>,
    heap_tables: Vec<Vec<u64>>, // tables are serialized (just a sequence of key/value words)
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
            heap_floats: Vec::new(),
            heap_strings: Vec::new(),
            heap_tables: Vec::new(),
        })
    }

    pub fn emit(&mut self) -> Result<(), LanguloErr> {
        for child in self.ast_root.children() {
            let word = self.emit_node(&child)?;
            self.bytecode.push(word);
        }
        self.bytecode.push(Word::int(0, OpCode::Stop));
        Ok(())
    }

    fn emit_node(&mut self, node: &LanguloSyntaxNode) -> Result<Word, LanguloErr> {
        // opcodes are laid out in a "vm-friendly" order, where when an operator comes up,
        // all the needed operands are already on the stack.
        match node.kind() {
            AstNode::Int => Ok(Word::int(cast_node!(node, i32)?, OpCode::Value)),
            AstNode::Float => {
                self.heap_floats.push(cast_node!(node, f32)?);
                Ok(Word::raw_float((self.heap_floats.len() - 1) as u32))
            }
            AstNode::Str => {
                self.heap_strings.push(cast_node!(node, String)?);
                Ok(Word::raw_str((self.heap_strings.len() - 1) as u32))
            }
            AstNode::Table => {
                // todo eval table
                let table = Table::new();
                self.heap_tables.push(encode_table(&table));
                Ok(Word::raw_table((self.heap_tables.len() - 1) as u32))
            }

            AstNode::Add => emit_binary!(self, node, OpCode::Add),
            AstNode::Subtract => emit_binary!(self, node, OpCode::Subtract),
            AstNode::Multiply => emit_binary!(self, node, OpCode::Multiply),
            AstNode::Divide => emit_binary!(self, node, OpCode::Divide),
            AstNode::Modulo => emit_binary!(self, node, OpCode::Modulo),
            AstNode::LogicalAnd => emit_binary!(self, node, OpCode::LogicalAnd),
            AstNode::LogicalOr => emit_binary!(self, node, OpCode::LogicalOr),
            AstNode::LogicalXor => emit_binary!(self, node, OpCode::LogicalXor),
            AstNode::Print => emit_unary!(self, node, OpCode::Print),
            _ => unimplemented!("todo: emit node type {:?}", node),
        }
    }

    #[cfg(test)]
    pub fn to_bytecode(self) -> Vec<Word> { self.bytecode }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_emit(input: &str, expected_bytecode: Vec<Word>) {
        let mut emitter = Emitter::new(input).unwrap();
        emitter.emit().unwrap();
        let instructions = emitter.to_bytecode();
        for (obtained, expected) in instructions.into_iter().zip(expected_bytecode) {
            assert_eq!(obtained, expected);
        }
    }

    #[test]
    fn simple_addition() {
        expect_emit("3;", vec![Word::int(3, OpCode::Value)]);
        expect_emit("3+2;", vec![Word::int(3, OpCode::Value), Word::int(2, OpCode::Add)]);
    }
}
