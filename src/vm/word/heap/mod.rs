use crate::vm::word::word_shape::OpCode;
use crate::vm::word::word_shape::ValueTag;
use crate::vm::word::Word;
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
    ($input:expr, $tag:expr, $opcode:expr) => {
        unsafe {
            let alloc = allocate(Layout::new::<Self>());
            let w = Word::new(
                alloc,
                false,
                false,
                $opcode,
                $tag,
            );

            debug_assert_eq!(alloc as u32, w.ptr() as u32);

            let container = unsafe { w.get_mut::<Self>() };
            init!(container.0 => $input);
            w
        }
    }
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

    fn write(value: f64, opcode: OpCode) -> Word {
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

    fn write(value: String, opcode: OpCode) -> Word {
        heap_write!(value, ValueTag::StrPtr, opcode)
    }
    fn destroy(w: Word) {
        heap_destroy!(w);
    }
}

pub struct HeapTable(pub BTreeMap<Word, Word>);
impl HeapValue for HeapTable {
    type Inner = BTreeMap<Word, Word>;

    fn read(w: &Word) -> &Self::Inner {
        heap_read!(w)
    }

    fn get_inner_mut(w: &mut Word) -> &mut Self::Inner {
        heap_get_inner_mut!(w)
    }

    fn write(value: BTreeMap<Word, Word>, opcode: OpCode) -> Word {
        heap_write!(value, ValueTag::TablePtr, opcode)
    }
    fn destroy(w: Word) {
        heap_destroy!(w);
    }
}
