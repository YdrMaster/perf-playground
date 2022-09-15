use core::{arch::asm, ptr::NonNull};
use page_table::{MmuMeta, Pte, Sv39, VAddr, VmFlags, VmMeta, PPN};

/// 启动页表。
pub(crate) struct BootPageTable(NonNull<Pte<Sv39>>);

impl BootPageTable {
    #[inline]
    pub fn new(addr: usize) -> Self {
        assert!(addr.trailing_zeros() as usize >= Sv39::PAGE_BITS);
        Self(NonNull::new(addr as _).unwrap())
    }

    /// 根据内核实际位置初始化启动页表，然后启动地址转换跃迁到高地址，并设置内核对用户页的访问权限。
    ///
    /// # Safety
    ///
    /// 调用前后位于不同的地址空间，必须内联。
    #[inline(always)]
    pub unsafe fn launch(&self, pbase: usize, offset: usize) -> usize {
        use riscv::register::satp;
        const FLAGS: VmFlags<Sv39> = VmFlags::build_from_str("DAG_XWRV");

        // 确保虚实地址在 1 GiB 内对齐
        assert!(offset.trailing_zeros() >= 30);
        let table = unsafe { core::slice::from_raw_parts_mut(self.0.as_ptr(), 512) };
        // 映射跳板页
        let base = VAddr::<Sv39>::new(pbase).floor().index_in(Sv39::MAX_LEVEL);
        table[base] = FLAGS.build_pte(PPN::new(base << 18));
        // 映射物理地址空间的前 128 GiB
        let base = VAddr::<Sv39>::new(offset).floor().index_in(Sv39::MAX_LEVEL);
        table[base..]
            .iter_mut()
            .take(128)
            .enumerate()
            .for_each(|(i, pte)| *pte = FLAGS.build_pte(PPN::new(i << 18)));
        // 启动地址转换
        satp::set(
            satp::Mode::Sv39,
            0,
            self.0.as_ptr() as usize >> Sv39::PAGE_BITS,
        );
        // 此时原本的地址空间还在，所以不用刷快表
        // riscv::asm::sfence_vma_all();
        // 跳到高页面对应位置
        Self::jump_higher(offset);
        // 设置内核可访问用户页
        let mut sstatus = 1usize << 18;
        asm!("csrrs {0}, sstatus, {0}", inlateout(reg) sstatus);
        sstatus | (1usize << 18)
    }

    /// 向上跳到距离为 `offset` 的新地址然后继续执行。
    ///
    /// # Safety
    ///
    /// 裸函数。
    ///
    /// 导致栈重定位，栈上的指针将失效！
    #[naked]
    unsafe extern "C" fn jump_higher(offset: usize) {
        asm!("add sp, sp, a0", "add ra, ra, a0", "ret", options(noreturn))
    }
}
