#[allow(unused, missing_docs)]

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
