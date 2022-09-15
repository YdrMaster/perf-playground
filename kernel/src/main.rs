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
#![deny(warnings)]

mod boot;
mod page;

use boot::BootPageTable;
use console::*;
use sbi_rt::*;
use spin::Once;

static MEM_INFO: Once<linker::MemInfo> = Once::new();

extern "C" fn rust_main(_hartid: usize, dtb_addr: usize) -> ! {
    // 收集内存信息
    let info = *MEM_INFO.call_once(|| unsafe { linker::MemInfo::locate() });
    // 上链接位置
    let _sstatus = unsafe { boot_page_table().launch(info.base, info.offset) };
    // 清零 .bss
    unsafe { r0::zero_bss(&mut linker::_bss, &mut linker::_end) };
    // 确认打印可用
    init_console(&Console);
    console::set_log_level(option_env!("LOG"));
    console::test_log();
    // 初始化页分配
    page::init_global(info, dtb_addr);
    unsafe { log::info!("{:?}", page::GLOBAL) };
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
        "la  sp, {_end} + {size}",
        "j   {main}",
        size = const STACK_SIZE,
        _end =   sym linker::_end,
        main =   sym rust_main,
        options(noreturn),
    )
}

/// 定位启动栈。
#[inline]
fn boot_stack() -> &'static mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(&linker::_end as *const _ as _, STACK_SIZE) }
}

/// 定位启动页表。
///
/// 启动页表直接放在启动栈栈底，用完可以丢掉。
#[inline]
fn boot_page_table() -> BootPageTable {
    unsafe { BootPageTable::new(&linker::_end as *const _ as _) }
}
