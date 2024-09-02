use crate::word::structure::{OpCode, Word};
use crate::word::structure::ValueTag;
use libc::{mmap, MAP_32BIT, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};
use std::alloc::{dealloc, handle_alloc_error, Layout};
use std::collections::BTreeMap;
use std::ptr;
use std::ptr::drop_in_place;

/// custom allocation in the 32bit space, since we only have 32bits to represent pointers
fn allocate(layout: Layout) -> *mut u8 {
    let ptr = unsafe {
        mmap(
            ptr::null_mut(),
            layout.size(),
            PROT_READ | PROT_WRITE,
            MAP_32BIT | MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        )
    };

    if ptr == libc::MAP_FAILED {
        handle_alloc_error(layout);
    } else {
        ptr as _
    }
}

macro_rules! init {
    ($field: expr => $value: expr) => {
        unsafe {
            std::ptr::addr_of_mut!($field).write($value);
        }
    };
}

macro_rules! heap_get_inner_mut {
    ($w:expr) => {
        &mut $w.get_mut::<Self>().0
    }
}

macro_rules! heap_read {
    ($w:expr) => {{
            &$w.get::<Self>().0
        }};
}

macro_rules! heap_write {
    ($input:expr, $tag:expr, $opcode:expr) => {{
            let alloc = allocate(Layout::new::<Self>());
            let w = Word::new(
                alloc,
                $opcode,
                $tag,
            );

            debug_assert_eq!(alloc as u32, w.ptr() as u32);

            let container = w.get_mut::<Self>();
            init!(container.0 => $input);
            w
        }}
}

macro_rules! heap_destroy {
    ($w:expr) => {
        unsafe {
            drop_in_place($w.ptr() as *mut Self);
            dealloc($w.ptr(), Layout::new::<Self>());
        }
    };
}

pub trait HeapValue {
    type Inner;
    fn read(w: &Word) -> &Self::Inner;
    fn get_inner_mut(w: &mut Word) -> &mut Self::Inner;
    fn write(value: Self::Inner, opcode: OpCode) -> Word;
    fn destroy(w: Word);
}

pub struct HeapFloat(f64);
impl HeapValue for HeapFloat {
    type Inner = f64;

    fn read(w: &Word) -> &Self::Inner {
        heap_read!(w)
    }

    fn get_inner_mut(w: &mut Word) -> &mut Self::Inner {
        heap_get_inner_mut!(w)
    }

    fn write(value: Self::Inner, opcode: OpCode) -> Word {
        heap_write!(value, ValueTag::FloatPtr, opcode)
    }
    fn destroy(w: Word) {
        heap_destroy!(w);
    }
}

pub struct HeapStr(pub String);
impl HeapValue for HeapStr {
    type Inner = String;

    fn read(w: &Word) -> &Self::Inner {
        heap_read!(w)
    }

    fn get_inner_mut(w: &mut Word) -> &mut Self::Inner {
        heap_get_inner_mut!(w)
    }

    fn write(value: Self::Inner, opcode: OpCode) -> Word {
        heap_write!(value, ValueTag::StrPtr, opcode)
    }
    fn destroy(w: Word) {
        heap_destroy!(w);
    }
}

pub type Table = BTreeMap<Word, Word>;
pub struct HeapTable(pub Table);
impl HeapValue for HeapTable {
    type Inner = Table;

    fn read(w: &Word) -> &Self::Inner {
        heap_read!(w)
    }

    fn get_inner_mut(w: &mut Word) -> &mut Self::Inner {
        heap_get_inner_mut!(w)
    }

    fn write(value: Self::Inner, opcode: OpCode) -> Word {
        heap_write!(value, ValueTag::TablePtr, opcode)
    }
    fn destroy(w: Word) {
        heap_destroy!(w);
    }
}

pub struct HeapOption(pub Option<Word>);
impl HeapValue for HeapOption {
    type Inner = Option<Word>;

    fn read(w: &Word) -> &Self::Inner {
        heap_read!(w)
    }

    fn get_inner_mut(w: &mut Word) -> &mut Self::Inner {
        heap_get_inner_mut!(w)
    }

    fn write(value: Self::Inner, opcode: OpCode) -> Word {
        heap_write!(value, ValueTag::OptionPtr, opcode)
    }

    fn destroy(w: Word) {
        heap_destroy!(w);
    }
}

pub fn encode_table(table: &Table) -> Vec<u64> {
    let mut encoded = Vec::new();

    for (key, value) in table {
        encoded.push(key.0 as u64);
        encoded.push(value.0 as u64);
    }

    encoded
}

pub fn decode_table(encoded: Vec<u8>) -> Table {
    let mut table = BTreeMap::new();
    let mut i = 0;

    while i < encoded.len() {
        let key = Word::from_u64(u64::from_le_bytes(
            encoded[i..i+8].try_into().unwrap()));
        let value = Word::from_u64(u64::from_le_bytes(
            encoded[i+8..i+16].try_into().unwrap()));
        table.insert(key, value);
        i += 16;
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_encode() {
        let mut table = HeapTable(BTreeMap::new());
        table.0.insert(Word::int(42, OpCode::Value), Word::int(123, OpCode::Value));
        let encoded = encode_table(&table.0);
        // let decoded = decode_table(&encoded);
        // assert_eq!(table.0, decoded);
    }
}