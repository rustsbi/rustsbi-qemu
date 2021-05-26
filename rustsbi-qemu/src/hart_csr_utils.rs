use alloc::vec::Vec;
use riscv::register::{misa::{self, MXL}, medeleg, mideleg};
use rustsbi::{print, println};

pub fn print_misa_medeleg_mideleg() {
    print_misa();
    print_mideleg();
    print_medeleg();
}

#[inline]
fn print_misa() {
    let isa = misa::read();
    if let Some(isa) = isa {
        let mxl_str = match isa.mxl() {
            MXL::XLEN32 => "RV32",
            MXL::XLEN64 => "RV64",
            MXL::XLEN128 => "RV128",
        };
        print!("[rustsbi] misa: {}", mxl_str);
        for ext in 'A'..='Z' {
            if isa.has_extension(ext) {
                print!("{}", ext);
            }
        }
        println!("");
    }
}

#[inline]
fn print_mideleg() {
    let mideleg = mideleg::read();
    let mut delegs = Vec::new();
    if mideleg.usoft() {
        delegs.push("usoft")
    }
    if mideleg.utimer() {
        delegs.push("utimer")
    }
    if mideleg.uext() {
        delegs.push("uext")
    }
    if mideleg.ssoft() {
        delegs.push("ssoft")
    }
    if mideleg.stimer() {
        delegs.push("stimer")
    }
    if mideleg.sext() {
        delegs.push("sext")
    }
    println!("[rustsbi] mideleg: {}", delegs.join(", "));
}

#[inline]
fn print_medeleg() {
    let medeleg = medeleg::read();
    let mut delegs = Vec::new();
    if medeleg.instruction_misaligned() {
        delegs.push("ima")
    }
    if medeleg.instruction_fault() {
        delegs.push("ia") // instruction access
    }
    if medeleg.illegal_instruction() {
        delegs.push("illinsn")
    }
    if medeleg.breakpoint() {
        delegs.push("bkpt")
    }
    if medeleg.load_misaligned() {
        delegs.push("lma")
    }
    if medeleg.load_fault() {
        delegs.push("la") // load access
    }
    if medeleg.store_misaligned() {
        delegs.push("sma")
    }
    if medeleg.store_fault() {
        delegs.push("sa") // store access
    }
    if medeleg.user_env_call() {
        delegs.push("uecall")
    }
    if medeleg.supervisor_env_call() {
        delegs.push("secall")
    }
    if medeleg.machine_env_call() {
        delegs.push("mecall")
    }
    if medeleg.instruction_page_fault() {
        delegs.push("ipage")
    }
    if medeleg.load_page_fault() {
        delegs.push("lpage")
    }
    if medeleg.store_page_fault() {
        delegs.push("spage")
    }
    println!("[rustsbi] medeleg: {}", delegs.join(", "));
}
