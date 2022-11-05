#![allow(unused, missing_docs)]

pub mod mie {
    use core::arch::asm;

    pub const SSIE: usize = 1 << 1;
    pub const VSSIE: usize = 1 << 2;
    pub const MSIE: usize = 1 << 3;
    pub const STIE: usize = 1 << 5;
    pub const VSTIE: usize = 1 << 6;
    pub const MTIE: usize = 1 << 7;
    pub const SEIE: usize = 1 << 9;
    pub const VSEIE: usize = 1 << 10;
    pub const MEIE: usize = 1 << 11;
    pub const SGEIE: usize = 1 << 12;

    #[inline(always)]
    pub fn write(bits: usize) {
        unsafe { asm!("csrw mie, {}", in(reg) bits, options(nomem)) };
    }
}

pub mod mstatus {
    use core::arch::asm;

    pub const SIE: usize = 1 << 1;
    pub const MIE: usize = 1 << 3;
    pub const SPIE: usize = 1 << 5;
    pub const MPIE: usize = 1 << 7;
    pub const SPP: usize = 1 << 8;
    pub const VS: usize = 3 << 9;
    pub const MPP: usize = 3 << 11;
    pub const FS: usize = 3 << 13;
    pub const XS: usize = 3 << 15;
    pub const MPRV: usize = 1 << 17;
    pub const SUM: usize = 1 << 18;
    pub const MXR: usize = 1 << 19;
    pub const TVM: usize = 1 << 20;
    pub const TW: usize = 1 << 21;
    pub const TSR: usize = 1 << 22;
    pub const UXL: usize = 3 << 32;
    pub const SXL: usize = 3 << 34;
    pub const SBE: usize = 1 << 36;
    pub const MBE: usize = 1 << 37;
    pub const SD: usize = 1 << 63;

    pub const MPP_MACHINE: usize = 3 << 11;
    pub const MPP_SUPERVISOR: usize = 1 << 11;
    pub const MPP_USER: usize = 0 << 11;

    pub fn update(f: impl FnOnce(&mut usize)) {
        let mut bits: usize;
        unsafe { asm!("csrr {}, mstatus", out(reg) bits, options(nomem)) };
        f(&mut bits);
        unsafe { asm!("csrw mstatus, {}", in(reg) bits, options(nomem)) };
    }
}

pub mod mepc {
    use core::arch::asm;

    #[inline(always)]
    pub fn next() {
        unsafe {
            asm!(
                "   csrr {0}, mepc
                    addi {0}, {0}, 4
                    csrw mepc, {0}
                ",
                out(reg) _,
                options(nomem),
            )
        }
    }

    #[inline(always)]
    pub fn read() -> usize {
        let bits: usize;
        unsafe { asm!("csrr {}, mepc", out(reg) bits, options(nomem)) };
        bits
    }

    #[inline(always)]
    pub fn write(bits: usize) {
        unsafe { asm!("csrw mepc, {}", in(reg) bits, options(nomem)) };
    }
}
