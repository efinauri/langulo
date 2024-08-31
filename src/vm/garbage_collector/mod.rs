use crate::word::structure::Word;
use bitvec::vec::BitVec;

pub struct GarbageCollector {
    tvs: Vec<Word>,
    marks: BitVec,
}

impl GarbageCollector {
    pub fn new() -> Self {
        Self {
            tvs: Vec::new(),
            marks: BitVec::new(),
        }
    }

    // #[cfg(feature = "debug")]
    pub fn all_marked(&self) -> bool {
        self.marks.all()
    }

    pub fn trace_if_heap(&mut self, tv: Word) {
        if tv.in_heap() {
            self.trace(tv);
        }
    }

    pub fn trace(&mut self, tv: Word) {
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
