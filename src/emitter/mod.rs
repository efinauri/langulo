use crate::errors::err::LanguloErr;
use crate::parser::ast::lang::LanguloSyntaxNode;
use crate::parser::ast::node::AstNode;
use crate::parser::Parser;
use crate::typecheck::TypeChecker;
use crate::word::heap::Table;
use crate::word::structure::{OpCode, ValueTag, Word};
use num_traits::ToBytes;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;

macro_rules! cast_node {
    ($node:expr, $typ:ident) => {
        $node.text().to_string().parse::<$typ>()
           .map_err(|_|
            LanguloErr::semantic("expected node to be of type")
            )
    }
}

macro_rules! push_embeddable {
    ($self:expr, $word:expr, $opcode:ident) => { paste::paste! {{
        if $word.is_embeddable() {
            $word.set_opcode(OpCode::[<$opcode This>]);
            $word
        }
        else {
            $self.bytecode.push($word);
            Word::int(0, OpCode::$opcode)
        }
    }}};
}

macro_rules! emit_binary {
    ($self:expr, $node:expr, $opcode:ident) => {{
        let lhs = $self.emit_node(&$node.first_child().unwrap())?;
        $self.bytecode.push(lhs);
        let mut rhs = $self.emit_node(&$node.last_child().unwrap())?;
        Ok(push_embeddable!($self, rhs, $opcode))
    }};
}

macro_rules! emit_unary {
    ($self:expr, $node:expr, $opcode:ident) => {{
        let mut operand = $self.emit_node(&$node.first_child().unwrap())?;
        Ok(push_embeddable!($self, operand, $opcode))
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
    heap_floats: Vec<f64>,
    heap_strings: Vec<String>,

    local_variables: Vec<LocalVarInfo>,
    curr_scope: usize,
}

#[derive(Debug)]
struct LocalVarInfo {
    name: String,
    scope: usize,
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
            local_variables: Vec::new(),
            curr_scope: 0,
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
            AstNode::Bool => Ok(Word::bool(cast_node!(node, bool)?, OpCode::Value)),
            AstNode::Char => Ok(Word::char(cast_node!(node, char)?, OpCode::Value)),
            AstNode::Float => {
                self.heap_floats.push(cast_node!(node, f64)?);
                Ok(Word::raw_float((self.heap_floats.len() - 1) as u32))
            }
            AstNode::Str => {
                self.heap_strings.push(cast_node!(node, String)?);
                Ok(Word::new(0 as _, OpCode::ReadFromMap, ValueTag::StrPtr))
            }
            AstNode::Table => {
                for pair in node.children() {
                    debug_assert_eq!(pair.kind(), AstNode::TablePair);
                    debug_assert_eq!(pair.children().count(), 2);
                    let key_word = self.emit_node(&pair.first_child().unwrap())?;
                    self.bytecode.push(key_word);
                    let value_word = self.emit_node(&pair.last_child().unwrap())?;
                    self.bytecode.push(value_word);
                }
                Ok(Word::new(node.children().count() as _, OpCode::ReadFromMap, ValueTag::TablePtr))
            }
            AstNode::TableIndexing => {
                debug_assert_eq!(node.children().count(), 2);
                let indexand = self.emit_node(&node.first_child().unwrap())?;
                self.bytecode.push(indexand);
                let mut indexer = self.emit_node(&node.last_child().unwrap())?;
                Ok(push_embeddable!(self, indexer, IndexGet))
            }
            AstNode::DefaultKey => Ok(Word::DEFAULTTABLEARM()),
            AstNode::Option => {
                let mut inner = node.first_child()
                    .map(|inner| self.emit_node(&inner))
                    .transpose()?
                    .unwrap_or(Word::NOOPTION());
                Ok(push_embeddable!(self, inner, WrapInOption))
            }
            AstNode::UnwrapOption => {
                let mut inner = self.emit_node(&node.first_child().unwrap())?;
                Ok(push_embeddable!(self, inner, UnwrapOption))
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
            AstNode::Scope => {
                self.curr_scope += 1;
                for child in node.children() {
                    let child_word = self.emit_node(&child)?;
                    self.bytecode.push(child_word);
                }
                self.curr_scope -= 1;
                Ok(Word::int(0, OpCode::Print))
            }
            AstNode::Grouping => Ok(self.emit_node(&node.first_child().unwrap())?),
            AstNode::Identifier => {
                let ident_name = node.text().to_string();
                let index = self.local_variables.iter().rposition(|el| el.name == ident_name);
                let mut ident_word = Word::int(0, OpCode::GetLocal);
                ident_word.set_aux(index.expect(&*format!("did not find varname in already defined vars. \nvars: {:?}", &self.local_variables)) as u32);
                Ok(ident_word)
            }
            AstNode::VarDecl => {
                let var_name = node.text().to_string().split_whitespace().next().unwrap().to_string();

                debug_assert!(
                    !self.local_variables.iter()
                    .any(|var| var.name == var_name && var.scope == self.curr_scope),
                );
                self.local_variables.push(LocalVarInfo {
                    name: var_name,
                    scope: self.curr_scope,
                });
                // first_child could be the type hint
                let mut decl_word = self.emit_node(&node.last_child().unwrap())?;
                if decl_word.is_embeddable() {
                    decl_word.set_opcode(OpCode::SetLocalThis);
                    Ok(decl_word)
                } else {
                    self.bytecode.push(decl_word);
                    Ok(Word::int(0, OpCode::SetLocal))
                }
            }
            AstNode::If => {
                let condition = self.emit_node(&node.first_child().unwrap())?;
                self.bytecode.push(condition);

                let jump_idx = self.bytecode.len();
                let jump_word = Word::int(0, OpCode::JumpIfFalse);
                self.bytecode.push(jump_word);

                let len_before_branch = self.bytecode.len();
                let mut branch = self.emit_node(&node.last_child().unwrap())?;
                let instructions_to_jump = self.bytecode.len() - len_before_branch + 2;

                self.bytecode.get_mut(jump_idx).unwrap().set_value(instructions_to_jump as u32);

                Ok(push_embeddable!(self, branch, WrapInOption))
            }
            AstNode::Else => {
                let option = self.emit_node(&node.first_child().unwrap())?;
                self.bytecode.push(option);

                let jump_idx = self.bytecode.len();
                let jump_word = Word::int(0, OpCode::JumpIfNo);
                self.bytecode.push(jump_word);

                let len_before_branch = self.bytecode.len();
                let branch = self.emit_node(&node.last_child().unwrap())?;
                // +1 instead of +2 because we don't "post-process" the branch like we did in "if"
                let instructions_to_jump = self.bytecode.len() - len_before_branch + 1;

                self.bytecode.get_mut(jump_idx).unwrap().set_value(instructions_to_jump as u32);
                Ok(branch)
            }


            _ => unimplemented!("todo: emit node type {:?}", node),
        }
    }

    #[cfg(test)]
    pub fn to_bytecode(self) -> Vec<Word> { self.bytecode }

    pub fn write_to_stream<W: Write>(&self, mut writer: W) -> io::Result<()> {
        debug_assert!(self.bytecode.len() > 0, "did not call emit() before writing to stream");
        #[cfg(test)] {
            println!("will write the following heap values:");
            println!("floats: {:?}", self.heap_floats);
            println!("strings: {:?}", self.heap_strings);
        }
        // writing the len of everything so that the parsing can be exact
        // writer.write_all(&[0xED, 0x0C, 0x0D, 0xED])?; // magic number
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
        let num_strings = self.heap_strings.len() as u32;
        writer.write_all(&num_strings.to_le_bytes())?;
        for string in &self.heap_strings {
            let bytes = string.as_bytes();
            writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
            writer.write_all(bytes)?;
        }

        writer.write_all(&[0x04])?;
        let num_vars = self.local_variables.len() as u32;
        writer.write_all(&num_vars.to_le_bytes())?;

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

    #[test]
    fn variable_decl() {
        expect_emit("var x = 3; x = 4;", vec![
            Word::int(3, OpCode::SetLocalThis),
            Word::int(4, OpCode::SetLocalThis),
        ]);
    }

    #[test]
    fn options() {
        // expect_emit("3??;")
    }
}
