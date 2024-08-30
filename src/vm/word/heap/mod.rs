use crate::vm::word::word_shape::OpCode;
use crate::vm::word::word_shape::ValueTag;
use crate::vm::word::Word;
use std::alloc::{dealloc, handle_alloc_error, Layout};
use std::collections::BTreeMap;
use std::ptr;
use std::ptr::drop_in_place;
use libc::{mmap, MAP_32BIT, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE};

/// custom allocation in the 32bit space, since we only have 32bits to represent pointers
fn allocate(layout: Layout) -> *mut u8 {
    let ptr = unsafe {
        mmap(
            ptr::null_mut(),
            layout.size(),
            PROT_READ | PROT_WRITE,
            MAP_32BIT | MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0
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

macro_rules! heap_read {
    ($w:expr) => {
        &$w.get::<Self>().0
    };
}

macro_rules! heap_write {
    ($input:expr, $tag:expr) => {{
        let alloc = allocate(Layout::new::<Self>());
        let w = Word::new(
            alloc,
            false,
            false,
            OpCode::Value,
            $tag,
        );

        debug_assert_eq!(alloc as u32, w.ptr() as u32);

        let container = unsafe { w.get_mut::<Self>() };
        init!(container.0 => $input);
        w
    }}
}

macro_rules! heap_destroy {
    ($w:expr) => {
        drop_in_place($w.ptr() as *mut Self);
        dealloc($w.ptr(), Layout::new::<Self>());
    };
}

pub trait HeapValue<T> {
    unsafe fn read(w: &Word) -> &T;
    unsafe fn write(value: T) -> Word;
    unsafe fn destroy(w: Word);
}

pub struct HeapFloat(f64);
impl HeapValue<f64> for HeapFloat {
    unsafe fn read(w: &Word) -> &f64 {
        heap_read!(w)
    }
    unsafe fn write(value: f64) -> Word {
        heap_write!(value, ValueTag::FloatPtr)
    }
    unsafe fn destroy(w: Word) {
        heap_destroy!(w);
    }
}

pub struct HeapStr(pub String);
impl HeapValue<String> for HeapStr {
    unsafe fn read(w: &Word) -> &String {
        heap_read!(w)
    }
    unsafe fn write(value: String) -> Word {
        heap_write!(value, ValueTag::StrPtr)
    }
    unsafe fn destroy(w: Word) {
        heap_destroy!(w);
    }
}

pub struct HeapTable(pub BTreeMap<Word, Word>);
impl HeapValue<BTreeMap<Word, Word>> for HeapTable {
    unsafe fn read(w: &Word) -> &BTreeMap<Word, Word> {
        heap_read!(w)
    }
    unsafe fn write(value: BTreeMap<Word, Word>) -> Word {
        heap_write!(value, ValueTag::TablePtr)
    }
    unsafe fn destroy(w: Word) {
        heap_destroy!(w);
    }
}
