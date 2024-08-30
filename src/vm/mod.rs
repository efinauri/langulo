use crate::errors::err::LanguloErr;
use crate::vm::garbage_collector::GarbageCollector;
use crate::vm::word::word_shape::{OpCode, Word};
use std::collections::VecDeque;

pub mod garbage_collector;
pub mod word;

// macro_rules! run_binary {
//     ($vm:expr, $op:expr) => {{
//         debug_assert!($vm.stack.len() >= 1);
//         let lhs = $vm.pop_value();
//         let result = $op(lhs)?;
//         $vm.stack.push_back(result)
//     }}
// }

pub struct VM {
    bytecode: Vec<Word>,
    stack: VecDeque<Word>,
    gc: GarbageCollector,
    ip: usize,
}

impl VM {
    pub fn new(bytecode: Vec<Word>) -> Self {
        VM {
            bytecode,
            stack: VecDeque::new(),
            gc: GarbageCollector::new(),
            ip: 0,
        }
    }

    pub fn pop_value(&mut self) -> Word {
        let back = self.stack.pop_back().expect("stack underflow");
        debug_assert_eq!(back.opcode(), OpCode::Constant);
        back
    }

    pub fn run(&mut self) -> Result<(), LanguloErr> {
        loop {
            let current = &self.bytecode[self.ip];
            self.ip += 1;
            debug_assert!(self.ip <= self.bytecode.len());
            match current.opcode() {
                OpCode::Stop => break,
                // OpCode::Negate => {
                //     let v = self.pop_value();
                //     self.stack.push_back(-v);
                // }
                OpCode::Constant => self.stack.push_back(*current),
                OpCode::Add => self
                    .stack
                    .back_mut()
                    .unwrap()
                    .add_inplace(current, &mut self.gc)?,

                // run_binary!(self, |lhs| current.add(lhs, &mut self.gc) ),
                // OpCode::Subtract => run_binary!(self, |a, b| a - b),
                // OpCode::Multiply => run_binary!(self, |a, b| a * b),
                OpCode::Print => println!("{:?}", self.stack.back().unwrap()),
                _ => unimplemented!(),
            }
        }
        Ok(())
    }

    pub fn finalize(mut self) -> Word {
        self.pop_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::word::word_shape::Word;

    // #[test]
    // fn negate() {
    //     let mut vm = VM::new(vec![
    //         Instruction::new(Constant, 5),
    //         Instruction::new(Negate, 0),
    //         Instruction::new(Return, 0)]);
    //     vm.run().unwrap();
    //     assert_eq!(vm.stack.pop_back().unwrap(), -5);
    // }

    #[test]
    fn add() {
        let mut vm = VM::new(vec![
            Word::int(3, OpCode::Constant),
            Word::int(5, OpCode::Add),
            Word::int(0, OpCode::Stop),
        ]);
        vm.run().unwrap();
        assert_eq!(vm.stack.pop_back().unwrap().to_int(), 8);
    }
}
