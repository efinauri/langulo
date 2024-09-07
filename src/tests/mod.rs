#[cfg(test)]
mod end_to_end_tests {
    use std::io;
    use crate::emitter::Emitter;
    use crate::vm::VM;
    use crate::word::structure::{OpCode, Word};

    fn expect_vm_output(input: &str, expected_output: &str) {
        let mut emitter = Emitter::new(input).unwrap();
        emitter.emit().unwrap();
        let mut buf = vec![];
        emitter.write_to_stream(&mut buf).expect("could not write to stream");
        let mut cursor = io::Cursor::new(buf);
        let mut vm = VM::from_compiled_stream(&mut cursor).expect("failed to spin vm up from stream");
        vm.run().expect("error while running");
        assert_eq!(expected_output, vm.finalize().to_string());
    }

    #[test]
    fn test_addition() {
        expect_vm_output("3+2;", "5");
    }

    #[test]
    fn test_if() {
        expect_vm_output("if true {2};", "2?");
        expect_vm_output("if false {2};", "no");
        expect_vm_output("if false {2}; 3;", "3");
    }

    #[test]
    fn test_else() {
        expect_vm_output("no else {3};", "3");
        expect_vm_output("2? else {3};", "2");
    }
}
