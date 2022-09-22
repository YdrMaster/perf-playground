use linker::MemInfo;
use page_table::{MmuMeta, Sv39};

/// 内核内存布局。
///
/// - 启动时：内核 | 启动栈 | 启动页表 | 动态区
/// - 启动后：内核 | 启动栈 | 动态区
pub struct KernelLayout {
    /// 链接时确定的符号。
    linked: MemInfo,
    /// 线性地址结束位置。
    top: usize,
}

impl KernelLayout {
    /// 启动栈容量。
    pub const BOOT_STACK_SIZE: usize = 4096 * 4;

    pub const INIT: Self = Self {
        linked: MemInfo::INIT,
        top: usize::MAX,
    };

    /// 物理地址动态定位。
    pub unsafe fn locate(&mut self) {
        self.linked = MemInfo::locate();
    }

    /// 设置线性地址结束位置。
    pub fn set_top(&mut self, top: usize) {
        self.top = top;
    }

    /// .bss 段清零。
    ///
    /// # Safety
    ///
    /// 调用时需要保证没有在 .bss 上写入值。这基本意味着应在到达链接地址空间后立即执行。
    pub unsafe fn zero_bss(&self) {
        let bss = self.linked.bss;
        let end = self.linked.end;
        core::slice::from_raw_parts_mut(bss as _, end - bss).fill(0u8);
    }

    /// 启动页表根节点：启动栈之后的第一个页
    pub fn boot_pt_root(&self) -> usize {
        const ALIGN: usize = (1 << Sv39::PAGE_BITS) - 1;
        (self.linked.end + Self::BOOT_STACK_SIZE + ALIGN) & !ALIGN
    }

    /// 线性区虚地址相对物理地址的偏移。
    pub const fn offset(&self) -> usize {
        self.linked.offset
    }

    /// 物理地址转换为线性区虚地址。
    pub const fn p_to_v(&self, paddr: usize) -> usize {
        paddr + self.linked.offset
    }

    /// 线性区虚地址转换为物理地址。
    pub const fn v_to_p(&self, vaddr: usize) -> usize {
        vaddr - self.linked.offset
    }

    /// 内核起始地址。
    pub const fn start(&self) -> usize {
        self.linked.start
    }

    /// 线性区结束位置。
    pub const fn top(&self) -> usize {
        self.top
    }
}
