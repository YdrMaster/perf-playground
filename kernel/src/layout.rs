use core::ptr::NonNull;

use crate::STACK_SIZE;
use linker::MemInfo;
use page_table::{MmuMeta, Pte, Sv39};

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

    pub fn p_boot_pt_root(&self) -> NonNull<Pte<Sv39>> {
        unsafe { NonNull::new_unchecked((self.boot_pt_root() - self.linked.offset) as _) }
    }

    /// 启动页表根节点：启动栈之后的第一个页
    pub fn boot_pt_root(&self) -> usize {
        const ALIGN: usize = (1 << Sv39::PAGE_BITS) - 1;
        (self.linked.end + STACK_SIZE + ALIGN) & !ALIGN
    }

    pub const fn offset(&self) -> usize {
        self.linked.offset
    }

    pub const fn p_to_v(&self, paddr: usize) -> usize {
        paddr + self.linked.offset
    }

    pub fn v_to_p<T>(&self, ptr: *const T) -> usize {
        ptr as usize - self.linked.offset
    }

    pub const fn start(&self) -> usize {
        self.linked.start
    }

    pub const fn top(&self) -> usize {
        self.top
    }

    pub const fn p_start(&self) -> usize {
        self.linked.start - self.linked.offset
    }

    pub const fn p_top(&self) -> usize {
        self.top - self.linked.offset
    }
}
