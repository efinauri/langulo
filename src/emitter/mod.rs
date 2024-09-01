use crate::errors::err::LanguloErr;
use crate::parser::ast::lang::LanguloSyntaxNode;
use crate::parser::ast::node::AstNode;
use crate::parser::Parser;
use crate::typecheck::TypeChecker;
use crate::word::heap::{encode_table, Table};
use crate::word::structure::{OpCode, Word};
use num_traits::ToBytes;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::Path;

macro_rules! cast_node {
    ($node:expr, $typ:ident) => {
        $node.text().to_string().parse::<$typ>()
           .map_err(|_|
            LanguloErr::semantic("expected node to be of type")
            )
    }
}

macro_rules! emit_binary {
    ($self:expr, $node:expr, $opcode:ident) => {paste::paste! {{
        let lhs = $self.emit_node(&$node.first_child().unwrap())?;
        $self.bytecode.push(lhs);
        let mut rhs = $self.emit_node(&$node.last_child().unwrap())?;
        if rhs.is_embeddable() {
            rhs.change_opcode(OpCode::[<$opcode This>]);
        } else {
            $self.bytecode.push(rhs);
            return Ok(Word::int(0, OpCode::$opcode));
        }
        Ok(rhs)
    }}};
}

macro_rules! emit_unary {
    ($self:expr, $node:expr, $opcode:ident) => {paste::paste! {{
        let mut operand = $self.emit_node(&$node.first_child().unwrap())?;
        if operand.is_embeddable() {
            operand.change_opcode(OpCode::[<$opcode This>]);
        } else {
            $self.bytecode.push(operand);
            return Ok(Word::int(0, OpCode::$opcode));
        }
        Ok(operand)
    }}};
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
    heap_floats: Vec<f64>,
    heap_strings: Vec<String>,
    heap_tables: Vec<Vec<u64>>, // tables are serialized (just a sequence of key/value words)
}

impl Emitter {
    pub fn new(input: &str) -> Result<Self, LanguloErr> {
        let mut parser = Parser::new(input);
        parser.parse()?;
        let ast_root = parser.to_ast();
        let mut type_checker = TypeChecker::new();
        // type_checker.typecheck(&ast_root)?;

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
                self.heap_floats.push(cast_node!(node, f64)?);
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

            AstNode::Add => emit_binary!(self, node, Add),
            AstNode::Subtract => emit_binary!(self, node, Subtract),
            AstNode::Multiply => emit_binary!(self, node, Multiply),
            AstNode::Divide => emit_binary!(self, node, Divide),
            AstNode::Modulo => emit_binary!(self, node, Modulo),
            AstNode::LogicalAnd => emit_binary!(self, node, LogicalAnd),
            AstNode::LogicalOr => emit_binary!(self, node, LogicalOr),
            AstNode::LogicalXor => emit_binary!(self, node, LogicalXor),
            AstNode::Print => emit_unary!(self, node, Print),
            _ => unimplemented!("todo: emit node type {:?}", node),
        }
    }

    #[cfg(test)]
    pub fn to_bytecode(self) -> Vec<Word> { self.bytecode }

    pub fn write_to_stream<W: Write>(&self, mut writer: W) -> io::Result<()> {
        #[cfg(test)] {
            println!("will write the following heap values:");
            println!("floats: {:?}", self.heap_floats);
            println!("tables: {:?}", self.heap_tables);
            println!("strings: {:?}", self.heap_strings);
        }
        // writing the len of everything so that the parsing can be exact
        writer.write_all(&[0x01])?;
        let bytecode_len = self.bytecode.len() as u32;
        writer.write_all(&bytecode_len.to_le_bytes())?;
        for word in &self.bytecode {
            writer.write_all(&(word.0 as u64).to_le_bytes())?;
        }

        writer.write_all(&[0x02])?;
        let floats_len = self.heap_floats.len() as u32;
        writer.write_all(&floats_len.to_le_bytes())?;
        for float in &self.heap_floats {
            writer.write_all(&float.to_le_bytes())?;
        }
        writer.write_all(&[0x03])?;
        let num_tables = self.heap_tables.len() as u32;
        writer.write_all(&num_tables.to_le_bytes())?;
        for table in &self.heap_tables {
            writer.write_all(&(table.len() as u32).to_le_bytes())?;
            for &entry in table {
                writer.write_all(&entry.to_le_bytes())?;
            }
        }
        writer.write_all(&[0x04])?;
        let num_strings = self.heap_strings.len() as u32;
        writer.write_all(&num_strings.to_le_bytes())?;
        for string in &self.heap_strings {
            let bytes = string.as_bytes();
            writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
            writer.write_all(bytes)?;
        }

        Ok(())
    }

    pub fn write_to_file(&self, path: &Path) -> io::Result<()> {
        let file = File::create(path)?;
        self.write_to_stream(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_emit(input: &str, expected_bytecode: Vec<Word>) {
        let mut emitter = Emitter::new(input).unwrap();
        emitter.emit().unwrap();
        let instructions = emitter.to_bytecode();
        assert_eq!(instructions.len(), 1 + expected_bytecode.len(),
                   "got this bytecode: {:?}", instructions); // taking into account the stop instruction
        for (obtained, expected) in instructions.into_iter().zip(expected_bytecode) {
            assert_eq!(obtained, expected);
            assert_eq!(obtained.tag(), expected.tag());
            assert_eq!(obtained.opcode(), expected.opcode());
        }
    }

    #[test]
    fn simple_addition() {
        expect_emit("3;", vec![Word::int(3, OpCode::Value)]);
        expect_emit("3+2;", vec![Word::int(3, OpCode::Value), Word::int(2, OpCode::AddThis)]);
        expect_emit("2+3*4;", vec![
            Word::int(2, OpCode::Value),
            Word::int(3, OpCode::Value),
            Word::int(4, OpCode::MultiplyThis),
            Word::int(0, OpCode::Add),
        ]);
        // todo could write in this optimization
        expect_emit("3*4+2;", vec![ // 1 less instruction!
            Word::int(3, OpCode::Value),
            Word::int(4, OpCode::MultiplyThis),
            Word::int(2, OpCode::AddThis),
        ]);
    }
}
