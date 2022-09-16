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
    _end = ALIGN(8);
}";

/// 内核地址信息。
#[derive(Clone, Copy, Debug)]
pub struct MemInfo {
    /// 线性区偏移。
    ///
    /// 即物理地址 0 映射的虚地址。
    pub offset: usize,
    /// 物理地址结束位置映射的虚地址。
    pub top: usize,

    /// 内核虚地址。
    pub start: usize,
    /// .bss 虚地址。
    pub bss: usize,
    /// 内核结束位置虚地址。
    pub end: usize,
}

impl MemInfo {
    /// 非零初始化，避免 bss。
    pub const INIT: Self = Self {
        offset: usize::MAX,
        top: usize::MAX,
        start: usize::MAX,
        bss: usize::MAX,
        end: usize::MAX,
    };

    /// 定位内核内核内存信息。
    ///
    /// # Safety
    ///
    /// 在物理地址空间中调用，用于自动定位内核物理地址。
    #[inline]
    pub unsafe fn locate() -> Self {
        extern "C" {
            fn _start();
            fn _bss();
            fn _end();
        }

        let offset = START - _start as usize;
        Self {
            offset,
            top: 0,
            start: _start as usize + offset,
            bss: _bss as usize + offset,
            end: _end as usize + offset,
        }
    }
}
