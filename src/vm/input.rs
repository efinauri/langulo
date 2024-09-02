use crate::vm::garbage_collector::GarbageCollector;
use crate::vm::VM;
use crate::word::structure::Word;
use std::collections::VecDeque;
use std::io;
use std::io::Read;

fn read_bytes<R: Read>(reader: &mut R, buffer: &mut [u8]) -> io::Result<()> {
    reader.read_exact(buffer)?;
    Ok(())
}
fn read_u64<R: Read>(reader: &mut R) -> io::Result<u64> {
    let mut buf = [0u8; 8];
    read_bytes(reader, &mut buf)?;
    Ok(u64::from_le_bytes(buf))
}
fn read_f64<R: Read>(reader: &mut R) -> io::Result<f64> {
    let mut buf = [0u8; 8];
    read_bytes(reader, &mut buf)?;
    println!("float bytes: {:?}", buf);
    Ok(f64::from_le_bytes(buf))
}
fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    read_bytes(reader, &mut buf)?;
    Ok(u32::from_le_bytes(buf))
}
fn read_vec<R: Read>(reader: &mut R, length: usize) -> io::Result<Vec<u8>> {
    let mut buffer = vec![0u8; length];
    read_bytes(reader, &mut buffer)?;
    Ok(buffer)
}

impl VM {
    pub fn new(bytecode: Vec<Word>, heap_floats: Vec<f64>, heap_strings: Vec<Option<String>>) -> Self {
        VM {
            bytecode,
            vars: VecDeque::new(),
            stack: VecDeque::new(),
            gc: GarbageCollector::new(),
            ip: 0,
            heap_floats,
            heap_strings,
        }
    }

    pub fn from_bytecode_only(bytecode: Vec<Word>) -> Self {
        VM {
            bytecode,
            stack: VecDeque::new(),
            vars: VecDeque::new(),
            gc: GarbageCollector::new(),
            ip: 0,
            heap_floats: Vec::new(),
            heap_strings: Vec::new(),
        }
    }
    pub fn from_compiled_stream<R: Read>(mut reader: R) -> io::Result<Self> {
        let mut bytecode = Vec::new();
        let mut heap_floats = Vec::new();
        let mut heap_strings = Vec::new();

        let mut section_id = [0u8; 1];
        reader.read_exact(&mut section_id)?;
        assert_eq!(section_id[0], 0x01, "Invalid compiled stream");

        let bytecode_len = read_u32(&mut reader)? as usize;
        for _ in 0..bytecode_len {
            let word = read_u64(&mut reader)?;
            bytecode.push(Word::from_u64(word));
        }
        reader.read_exact(&mut section_id)?;
        assert_eq!(section_id[0], 0x02, "Invalid compiled stream");
        let floats_len = read_u32(&mut reader)? as usize;
        for _ in 0..floats_len {
            let float = read_f64(&mut reader)?;
            println!("float: {}", float);
            heap_floats.push(float);
        }
        reader.read_exact(&mut section_id)?;
        assert_eq!(section_id[0], 0x03, "Invalid compiled stream");

        let num_strings = read_u32(&mut reader)? as usize;
        for _ in 0..num_strings {
            let str_len = read_u32(&mut reader)? as usize;
            let str_data = read_vec(&mut reader, str_len)?;
            let string = String::from_utf8(str_data).expect("Invalid UTF-8 data");
            heap_strings.push(Some(string));
        }

        reader.read_exact(&mut section_id)?;
        assert_eq!(section_id[0], 0x04, "Invalid compiled stream");
        let num_vars = read_u32(&mut reader)? as usize;

        #[feature(test)] {
            println!("spinning up the vm with these raw heap maps:");
            println!("heap floats: {:?}", heap_floats);
            println!("heap strings: {:?}", heap_strings);
        }

        Ok(Self {
            bytecode,
            vars: VecDeque::with_capacity(num_vars),
            stack: VecDeque::new(),
            gc: GarbageCollector::new(),
            ip: 0,
            heap_floats,
            heap_strings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emitter::Emitter;
    #[test]
    fn emit_stream_with_heap_allocs() {
        let mut emitter = Emitter::new(r#"
        3.3 + 5.6;
        "#).expect("could not emit");
        emitter.emit().unwrap();
        let mut buf = vec![];
        emitter.write_to_stream(&mut buf).expect("could not write to stream");
        let mut cursor = io::Cursor::new(buf);
        let mut vm = VM::from_compiled_stream(&mut cursor).expect("failed to spin vm up from stream");
        vm.run().expect("error while running");
        let result = vm.finalize();
        assert_eq!(result.to_float(), 3.3 + 5.6);
    }

    use crate::word::structure::{OpCode, ValueTag};

    #[test]
    fn from_emitted_stream() {
        let mut emitter = Emitter::new(r#"
        2 + 3 * 4;
        "#).expect("could not emit");
        emitter.emit().unwrap();
        let mut buf = vec![];
        emitter.write_to_stream(&mut buf).expect("could not write to stream");
        let mut cursor = io::Cursor::new(buf);
        let mut vm = VM::from_compiled_stream(&mut cursor).expect("failed to spin vm up from stream");
        vm.run().expect("error while running");
        let result = vm.finalize();
        assert_eq!(result, Word::int(14, OpCode::Value));
    }

    #[test]
    fn emit_option() {
        let mut emitter = Emitter::new(r#"
        3??;
        "#).expect("could not emit");
        emitter.emit().unwrap();
        let mut buf = vec![];
        emitter.write_to_stream(&mut buf).expect("could not write to stream");
        let mut cursor = io::Cursor::new(buf);
        let mut vm = VM::from_compiled_stream(&mut cursor).expect("failed to spin vm up from stream");
        vm.run().expect("error while running");

        let result = vm.finalize();
        assert_eq!(result.tag(), ValueTag::OptionPtr);
        let result = result.as_option();
        assert!(result.is_some());
        println!("result:\n {:?}", result);

        let inner = result.expect("expected a nested value");
        assert_eq!(inner.tag(), ValueTag::OptionPtr);
        let inner = inner.as_option();
        // assert!(inner.is_some());
        // println!("inner:\n {:?}", inner);

        // let integer = inner.expect("expected a number");
        // assert_eq!(integer, Word::int(3, OpCode::Value));
    }
}