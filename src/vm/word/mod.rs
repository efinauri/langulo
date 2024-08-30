pub mod heap;
pub mod operations;
pub mod word_shape;

use crate::vm::garbage_collector::GarbageCollector;
use crate::vm::word::heap::{HeapFloat, HeapStr, HeapTable, HeapValue};
use crate::vm::word::word_shape::{OpCode, ValueTag, Word, PTR_MASK, PTR_START};
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

    pub fn float(value: f64, gc: &mut GarbageCollector, opcode: OpCode) -> Self {
        let ptr = unsafe { HeapFloat::write(value) };
        gc.trace(ptr);
        ptr
    }

    pub fn str(value: &str, gc: &mut GarbageCollector) -> Self {
        let ptr = unsafe { HeapStr::write(value.to_string()) };
        gc.trace(ptr);
        ptr
    }

    pub fn table(value: BTreeMap<Word, Word>, gc: &mut GarbageCollector) -> Self {
        let ptr = unsafe { HeapTable::write(value) };
        gc.trace(ptr);
        ptr
    }
}

// accessors
impl Word {
    pub fn in_heap(&self) -> bool {
        self.tag() > ValueTag::Char
    }

    unsafe fn get<'a, T>(self) -> &'a T {
        &*(self.ptr() as *const T)
    }

    unsafe fn get_mut<'a, T>(self) -> &'a mut T {
        &mut *(self.ptr() as *mut T)
    }

    fn value(&self) -> u64 {
        (self.0 as u64 & PTR_MASK) >> PTR_START
    }

    fn set_value(&mut self, value: u64) {
        self.0 = ((self.0 as u64 & !PTR_MASK) | (value << PTR_START)) as _
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
                ValueTag::Int | ValueTag::Bool | ValueTag::Char => self.0 == other.0,
                ValueTag::FnPtr => unimplemented!(),
                ValueTag::FloatPtr => self.to_float() == other.to_float(),
                ValueTag::StrPtr => self.as_str() == other.as_str(),
                ValueTag::TablePtr => unimplemented!(),
            }
    }
}
impl PartialOrd for Word {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        debug_assert_eq!(self.tag(), other.tag());
        match self.tag() {
            ValueTag::Int | ValueTag::Bool | ValueTag::Char => self.0.partial_cmp(&other.0),
            ValueTag::FnPtr => unimplemented!(),
            ValueTag::FloatPtr => self.to_float().partial_cmp(&other.to_float()),
            ValueTag::StrPtr => self.as_str().partial_cmp(&other.as_str()),
            ValueTag::TablePtr => unimplemented!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int() {
        let tv = Word::int(2345, OpCode::Constant);
        println!("{:?}", tv);
        assert_eq!(tv.tag(), ValueTag::Int);
        assert_eq!(tv.to_int(), 2345);
    }

    #[test]
    fn set_value() {
        let mut tv = Word::int(2345, OpCode::Constant);
        println!("{:?}", tv);
        tv.set_value(123);
        println!("{:?}", tv);
        assert_eq!(tv.to_int(), 123);
    }

    #[test]
    fn bools() {
        let tv = Word::bool(true, OpCode::Constant);
        println!("{:?}", tv);
        assert_eq!(tv.tag(), ValueTag::Bool);
        assert_eq!(tv.to_bool(), true);

        let tv = Word::bool(false, OpCode::Constant);
        println!("{:?}", tv);
        assert_eq!(tv.tag(), ValueTag::Bool);
        assert_eq!(tv.to_bool(), false);
    }

    #[test]
    fn chars() {
        for ch in "Hello, world!".chars() {
            let tv = Word::char(ch, OpCode::Constant);
            println!("{:?}", tv);
            assert_eq!(tv.tag(), ValueTag::Char);
            assert_eq!(tv.to_char(), ch);
        }
    }

    #[test]
    fn float() {
        let mut gc = GarbageCollector::new();
        let tv = Word::float(3.14, &mut gc, OpCode::Constant);
        println!("{:?}", tv);
        assert_eq!(tv.to_float(), 3.14);
    }

    #[test]
    fn string() {
        let mut gc = GarbageCollector::new();
        let tv = Word::str("Hello, world!", &mut gc);
        println!("{:?}", tv);
        assert_eq!(tv.as_str(), "Hello, world!");

        let mut tv_mut = Word::str("Hello", &mut gc);
        tv_mut.as_str_mut().push_str(", world!");
        assert_eq!(tv_mut.as_str(), "Hello, world!");
    }

    #[test]
    fn table() {
        let mut gc = GarbageCollector::new();
        let mut table = BTreeMap::new();
        table.insert(Word::int(1, OpCode::Constant), Word::str("hello", &mut gc));
        table.insert(Word::int(2, OpCode::Constant), Word::str("world", &mut gc));

        let mut tv = Word::table(table, &mut gc);
        println!("{:?}", tv);
        assert_eq!(tv.as_table().len(), 2);
        assert_eq!(
            tv.as_table()
                .get(&Word::int(1, OpCode::Constant))
                .unwrap()
                .as_str(),
            "hello"
        );
        assert_eq!(
            tv.as_table()
                .get(&Word::int(2, OpCode::Constant))
                .unwrap()
                .as_str(),
            "world"
        );
        tv.as_table_mut().remove(&Word::int(1, OpCode::Constant));
        assert_eq!(tv.as_table().len(), 1);
    }
}
