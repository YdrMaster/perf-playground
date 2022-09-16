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
    let mut manager = address_space::Manager::new();
    manager.kernel();
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
    use core::{alloc::Layout, ptr::NonNull};
    use page_table::{PageTable, PageTableFormatter, Pte, Sv39, VAddr, VmFlags, VmMeta, PPN, VPN};
    use rangemap::RangeMap;

    pub(crate) struct Manager {
        segments: RangeMap<VPN<Sv39>, ()>,
        pages: NonNull<Pte<Sv39>>,
    }

    impl Manager {
        pub fn new() -> Self {
            let (root, size) = unsafe { GLOBAL.allocate_layout(PAGE_LAYOUT) }.unwrap();
            assert_eq!(size, 4096);
            Self {
                segments: RangeMap::new(),
                pages: root,
            }
        }

        pub fn kernel(&mut self) {
            const FLAGS: VmFlags<Sv39> = VmFlags::build_from_str("DAG_XWRV");

            let info = unsafe { MEM_INFO };
            let ptop = info.top - info.offset;
            self.segments
                .insert(VPN::new(0)..VPN::new(ptop >> 30 << 18), ());
            let table = unsafe { core::slice::from_raw_parts_mut(self.pages.as_ptr(), 512) };
            let base = VAddr::<Sv39>::new(info.offset)
                .floor()
                .index_in(Sv39::MAX_LEVEL);
            table[base..]
                .iter_mut()
                .take(ptop >> 30)
                .enumerate()
                .for_each(|(i, pte)| *pte = FLAGS.build_pte(PPN::new(i << 18)));
            println!("{:?}", self.segments);
            println!(
                "{:?}",
                PageTableFormatter {
                    pt: unsafe { PageTable::from_root(self.pages) },
                    f: |ppn| unsafe {
                        NonNull::new_unchecked(VPN::<Sv39>::new(ppn.val()).base().val() as _)
                    }
                }
            )
        }
    }

    const PAGE_LAYOUT: Layout = unsafe { Layout::from_size_align_unchecked(4096, 4096) };
}
