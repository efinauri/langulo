use bitvec::vec::BitVec;
use crate::vm::tagged_value::TaggedValue;

pub struct GarbageCollector {
    tvs: Vec<TaggedValue>,
    marks: BitVec
}

impl GarbageCollector {
    pub fn new() -> Self {
        Self {
            tvs: Vec::new(),
            marks: BitVec::new(),
        }
    }

    pub fn trace_if_heap(&mut self, tv: TaggedValue) {
        if tv.in_heap() { self.trace(tv); }
    }

    pub fn trace(&mut self, tv: TaggedValue) {
        self.tvs.push(tv);
        self.marks.reserve(1);
    }

    // pub fn run(&mut self, roots: &[&[TaggedValue]]) {
    //     if self.tvs.is_empty() { return; } // nothing to clear
    //     self.marks.clear();
    //     for root in roots {
    //         for tv in root {
    //             self.mark(tv);
    //         }
    //     }
    //     self.sweep();
    // }

    // pub fn sweep(&mut self) {
    //
    // }
}

