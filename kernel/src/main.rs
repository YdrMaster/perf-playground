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
#![deny(warnings)]

mod boot;
mod heap;
mod page;

#[macro_use]
extern crate console;
extern crate alloc;

use boot::BootPageTable;
use linker::MemInfo;
use page_table::{Sv39, VmFlags};
// use console::log;
use sbi_rt::*;

static mut MEM_INFO: MemInfo = MemInfo::INIT;

extern "C" fn rust_main(_hartid: usize, dtb_addr: usize) -> ! {
    // 收集内存信息
    unsafe { MEM_INFO = MemInfo::locate() };
    let info = unsafe { MEM_INFO };
    // 上链接位置
    let _sstatus = unsafe {
        const ALIGN: usize = 4096 - 1;
        let addr = (info.end - info.offset + STACK_SIZE + ALIGN) & !ALIGN;
        BootPageTable::new(addr).launch(info.start - info.offset, info.offset)
    };
    // 清零 .bss
    unsafe { core::slice::from_raw_parts_mut(info.bss as _, info.end - info.bss).fill(0) };
    // 确认打印可用
    console::init_console(&Console);
    console::set_log_level(option_env!("LOG"));
    console::test_log();
    // 初始化页分配
    unsafe { MEM_INFO.top = page::init_global(info, dtb_addr) };
    // 初始化堆分配
    heap::init_heap(info);
    // x
    let mut manager = address_space::AddressSpace::<Sv39>::new();
    manager.kernel(VmFlags::build_from_str("DAG_XWRV"));
    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_NO_REASON);
    unreachable!()
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
    use crate::{page::GLOBAL, MEM_INFO};
    use core::{alloc::Layout, fmt, ptr::NonNull};
    use page_table::{PageTable, PageTableFormatter, Pte, VAddr, VmFlags, VmMeta, PPN, VPN};
    use rangemap::RangeSet;

    pub(crate) struct AddressSpace<Meta: VmMeta> {
        segments: RangeSet<VPN<Meta>>,
        root: NonNull<Pte<Meta>>,
    }

    impl<Meta: VmMeta> AddressSpace<Meta> {
        const PAGE_LAYOUT: Layout = unsafe {
            Layout::from_size_align_unchecked(1 << Meta::PAGE_BITS, 1 << Meta::PAGE_BITS)
        };

        pub fn new() -> Self {
            let (root, size) = unsafe { GLOBAL.allocate_layout(Self::PAGE_LAYOUT) }.unwrap();
            assert_eq!(size, 4096);
            Self {
                segments: RangeSet::new(),
                root,
            }
        }

        pub fn kernel(&mut self, flags: VmFlags<Meta>) {
            let info = unsafe { MEM_INFO };
            let top_entries = 1 << Meta::LEVEL_BITS.last().unwrap();
            let ppn_bits = Meta::pages_in_table(Meta::MAX_LEVEL - 1).trailing_zeros();
            // 内核线性段
            self.segments.insert(
                VAddr::<Meta>::new(info.offset).floor()..VAddr::<Meta>::new(info.top).ceil(),
            );
            // 页表
            unsafe { core::slice::from_raw_parts_mut(self.root.as_ptr(), top_entries) }
                .iter_mut()
                .skip(
                    VAddr::<Meta>::new(info.offset)
                        .floor()
                        .index_in(Meta::MAX_LEVEL),
                )
                .take(
                    VAddr::<Meta>::new(info.top - info.offset)
                        .ceil()
                        .ceil(Meta::MAX_LEVEL),
                )
                .enumerate()
                .for_each(|(i, pte)| *pte = flags.build_pte(PPN::new(i << ppn_bits)));

            println!("{self:?}")
        }
    }

    impl<Meta: VmMeta> fmt::Debug for AddressSpace<Meta> {
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
    }
}
