#[cfg(test)]
mod end_to_end_tests {
    use crate::emitter::Emitter;
    use crate::vm::VM;
    use crate::word::structure::{OpCode, Word};

    fn expect_vm_output(input: &str, expected_output: Word) {
        let mut emitter = Emitter::new(input).unwrap();
        emitter.emit().unwrap();
        let instructions = emitter.to_bytecode();
        let mut vm = VM::new(instructions);
        vm.run().unwrap();
        assert_eq!(expected_output, vm.finalize());
    }

    #[test]
    fn test_addition() {
        expect_vm_output("3+2;", Word::int(5, OpCode::Value));
    }
}
