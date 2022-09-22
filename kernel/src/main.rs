#![no_std]
#![no_main]
#![feature(naked_functions, asm_sym, asm_const)]
#![feature(default_alloc_error_handler)]
#![deny(warnings)]

mod boot;
mod heap;
mod layout;
mod page;
mod space;

#[macro_use]
extern crate console;
extern crate alloc;

use boot::BootPageTable;
use core::{alloc::Layout, ptr::NonNull};
use layout::KernelLayout;
use page::GLOBAL;
use page_table::{MmuMeta, Pte, Sv39, VmFlags, PPN, VPN};
use riscv::register::satp;
use sbi_rt::*;
use space::{AddressSpace, PageManager};

static mut LAYOUT: KernelLayout = KernelLayout::INIT;

extern "C" fn rust_main(_hartid: usize, dtb_addr: usize) -> ! {
    // 收集内存信息
    unsafe { LAYOUT.locate() };
    // 上链接位置
    let _ = unsafe {
        BootPageTable(non_null(LAYOUT.v_to_p(LAYOUT.boot_pt_root()))).launch(&LAYOUT);
    };
    // FIXME 强行通过虚地址访问静态变量。不这么写编译器没法知道这个变量有两个地址。
    let info = unsafe { &mut *(LAYOUT.p_to_v((&LAYOUT) as *const _ as _) as *mut KernelLayout) };
    // 清零 .bss
    unsafe { info.zero_bss() };
    // 确认打印可用
    console::init_console(&Console);
    console::set_log_level(option_env!("LOG"));
    console::test_log();
    // 初始化页分配
    info.set_top(page::init_global(info, dtb_addr));
    // 初始化堆分配
    heap::init_heap(info.start());
    // 建立内核地址空间
    let mut kernel = AddressSpace::<Sv39, Global>::new(Global);
    kernel.kernel(VmFlags::build_from_str("DAG_XWRV"));
    unsafe { satp::set(satp::Mode::Sv39, 0, kernel.root_ppn().val()) };
    println!("{kernel:?}");
    // 回收启动页表
    unsafe { GLOBAL.deallocate(non_null::<u8>(info.boot_pt_root() as _), 4096) };
    unsafe { println!("{GLOBAL:?}") };
    system_reset(RESET_TYPE_SHUTDOWN, RESET_REASON_NO_REASON);
    unreachable!()
}

struct Global;

impl PageManager<Sv39> for Global {
    fn allocate(&mut self, flags: VmFlags<Sv39>, len: usize) -> Pte<Sv39> {
        let (ptr, _) = unsafe {
            GLOBAL.allocate_layout::<u8>(Layout::from_size_align_unchecked(
                len << Sv39::PAGE_BITS,
                1 << Sv39::PAGE_BITS,
            ))
        }
        .unwrap();
        flags.build_pte(self.v_to_p(ptr))
    }

    fn deallocate(&mut self, _pte: Pte<Sv39>, _len: usize) {
        todo!()
    }

    fn share(&mut self, _pte: Pte<Sv39>, _len: usize) -> (Pte<Sv39>, Pte<Sv39>) {
        todo!()
    }

    fn exclude(&mut self, _pte: Pte<Sv39>, _len: usize) -> Pte<Sv39> {
        todo!()
    }

    fn p_to_v<T>(&self, ppn: PPN<Sv39>) -> NonNull<T> {
        non_null(unsafe { LAYOUT.p_to_v(VPN::<Sv39>::new(ppn.val()).base().val()) } as _)
    }

    fn v_to_p<T>(&self, ptr: NonNull<T>) -> PPN<Sv39> {
        PPN::new((unsafe { LAYOUT.v_to_p(ptr.as_ptr() as _) }) >> Sv39::PAGE_BITS)
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

#[naked]
#[no_mangle]
#[link_section = ".text.entry"]
unsafe extern "C" fn _start() -> ! {
    core::arch::asm!(
        "la sp, _end + {size}",
        "mv tp, a0",
        "j  {main}",
        size = const KernelLayout::BOOT_STACK_SIZE,
        main =   sym rust_main,
        options(noreturn),
    )
}

#[inline]
fn non_null<T>(addr: usize) -> NonNull<T> {
    unsafe { NonNull::new_unchecked(addr as _) }
}
