#[macro_use]
extern crate clap;

use clap::Parser;
use os_xtask_utils::{BinUtil, Cargo, CommandExt, Qemu};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process,
    sync::OnceLock,
};

fn project() -> &'static Path {
    static PROJECT: OnceLock<&'static Path> = OnceLock::new();
    PROJECT.get_or_init(|| Path::new(std::env!("CARGO_MANIFEST_DIR")).parent().unwrap())
}

#[derive(Parser)]
#[clap(name = "RustSBI-Qemu")]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Make this project
    Make(BuildArgs),
    /// Dump assembly code of RustSBI-QEMU
    Asm(AsmArgs),
    /// Run RustSBI-QEMU and test-kernel in QEMU
    Qemu(QemuArgs),
}

fn main() {
    use Commands::*;
    match Cli::parse().command {
        Make(args) => {
            args.make(package(args.kernel.as_ref()), true);
        }
        Asm(args) => args.dump(),
        Qemu(args) => args.run(),
    }
}

#[derive(Args, Default)]
struct BuildArgs {
    /// With supervisor.
    #[clap(short, long)]
    kernel: Option<String>,
    /// Log level.
    #[clap(long)]
    log: Option<String>,
    /// Build in debug mode.
    #[clap(long)]
    debug: bool,
}

impl BuildArgs {
    fn make(&self, package: &str, binary: bool) -> PathBuf {
        let target = "riscv64imac-unknown-none-elf";
        Cargo::build()
            .package(package)
            .optional(&self.log, |cargo, log| {
                cargo.env("LOG", log);
            })
            .conditional(!self.debug, |cargo| {
                cargo.release();
            })
            .target(target)
            .invoke();
        let elf = project()
            .join("target")
            .join(target)
            .join(if self.debug { "debug" } else { "release" })
            .join(package);
        if binary {
            let bin = elf.with_extension("bin");
            BinUtil::objcopy()
                .arg(elf)
                .arg("--strip-all")
                .args(["-O", "binary"])
                .arg(&bin)
                .invoke();
            bin
        } else {
            elf
        }
    }
}

#[derive(Args)]
struct AsmArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// Output file.
    #[clap(short, long)]
    output: Option<String>,
}

impl AsmArgs {
    /// 如果没有设置 `kernel`，将 `rustsbi-qemu` 反汇编，并保存到指定位置。
    ///
    /// 如果设置了 `kernel` 是 'test' 或 'test-kernel'，将 `test-kernel` 反汇编，并保存到指定位置。
    ///
    /// 如果设置了 `kernel` 但不是 'test' 或 'test-kernel'，将 `kernel` 指定的二进制文件反汇编，并保存到指定位置。
    fn dump(self) {
        let elf = self.build.make(package(self.build.kernel.as_ref()), false);
        let out = project().join(self.output.unwrap_or(format!(
            "{}.asm",
            elf.file_stem().unwrap().to_string_lossy()
        )));
        println!("Asm file dumps to '{}'.", out.display());
        fs::write(out, BinUtil::objdump().arg(elf).arg("-d").output().stdout).unwrap();
    }
}

#[derive(Args, Default)]
struct QemuArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// Which sbi to use, open or rust.
    #[clap(long)]
    sbi: Option<String>,
    /// Number of hart (SMP for Symmetrical Multiple Processor).
    #[clap(long)]
    smp: Option<u8>,
    /// Port for gdb to connect. If set, qemu will block and wait gdb to connect.
    #[clap(long)]
    gdb: Option<u16>,
}

impl QemuArgs {
    fn run(mut self) {
        let sbi = self.sbi.take().unwrap_or_else(|| "rust".into());
        let sbi = match sbi.to_lowercase().as_str() {
            "rust" | "rustsbi" => self.build.make("rustsbi-qemu", true),
            "open" | "opensbi" => PathBuf::from("default"),
            _ => panic!(),
        };
        let kernel = self.build.kernel.take().unwrap_or_else(|| "test".into());
        let kernel = match kernel.to_lowercase().as_str() {
            "test" | "test-kernel" => self.build.make("test-kernel", true),
            "bench" | "bench-kernel" => self.build.make("bench-kernel", true),
            _ => panic!(),
        };
        let status = Qemu::system("riscv64")
            .args(["-machine", "virt"])
            .arg("-nographic")
            .arg("-bios")
            .arg(sbi)
            .arg("-kernel")
            .arg(kernel)
            .args(["-serial", "mon:stdio"])
            .args(["-smp", &self.smp.unwrap_or(8).to_string()])
            .optional(&self.gdb, |qemu, gdb| {
                qemu.args(["-S", "-gdb", &format!("tcp::{gdb}")]);
            })
            .as_mut()
            .status();
        if let Err(e) = status {
            if e.kind() == io::ErrorKind::NotFound {
                println!("xtask: QEMU command not found. Does your system have QEMU installed and environment variable configured?");
                println!("xtask: error: {e}");
            } else {
                println!("xtask: error: {e}");
            }
            process::exit(1);
        }
    }
}

fn package<T: AsRef<str>>(name: Option<T>) -> &'static str {
    if let Some(t) = name {
        match t.as_ref().to_lowercase().as_str() {
            "test" | "test-kernel" => "test-kernel",
            "bench" | "bench-kernel" => "bench-kernel",
            "rustsbi" | "rustsbi-qemu" => "rustsbi-qemu",
            _ => panic!(),
        }
    } else {
        "rustsbi-qemu"
    }
}

#[test]
fn test() {
    QemuArgs::default().run();
}
