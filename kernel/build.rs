fn main() {
    use std::{env, fs, path::PathBuf};
    const START: usize = 0xffff_ffc0_8020_0000;

    let ld = &PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("linker.ld");
    fs::write(ld, format!("START = {START:#x};{LINKER}")).unwrap();

    println!("cargo:rustc-env=ENTRY_VADDR={START:#x}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LOG");
    println!("cargo:rustc-link-arg=-T{}", ld.display());
}

const LINKER: &str = "
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
    .bss : {
        *(.bss.uninit)
        sbss = ALIGN(8);
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        ebss = ALIGN(8);
    }
}";
