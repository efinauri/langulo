use crate::vm::garbage_collector::GarbageCollector;
use crate::word::heap::{HeapFloat, HeapStr, HeapTable, HeapValue};
use crate::word::structure::{OpCode, ValueTag, Word};
use std::cmp::Ordering;
use std::collections::BTreeMap;

// constructors
impl Word {
    pub fn bool(value: bool, opcode: OpCode) -> Self {
        Self::new(value as u8 as _, false, false, opcode, ValueTag::Bool)
    }

    pub fn int(value: i32, opcode: OpCode) -> Self {
        Self::new(value as _, false, false, opcode, ValueTag::Int)
    }

    pub fn char(value: char, opcode: OpCode) -> Self {
        Self::new(value as u8 as _, false, false, opcode, ValueTag::Char)
    }

    pub fn float(value: f64, opcode: OpCode, gc: &mut GarbageCollector) -> Self {
        let ptr = HeapFloat::write(value, opcode);
        gc.trace(ptr);
        ptr
    }
    
    pub fn raw_float(pointer_to_float_map: u32) -> Self {
        Self::new(pointer_to_float_map as _, false, false, OpCode::ReadFromMap, ValueTag::FloatPtr)
    }

    pub fn str(value: &str, gc: &mut GarbageCollector, opcode: OpCode) -> Self {
        let ptr = unsafe { HeapStr::write(value.to_string(), opcode) };
        gc.trace(ptr);
        ptr
    }

    pub fn raw_str(pointer_to_str_map: u32) -> Self {
        Self::new(pointer_to_str_map as _, false, false, OpCode::ReadFromMap, ValueTag::StrPtr)
    }

    pub fn table(value: BTreeMap<Word, Word>, gc: &mut GarbageCollector, opcode: OpCode) -> Self {
        let ptr = HeapTable::write(value, opcode);
        gc.trace(ptr);
        ptr
    }

    pub fn raw_table(pointer_to_table_map: u32) -> Self {
        Self::new(pointer_to_table_map as _, false, false, OpCode::ReadFromMap, ValueTag::TablePtr)
    }
}

// finalizers
impl Word {
    pub fn to_bool(self) -> bool {
        debug_assert_eq!(self.tag(), ValueTag::Bool);
        self.value() == 1
    }

    pub fn to_int(self) -> i32 {
        debug_assert_eq!(self.tag(), ValueTag::Int);
        self.value() as i32
    }

    pub fn to_char(self) -> char {
        debug_assert_eq!(self.tag(), ValueTag::Char);
        self.value() as u8 as char
    }

    pub fn to_float(self) -> f64 {
        debug_assert!(self.in_heap());
        debug_assert_eq!(self.tag(), ValueTag::FloatPtr);
        unsafe { *HeapFloat::read(&self) }
    }

    pub fn as_str(&self) -> &str {
        debug_assert!(self.in_heap());
        debug_assert_eq!(self.tag(), ValueTag::StrPtr);
        unsafe { HeapStr::read(&self) }
    }

    pub fn as_str_mut(&mut self) -> &mut String {
        debug_assert!(self.in_heap());
        debug_assert_eq!(self.tag(), ValueTag::StrPtr);
        unsafe { &mut self.get_mut::<HeapStr>().0 }
    }

    pub fn as_table(&self) -> &BTreeMap<Word, Word> {
        debug_assert!(self.in_heap());
        assert_eq!(self.tag(), ValueTag::TablePtr);
        unsafe { HeapTable::read(&self) }
    }

    pub fn as_table_mut(&mut self) -> &mut BTreeMap<Word, Word> {
        debug_assert!(self.in_heap());
        assert_eq!(self.tag(), ValueTag::TablePtr);
        unsafe { &mut self.get_mut::<HeapTable>().0 }
    }

    pub fn free(self) {
        unsafe {
            match self.tag() {
                ValueTag::FloatPtr => HeapFloat::destroy(self),
                ValueTag::StrPtr => HeapStr::destroy(self),
                ValueTag::TablePtr => HeapTable::destroy(self),
                _ => (),
            }
        }
    }
}

impl PartialEq for Word {
    fn eq(&self, other: &Self) -> bool {
        self.tag() == other.tag()
            && match self.tag() {
                ValueTag::Int | ValueTag::Bool | ValueTag::Char => self.value() == other.value(),
                ValueTag::FloatPtr => self.to_float() == other.to_float(),
                ValueTag::StrPtr => self.as_str() == other.as_str(),
                _ => unimplemented!("no partialeq impl for tag {:?}", self.tag()),
            }
    }
}
impl PartialOrd for Word {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        debug_assert_eq!(self.tag(), other.tag());
        match self.tag() {
            ValueTag::Int | ValueTag::Bool | ValueTag::Char => self.value().partial_cmp(&other.value()),
            ValueTag::FloatPtr => self.to_float().partial_cmp(&other.to_float()),
            ValueTag::StrPtr => self.as_str().partial_cmp(&other.as_str()),
            _ => unimplemented!("no partialord impl for tag {:?}", self.tag()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int() {
        let w = Word::int(2345, OpCode::Value);
        println!("{:?}", w);
        assert_eq!(w.tag(), ValueTag::Int);
        assert_eq!(w.to_int(), 2345);
        let w = Word::int(-21, OpCode::Value);
        println!("{:?}", w);
        assert_eq!(w.tag(), ValueTag::Int);
        assert_eq!(w.to_int(), -21);
        let w = Word::int(-21, OpCode::Add);
        println!("{:?}", w);
        assert_eq!(w.tag(), ValueTag::Int);
        assert_eq!(w.opcode(), OpCode::Add);
        assert_eq!(w.to_int(), -21);

    }

    #[test]
    fn bools() {
        let w = Word::bool(true, OpCode::Value);
        println!("{:?}", w);
        assert_eq!(w.tag(), ValueTag::Bool);
        assert_eq!(w.to_bool(), true);

        let w = Word::bool(false, OpCode::Value);
        println!("{:?}", w);
        assert_eq!(w.tag(), ValueTag::Bool);
        assert_eq!(w.to_bool(), false);
    }

    #[test]
    fn chars() {
        for ch in "Hello, world!".chars() {
            let w = Word::char(ch, OpCode::Value);
            println!("{:?}", w);
            assert_eq!(w.tag(), ValueTag::Char);
            assert_eq!(w.to_char(), ch);
        }
    }

    #[test]
    fn float() {
        let mut gc = GarbageCollector::new();
        let w = Word::float(3.14, OpCode::Value, &mut gc);
        println!("{:?}", w);
        assert_eq!(w.to_float(), 3.14);
        let w = Word::float(-2.7181, OpCode::Value, &mut gc);
        println!("{:?}", w);
        assert_eq!(w.to_float(), -2.7181);
        let w = Word::float(0.0, OpCode::Add, &mut gc);
        println!("{:?}", w);
        assert_eq!(w.opcode(), OpCode::Add);
        assert_eq!(w.to_float(), 0.0);
    }

    #[test]
    fn string() {
        let mut gc = GarbageCollector::new();
        let w = Word::str("Hello, world!", &mut gc, OpCode::Value);
        println!("{:?}", w);
        assert_eq!(w.as_str(), "Hello, world!");

        let mut w_mut = Word::str("Hello", &mut gc, OpCode::Value);
        w_mut.as_str_mut().push_str(", world!");
        assert_eq!(w_mut.as_str(), "Hello, world!");
    }

    #[test]
    fn table() {
        let mut gc = GarbageCollector::new();
        let mut table = BTreeMap::new();
        table.insert(Word::int(1, OpCode::Value), Word::str("hello", &mut gc, OpCode::Value));
        table.insert(Word::int(2, OpCode::Value), Word::str("world", &mut gc, OpCode::Value));

        let mut w = Word::table(table, &mut gc, OpCode::Value);
        println!("{:?}", w);
        assert_eq!(w.as_table().len(), 2);
        assert_eq!(
            w.as_table()
                .get(&Word::int(1, OpCode::Value))
                .unwrap()
                .as_str(),
            "hello"
        );
        assert_eq!(
            w.as_table()
                .get(&Word::int(2, OpCode::Value))
                .unwrap()
                .as_str(),
            "world"
        );
        w.as_table_mut().remove(&Word::int(1, OpCode::Value));
        assert_eq!(w.as_table().len(), 1);
    }
}
