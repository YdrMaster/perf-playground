#[macro_use]
extern crate clap;

use clap::Parser;
use command_ext::{BinUtil, Cargo, CommandExt, Qemu};
use once_cell::sync::Lazy;
use std::{
    fs,
    path::{Path, PathBuf},
};

const TARGET_ARCH: &str = "riscv64gc-unknown-none-elf";

static PROJECT: Lazy<&'static Path> =
    Lazy::new(|| Path::new(std::env!("CARGO_MANIFEST_DIR")).parent().unwrap());

static TARGET: Lazy<PathBuf> = Lazy::new(|| PROJECT.join("target").join(TARGET_ARCH));

#[derive(Parser)]
#[clap(name = "perf-playground")]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Make(BuildArgs),
    Asm(BuildArgs),
    Qemu(BuildArgs),
}

fn main() {
    use Commands::*;
    match Cli::parse().command {
        Make(args) => args.make(),
        Asm(args) => args.asm(),
        Qemu(args) => args.qemu(),
    }
}

#[derive(Args, Default)]
struct BuildArgs {
    /// log level
    #[clap(long)]
    log: Option<String>,
}

impl BuildArgs {
    fn make(&self) {
        Cargo::build()
            .package("kernel")
            .optional(&self.log, |cargo, level| {
                cargo.env("LOG", level);
            })
            .release()
            .target(TARGET_ARCH)
            .invoke();
    }

    fn asm(&self) {
        self.make();
        let elf = TARGET.join("release").join("kernel");
        let out = PROJECT.join("kernel.asm");
        fs::write(out, BinUtil::objdump().arg(elf).arg("-d").output().stdout).unwrap();
    }

    fn qemu(&self) {
        self.make();
        let elf = TARGET.join("release").join("kernel");
        Qemu::system("riscv64")
            .args(["-machine", "virt"])
            .arg("-bios")
            .arg(PROJECT.join("rustsbi-qemu.bin"))
            .arg("-kernel")
            .arg(objcopy(elf, true))
            .args(["-smp", "1"])
            .args(["-serial", "mon:stdio"])
            .args(["-m", "2G"])
            .arg("-nographic")
            .invoke();
    }
}

fn objcopy(elf: impl AsRef<Path>, binary: bool) -> PathBuf {
    let elf = elf.as_ref();
    let bin = elf.with_extension("bin");
    BinUtil::objcopy()
        .arg(elf)
        .arg("--strip-all")
        .conditional(binary, |binutil| {
            binutil.args(["-O", "binary"]);
        })
        .arg(&bin)
        .invoke();
    bin
}
