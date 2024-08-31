use crate::errors::err::LanguloErr;
use crate::vm::garbage_collector::GarbageCollector;
use crate::word::structure::{OpCode, Word};
use std::collections::VecDeque;

pub mod garbage_collector;

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
        debug_assert_eq!(back.opcode(), OpCode::Value);
        back
    }

    pub fn run(&mut self) -> Result<(), LanguloErr> {
        loop {
            let current = &self.bytecode[self.ip];
            #[feature(debug)] {
                println!("running bytecode [{}]: {:?}", self.ip, current);
            }
            self.ip += 1;
            debug_assert!(self.ip <= self.bytecode.len());
            match current.opcode() {
                OpCode::Stop => break,
                OpCode::Value => self.stack.push_back(*current),
                OpCode::Print => println!("(TEMPORARY PRINT) {:?}", self.stack.back().unwrap()), // todo impl Display and show that

                OpCode::Add => self.stack.back_mut().unwrap().add_inplace(current)?,
                OpCode::Subtract => self.stack.back_mut().unwrap().subtract_inplace(current)?,
                OpCode::Multiply => self.stack.back_mut().unwrap().multiply_inplace(current)?,
                OpCode::Divide => self.stack.back_mut().unwrap().divide_inplace(current)?,
                OpCode::Power => self.stack.back_mut().unwrap().exponentiate_inplace(current, &mut self.gc)?,
                OpCode::Modulo => self.stack.back_mut().unwrap().modulo_inplace(current)?,

                OpCode::Negate => self.stack.push_back(Word::bool(!current.to_bool(), OpCode::Value)),
                OpCode::LogicalAnd => self.stack.back_mut().unwrap().logical_and_inplace(current)?,
                OpCode::LogicalOr => self.stack.back_mut().unwrap().logical_or_inplace(current)?,
                OpCode::LogicalXor => self.stack.back_mut().unwrap().logical_xor_inplace(current)?,

                OpCode::Equals => self.stack.back_mut().unwrap().equals_inplace(current)?,
                OpCode::NotEquals => self.stack.back_mut().unwrap().not_equals_inplace(current)?,

                OpCode::GreaterThan => self.stack.back_mut().unwrap().greater_than_inplace(current)?,
                OpCode::LessThan => self.stack.back_mut().unwrap().less_than_inplace(current)?,
                OpCode::GreaterThanEq => self.stack.back_mut().unwrap().greater_than_eq_inplace(current)?,
                OpCode::LessThanEq => self.stack.back_mut().unwrap().less_than_eq_inplace(current)?,

                // OpCode::SetLocal
                // OpCode::GetLocal

                _ => unimplemented!("opcode not implemented: {:?}", current.opcode()),
            }
        }
        Ok(())
    }

    pub fn finalize(mut self) -> Word {
        debug_assert_eq!(self.stack.len(), 1);
        self.pop_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::structure::Word;
    fn expect_float_vm_execution_approx(lhs: f32, rhs: f32, op: OpCode, expected_output: f32) {
        let mut gc = GarbageCollector::new();
        let bytecode = vec![
            Word::float(lhs, OpCode::Value, &mut gc),
            Word::float(rhs, op, &mut gc),
            Word::int(0, OpCode::Stop)
        ];
        let mut vm = VM::new(bytecode);
        vm.run().unwrap();
        let result = vm.finalize();
        println!("{:?}", result);
        let result_flt = result.to_float();
        assert!((result_flt - expected_output).abs() < 0.00001,
        "Result: {}, Expected: {}", result_flt, expected_output);
    }

    #[test]
    fn float_arithmetic_tests() {
        expect_float_vm_execution_approx(3.0, 5.0, OpCode::Add, 8.0);
        expect_float_vm_execution_approx(3.0, 5.0, OpCode::Subtract, -2.0);
        expect_float_vm_execution_approx(3.0, 5.0, OpCode::Multiply, 15.0);
        expect_float_vm_execution_approx(3.0, 5.0, OpCode::Divide, 0.6);
    }

    fn expect_vm_execution(mut bytecode: Vec<Word>, expected_output: Word) {
        bytecode.push(Word::int(0, OpCode::Stop));
        let mut vm = VM::new(bytecode);
        vm.run().unwrap();
        assert_eq!(vm.finalize(), expected_output);
    }

    #[test]
    fn vm_arithmetic() {
        expect_vm_execution(
            vec![Word::bool(true, OpCode::Negate)],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![Word::int(3, OpCode::Value), Word::int(5, OpCode::Add)],
            Word::int(8, OpCode::Value),
        );
        expect_vm_execution(
            vec![Word::int(3, OpCode::Value), Word::int(5, OpCode::Subtract)],
            Word::int(-2, OpCode::Value),
        );
        expect_vm_execution(
            vec![Word::int(3, OpCode::Value), Word::int(5, OpCode::Multiply)],
            Word::int(15, OpCode::Value),
        );
    }

    #[test]
    fn vm_logic() {
        expect_vm_execution(
            vec![
                Word::bool(true, OpCode::Value),
                Word::bool(false, OpCode::LogicalAnd),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::bool(true, OpCode::Value),
                Word::bool(false, OpCode::LogicalOr),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::bool(true, OpCode::Value),
                Word::bool(false, OpCode::LogicalXor),
            ],
            Word::bool(true, OpCode::Value),
        );
    }

    #[test]
    fn vm_comparisons() {
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::Equals),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(3, OpCode::Equals),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::NotEquals),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::GreaterThan),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(5, OpCode::Value),
                Word::int(3, OpCode::GreaterThan),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::LessThan),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(5, OpCode::Value),
                Word::int(3, OpCode::LessThan),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::GreaterThanEq),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(5, OpCode::Value),
                Word::int(3, OpCode::GreaterThanEq),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(3, OpCode::GreaterThanEq),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(5, OpCode::LessThanEq),
            ],
            Word::bool(true, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(5, OpCode::Value),
                Word::int(3, OpCode::LessThanEq),
            ],
            Word::bool(false, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(3, OpCode::Value),
                Word::int(3, OpCode::LessThanEq),
            ],
            Word::bool(true, OpCode::Value),
        );
    }

    #[test]
    fn vm_power() {
        let mut gc = GarbageCollector::new();
        expect_vm_execution(
            vec![
                Word::int(2, OpCode::Value),
                Word::int(3, OpCode::Power),
            ],
            Word::float(8.0, OpCode::Value, &mut gc),
        );
        expect_vm_execution(
            vec![
                Word::float(2.0, OpCode::Value, &mut gc),
                Word::float(3.0, OpCode::Power, &mut gc),
            ],
            Word::float(8.0, OpCode::Value, &mut gc),
        );
        expect_vm_execution(
            vec![
                Word::int(2, OpCode::Value),
                Word::float(3.0, OpCode::Power, &mut gc),
            ],
            Word::float(8.0, OpCode::Value, &mut gc),
        );
        expect_float_vm_execution_approx(8.0, 0.33333333, OpCode::Power, 2.0);
        expect_float_vm_execution_approx(4.0, -2.0, OpCode::Power, 0.0625)
    }

    #[test]
    fn vm_modulo() {
        expect_vm_execution(
            vec![
                Word::int(7, OpCode::Value),
                Word::int(3, OpCode::Modulo),
            ],
            Word::int(1, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(10, OpCode::Value),
                Word::int(3, OpCode::Modulo),
            ],
            Word::int(1, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(-7, OpCode::Value),
                Word::int(3, OpCode::Modulo),
            ],
            Word::int(-1, OpCode::Value),
        );
        expect_vm_execution(
            vec![
                Word::int(-10, OpCode::Value),
                Word::int(3, OpCode::Modulo),
            ],
            Word::int(-1, OpCode::Value),
        );

        expect_float_vm_execution_approx(7.2, 3.3, OpCode::Modulo, 0.6);
        expect_float_vm_execution_approx(7.2, -3.3, OpCode::Modulo, 0.6);
        expect_float_vm_execution_approx(7.2, 55.4, OpCode::Modulo, 7.2);
        expect_float_vm_execution_approx(7.2, -55.4, OpCode::Modulo, 7.2);
    }


}
