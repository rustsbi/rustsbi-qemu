#[macro_use]
extern crate clap;

use clap::Parser;
use command_ext::{BinUtil, Cargo, CommandExt, Ext};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

lazy_static::lazy_static! {
    static ref PROJECT_DIR: &'static Path = Path::new(std::env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    static ref TARGET: PathBuf = PROJECT_DIR.join("target");
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
    Make(BuildArgs),
    Asm(AsmArgs),
    Qemu(QemuArgs),
}

fn main() {
    use Commands::*;
    match Cli::parse().command {
        Make(args) => args.make(),
        Asm(args) => args.dump(),
        Qemu(args) => args.run(),
    }
}

#[derive(Args, Default)]
struct BuildArgs {
    /// With supervisor.
    #[clap(short, long)]
    kernel: Option<String>,
    /// Target arch.
    #[clap(long)]
    target: Option<String>,
    /// Build in debug mode.
    #[clap(long)]
    debug: bool,
}

impl BuildArgs {
    /// Returns the build target name.
    fn target(&self) -> &str {
        self.target
            .as_ref()
            .map_or("riscv64imac-unknown-none-elf", |s| s.as_str())
    }

    fn arch(&self) -> &str {
        if self
            .target
            .as_ref()
            .map_or(false, |t| t.contains("riscv32"))
        {
            "riscv32"
        } else {
            "riscv64"
        }
    }

    /// Returns the dir of target files.
    fn dir(&self) -> PathBuf {
        PROJECT_DIR
            .join("target")
            .join(self.target())
            .join(if self.debug { "debug" } else { "release" })
    }

    /// 编译 `rustsbi-qemu`。
    ///
    /// 如果设置了 `kernel` 是 'test' 或 'test-kernel'，同时编译 `test-kernel`。
    ///
    /// 如果设置了 `kernel` 但不是 'test' 或 'test-kernel'，则检查 `kernel` 是一个编译好的二进制文件。
    fn make(&self) {
        self.make_package("rustsbi-qemu");
        if let Some(ref kernel) = self.kernel {
            if kernel == "test" || kernel == "test-kernel" {
                self.make_package("test-kernel");
            } else {
                todo!("检查内核是一个二进制文件");
            }
        }
    }

    fn make_package(&self, package: &str) {
        // 生成
        Cargo::build()
            .package(package)
            .conditional(!self.debug, |sbi| {
                sbi.release();
            })
            .target(self.target())
            .invoke();
        // 裁剪
        let target = self.dir().join(package);
        BinUtil::objcopy()
            .arg(format!("--binary-architecture={}", self.arch()))
            .arg(&target)
            .arg("--strip-all")
            .arg("-O")
            .arg("binary")
            .arg(target.with_extension("bin"))
            .invoke();
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
        self.build.make();
        let bin = if let Some(kernel) = &self.build.kernel {
            if kernel == "test" || kernel == "test-kernel" {
                self.build.dir().join("test-kernel")
            } else {
                PathBuf::from(kernel)
            }
        } else {
            self.build.dir().join("rustsbi-qemu")
        };
        let out = PROJECT_DIR.join(self.output.unwrap_or(format!(
            "{}.asm",
            bin.file_stem().unwrap().to_string_lossy()
        )));
        println!("Asm file dumps to '{}'.", out.display());
        fs::write(out, BinUtil::objdump().arg(bin).arg("-d").output().stdout).unwrap();
    }
}

#[derive(Args, Default)]
struct QemuArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// Path of executable qemu-system-x.
    #[clap(long)]
    qemu_dir: Option<String>,
    /// Number of hart (SMP for Symmetrical Multiple Processor).
    #[clap(long)]
    smp: Option<u8>,
    /// Port for gdb to connect. If set, qemu will block and wait gdb to connect.
    #[clap(long)]
    gdb: Option<u16>,
}

impl QemuArgs {
    fn find_qemu(&self) -> OsString {
        let name = if cfg!(target_os = "windows") {
            format!("qemu-system-{}.exe", self.build.arch())
        } else {
            format!("qemu-system-{}", self.build.arch())
        };
        if let Some(path) = &self.qemu_dir {
            let target = PathBuf::from(path).join(&name);
            if target.is_file() {
                return target.into_os_string();
            }
        }
        #[cfg(target_os = "windows")]
        {
            let target = PathBuf::from(r"C:\Program Files\qemu").join(&name);
            if target.is_file() {
                return target.into_os_string();
            }
        }
        OsString::from(name)
    }

    fn run(mut self) {
        self.build.kernel.get_or_insert_with(|| "test".into());
        self.build.make();
        Ext::new(self.find_qemu())
            .args(&["-machine", "virt"])
            .arg("-bios")
            .arg(self.build.dir().join("rustsbi-qemu.bin"))
            .arg("-kernel")
            .arg(self.build.dir().join("test-kernel.bin"))
            .args(&["-smp", &self.smp.unwrap_or(8).to_string()])
            .args(&["-serial", "mon:stdio"])
            .arg("-nographic")
            .optional(&self.gdb, |qemu, gdb| {
                qemu.args(&["-gdb", &format!("tcp::{gdb}")]);
            })
            .invoke();
    }
}

#[test]
fn test() {
    QemuArgs::default().run();
}
