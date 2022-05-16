use alloc::vec::Vec;
use riscv::register::{
    medeleg, mideleg,
    misa::{self, MXL},
};

pub fn print_hart_csrs() {
    print_misa();
    print_mideleg();
    print_medeleg();
    print_pmps();
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
    println!(
        "[rustsbi] mideleg: {} ({:#x})",
        delegs.join(", "),
        mideleg.bits()
    );
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
    println!(
        "[rustsbi] medeleg: {} ({:#x})",
        delegs.join(", "),
        medeleg.bits()
    );
}

// TODO riscv32
// TODO riscv 只实现了前 16 个
#[cfg(target_arch = "riscv64")]
fn print_pmps() {
    const ITEM_PER_CFG: usize = core::mem::size_of::<usize>();
    const CFG_STEP: usize = ITEM_PER_CFG / core::mem::size_of::<u32>();

    let mut i_cfg = 0;
    while i_cfg < 4 {
        let base = i_cfg * ITEM_PER_CFG;
        let mut cfg = pmpcfg(i_cfg);
        let mut i_addr = 0;
        while i_addr < ITEM_PER_CFG {
            let step = match (cfg >> 3) & 0b11 {
                0b00 => 1,
                0b01 => {
                    dump_pmp(
                        base + i_addr,
                        pmpaddr(base + i_addr) << 2,
                        pmpaddr(base + i_addr + 1) << 2,
                        cfg & 0b111,
                    );
                    2
                }
                0b10 => {
                    let s = pmpaddr(base + i_addr);
                    dump_pmp(base + i_addr, s << 2, (s + 1) << 2, cfg & 0b111);
                    1
                }
                0b11 => {
                    let addr = pmpaddr(base + i_addr);
                    let len = 1usize << (addr.trailing_ones() + 2);
                    let s = (addr & !(len - 1)) << 2;
                    let e = s + len;
                    dump_pmp(base + i_addr, s, e, cfg & 0b111);
                    1
                }
                _ => unreachable!(),
            };
            cfg >>= 8;
            i_addr += step;
        }
        i_cfg += CFG_STEP;
    }
}

fn dump_pmp(i: usize, s: usize, e: usize, permission: usize) {
    let permission = match permission {
        0b000 => "---",
        0b100 => "x--",
        0b010 => "-w-",
        0b001 => "--r",
        0b110 => "xw-",
        0b101 => "x-r",
        0b011 => "-wr",
        0b111 => "xwr",
        _ => unreachable!(),
    };
    println!(
        "[rustsbi] pmp{}: {:#010x}..{:#010x} ({})",
        i, s, e, permission
    );
}

fn pmpcfg(i: usize) -> usize {
    use riscv::register::*;
    match i {
        0 => pmpcfg0::read().bits,
        #[cfg(target_arch = "riscv32")]
        1 => pmpcfg1::read().bits,
        2 => pmpcfg2::read().bits,
        #[cfg(target_arch = "riscv32")]
        3 => pmpcfg3::read().bits,
        _ => todo!(),
    }
}

fn pmpaddr(i: usize) -> usize {
    use riscv::register::*;
    match i {
        0x0 => pmpaddr0::read(),
        0x1 => pmpaddr1::read(),
        0x2 => pmpaddr2::read(),
        0x3 => pmpaddr3::read(),
        0x4 => pmpaddr4::read(),
        0x5 => pmpaddr5::read(),
        0x6 => pmpaddr6::read(),
        0x7 => pmpaddr7::read(),
        0x8 => pmpaddr8::read(),
        0x9 => pmpaddr9::read(),
        0xa => pmpaddr10::read(),
        0xb => pmpaddr11::read(),
        0xc => pmpaddr12::read(),
        0xd => pmpaddr13::read(),
        0xe => pmpaddr14::read(),
        0xf => pmpaddr15::read(),
        _ => todo!(),
    }
}
