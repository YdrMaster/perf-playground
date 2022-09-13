#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![deny(warnings)]

mod boot;

use boot::BootPageTable;
use console::*;
use sbi_rt::*;

#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    const STACK_SIZE: usize = 4096;

    #[link_section = ".bss.uninit"]
    static mut STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];

    core::arch::asm!(
        "la sp, {stack} + {stack_size}",
        "j  {main}",
        stack_size = const STACK_SIZE,
        stack      =   sym STACK,
        main       =   sym rust_main,
        options(noreturn),
    )
}

const ENTRY_VADDR: usize = 0xffff_ffc0_8020_0000;

static mut BOOT_PAGE_TABLE: BootPageTable = BootPageTable::ZERO;

extern "C" fn rust_main(hartid: usize, dtb_addr: usize) -> ! {
    // 清零 bss 段
    extern "C" {
        static mut sbss: u64;
        static mut ebss: u64;
    }
    unsafe { r0::zero_bss(&mut sbss, &mut ebss) };
    // 使能启动页表
    let _sstatus = unsafe {
        BOOT_PAGE_TABLE.init();
        BOOT_PAGE_TABLE.launch()
    };
    init_console(&Console);
    console::set_log_level(option_env!("LOG"));
    console::test_log();
    println!("hartid = {hartid}, dtb = {dtb_addr:#x}");
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
