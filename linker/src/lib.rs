//! 在 kernel 的 build.rs 和 src 之间共享常量。

#![no_std]
#![deny(warnings, missing_docs)]

/// 内核链接位置。
pub const START: usize = 0xffff_ffc0_8020_0000;

/// 链接脚本内容。
pub const BODY: &str = "
OUTPUT_ARCH(riscv)
ENTRY(_start)
SECTIONS {
    . = START;
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss : ALIGN(8) {
        _bss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
    _end = ALIGN(4K);
}";

// 链接脚本里定义的符号。
extern "C" {
    /// 内核起始位置。
    pub static _start: u64;

    /// .bss 段起始位置。
    pub static mut _bss: u64;

    /// 内核结束位置。
    pub static mut _end: u64;
}

/// 内核地址信息。
#[derive(Clone, Copy, Debug)]
pub struct MemInfo {
    /// 物理地址。
    pub base: usize,
    /// 虚地址。
    pub offset: usize,
}

impl MemInfo {
    /// 定位内核内核内存信息。
    ///
    /// # Safety
    ///
    /// 在物理地址空间中调用，用于自动定位内核物理地址。
    #[inline]
    pub unsafe fn locate() -> Self {
        let base = unsafe { &_start as *const _ as usize };
        Self {
            base,
            offset: START - base,
        }
    }
}
