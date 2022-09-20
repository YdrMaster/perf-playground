use linker::MemInfo;

pub struct MemLayout {
    linked: MemInfo,
    top: usize,
}

impl MemLayout {
    pub const INIT: Self = Self {
        linked: MemInfo::INIT,
        top: usize::MAX,
    };

    pub unsafe fn locate(&mut self) {
        self.linked = MemInfo::locate();
    }

    pub fn set_top(&mut self, top: usize) {
        self.top = top;
    }

    pub fn zero_bss(&self) {
        unsafe {
            core::slice::from_raw_parts_mut(self.linked.bss as _, self.linked.end - self.linked.bss)
                .fill(0u8)
        };
    }

    pub const fn offset(&self) -> usize {
        self.linked.offset
    }

    pub const fn start(&self) -> usize {
        self.linked.start
    }

    pub const fn end(&self) -> usize {
        self.linked.end
    }

    pub const fn top(&self) -> usize {
        self.top
    }

    pub const fn p_start(&self) -> usize {
        self.linked.start - self.linked.offset
    }

    pub const fn p_end(&self) -> usize {
        self.linked.end - self.linked.offset
    }

    pub const fn p_top(&self) -> usize {
        self.top - self.linked.offset
    }
}
