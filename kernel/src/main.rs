//! ## Stage 0
//!
//!    1. 上链接位置
//!    2. 清零 .bss
//!    3. 确认打印可用
//!    4. 建立页分配器（第一次解析设备树得到内存信息）
//!    5. 建立内核地址空间
//!
//! ## Stage 1
//!
//!    1. 上内核地址空间

#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
// #![deny(warnings)]

mod boot;
mod heap;
mod layout;
mod page;

#[macro_use]
extern crate console;
extern crate alloc;

use address_space::{AddressSpace, PageManager};
use boot::BootPageTable;
use core::{alloc::Layout, ptr::NonNull};
use layout::MemLayout;
use page::GLOBAL;
use page_table::{MmuMeta, Pte, Sv39, VmFlags, PPN, VPN};
use riscv::register::satp;
use sbi_rt::*;

static mut MEM_INFO: MemLayout = MemLayout::INIT;

extern "C" fn rust_main(_hartid: usize, dtb_addr: usize) -> ! {
    // 收集内存信息
    unsafe { MEM_INFO.locate() };
    // 上链接位置
    let _ = unsafe {
        BootPageTable(MEM_INFO.p_boot_pt_root()).launch(MEM_INFO.p_start(), MEM_INFO.offset());
    };
    let info =
        unsafe { &mut *(MEM_INFO.p_to_v((&MEM_INFO) as *const _ as usize) as *mut MemLayout) };
    // 清零 .bss
    info.zero_bss();
    // 确认打印可用
    console::init_console(&Console);
    console::set_log_level(option_env!("LOG"));
    console::test_log();
    // 初始化页分配
    unsafe { info.set_top(page::init_global(info, dtb_addr)) };
    // 初始化堆分配
    heap::init_heap(info.start());
    // 建立内核地址空间
    let mut kernel = AddressSpace::<Sv39, Global>::new(Global);
    kernel.kernel(VmFlags::build_from_str("DAG_XWRV"));
    unsafe { satp::set(satp::Mode::Sv39, 0, kernel.root_ppn().val()) };
    println!("{kernel:?}");
    // 回收启动页表
    unsafe { GLOBAL.deallocate(NonNull::<u8>::new_unchecked(info.boot_pt_root() as _), 4096) };
    unsafe { println!("{GLOBAL:?}") };
    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_NO_REASON);
    unreachable!()
}

struct Global;

impl PageManager<Sv39> for Global {
    fn allocate(&mut self, flags: VmFlags<Sv39>, len: usize) -> Pte<Sv39> {
        const PAGE_SIZE: usize = 1 << Sv39::PAGE_BITS;
        let (ptr, _) = unsafe {
            GLOBAL.allocate_layout::<u8>(Layout::from_size_align_unchecked(PAGE_SIZE, PAGE_SIZE))
        }
        .unwrap();
        flags.build_pte(self.v_to_p(ptr))
    }

    fn deallocate(&mut self, pte: Pte<Sv39>, len: usize) {
        todo!()
    }

    fn share(&mut self, pte: Pte<Sv39>, len: usize) -> (Pte<Sv39>, Pte<Sv39>) {
        todo!()
    }

    fn exclude(&mut self, pte: Pte<Sv39>, len: usize) -> Pte<Sv39> {
        todo!()
    }

    fn p_to_v<T>(&self, ppn: PPN<Sv39>) -> NonNull<T> {
        unsafe {
            NonNull::new_unchecked((MEM_INFO.p_to_v(VPN::<Sv39>::new(ppn.val()).base().val())) as _)
        }
    }

    fn v_to_p<T>(&self, ptr: NonNull<T>) -> PPN<Sv39> {
        PPN::new((unsafe { MEM_INFO.v_to_p(ptr.as_ptr()) }) >> Sv39::PAGE_BITS)
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_SYSTEM_FAILURE);
    unreachable!()
}

struct Console;

impl console::Console for Console {
    fn put_char(&self, c: u8) {
        #[allow(deprecated)]
        legacy::console_putchar(c as _);
    }
}

/// 检测支持的 ASID 位数。
#[allow(unused)]
fn asid_detect() -> usize {
    use core::arch::asm;
    unsafe {
        const MASK: usize = (1 << 16) - 1;
        let mut mask = MASK << 44;
        asm!(
            "csrrs zero, satp, {0}",
            "csrr   {0}, satp",
            inlateout(reg) mask
        );
        ((mask >> 44) & MASK).trailing_ones() as _
    }
}

/// 启动栈容量。
const STACK_SIZE: usize = 4096 * 4;

#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    core::arch::asm!(
        "la sp, _end + {size}",
        "mv tp, a0",
        "j  {main}",
        size = const STACK_SIZE,
        main =   sym rust_main,
        options(noreturn),
    )
}

mod address_space {
    use crate::MEM_INFO;
    use core::{alloc::Layout, fmt, ptr::NonNull};
    use page_table::{PageTable, PageTableFormatter, Pte, VAddr, VmFlags, VmMeta, PPN, VPN};
    use rangemap::RangeSet;

    pub(crate) struct AddressSpace<Meta: VmMeta, M: PageManager<Meta>> {
        segments: RangeSet<VPN<Meta>>,
        root: NonNull<Pte<Meta>>,
        manager: M,
    }

    impl<Meta: VmMeta, M: PageManager<Meta>> AddressSpace<Meta, M> {
        const PAGE_LAYOUT: Layout = unsafe {
            Layout::from_size_align_unchecked(1 << Meta::PAGE_BITS, 1 << Meta::PAGE_BITS)
        };

        pub fn new(mut manager: M) -> Self {
            let ppn = manager.allocate(VmFlags::VALID, 1).ppn();
            Self {
                segments: RangeSet::new(),
                root: manager.p_to_v(ppn),
                manager,
            }
        }

        pub fn root_ppn(&self) -> PPN<Meta> {
            self.manager.v_to_p(self.root)
        }

        pub fn kernel(&mut self, flags: VmFlags<Meta>) {
            let info = unsafe { &MEM_INFO };
            let top_entries = 1 << Meta::LEVEL_BITS.last().unwrap();
            let ppn_bits = Meta::pages_in_table(Meta::MAX_LEVEL - 1).trailing_zeros();
            // 内核线性段
            self.segments.insert(
                VAddr::<Meta>::new(info.offset()).floor()..VAddr::<Meta>::new(info.top()).ceil(),
            );
            // 页表
            unsafe { core::slice::from_raw_parts_mut(self.root.as_ptr(), top_entries) }
                .iter_mut()
                .skip(
                    VAddr::<Meta>::new(info.offset())
                        .floor()
                        .index_in(Meta::MAX_LEVEL),
                )
                .take(
                    VAddr::<Meta>::new(info.p_top())
                        .ceil()
                        .ceil(Meta::MAX_LEVEL),
                )
                .enumerate()
                .for_each(|(i, pte)| *pte = flags.build_pte(PPN::new(i << ppn_bits)));
        }
    }

    impl<Meta: VmMeta, M: PageManager<Meta>> fmt::Debug for AddressSpace<Meta, M> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for seg in self.segments.iter() {
                writeln!(
                    f,
                    "{:#x}..{:#x}",
                    seg.start.base().val(),
                    seg.end.base().val()
                )?;
            }
            writeln!(
                f,
                "{:?}",
                PageTableFormatter {
                    pt: unsafe { PageTable::from_root(self.root) },
                    f: |ppn| unsafe {
                        NonNull::new_unchecked(VPN::<Meta>::new(ppn.val()).base().val() as _)
                    }
                }
            )
        }
    }

    pub trait PageManager<Meta: VmMeta> {
        fn allocate(&mut self, flags: VmFlags<Meta>, len: usize) -> Pte<Meta>;
        fn deallocate(&mut self, pte: Pte<Meta>, len: usize);
        fn share(&mut self, pte: Pte<Meta>, len: usize) -> (Pte<Meta>, Pte<Meta>);
        fn exclude(&mut self, pte: Pte<Meta>, len: usize) -> Pte<Meta>;
        fn p_to_v<T>(&self, ppn: PPN<Meta>) -> NonNull<T>;
        fn v_to_p<T>(&self, ptr: NonNull<T>) -> PPN<Meta>;
    }
}
