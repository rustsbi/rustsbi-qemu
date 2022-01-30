use alloc::format;
use alloc::vec::Vec;
use bit_field::BitField;
use riscv::register::{
    medeleg, mideleg,
    misa::{self, MXL},
};

pub fn print_hart_csrs() {
    print_misa();
    print_mideleg();
    print_medeleg();
    print_pmp();
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

#[cfg(target_pointer_width = "64")]
#[inline]
fn print_pmp() {
    let pmps = unsafe { pmps::<16>() };
    for (i, (pmpicfg, pmpiaddr)) in pmps.iter().enumerate() {
        let pmpicfg = PmpCfg::from(*pmpicfg);
        let range = match pmpicfg.a() {
            AddressMatching::Off => continue,
            AddressMatching::Tor => (0, (1 << (55 + 1)) - 1), // max pmp bits = 55
            AddressMatching::Na4 => ((*pmpiaddr as u128) << 2, ((*pmpiaddr as u128) << 2) + 4),
            AddressMatching::Napot => napot_pmpaddr_cfg(*pmpiaddr as u128),
        };
        let range = format!("{:#x} ..= {:#x}", range.0, range.1);
        let privilege = format!(
            "{}{}{}",
            if pmpicfg.r() { "r" } else { "-" },
            if pmpicfg.w() { "w" } else { "-" },
            if pmpicfg.x() { "x" } else { "-" },
        );
        let l = if pmpicfg.l() { "l, " } else { "" };
        println!("[rustsbi] pmp{}: {} ({}{})", i, range, privilege, l);
    }
}

fn napot_pmpaddr_cfg(input: u128) -> (u128, u128) {
    let trailing_ones = input.trailing_ones();
    if trailing_ones == 0 {
        return (input, input);
    }
    let mask = (1 << trailing_ones) - 1;
    ((input - mask) << 2, ((input + 1) << 2) - 1)
}

struct PmpCfg {
    bits: u8,
}

impl From<u8> for PmpCfg {
    fn from(bits: u8) -> PmpCfg {
        PmpCfg { bits }
    }
}

impl PmpCfg {
    #[inline]
    pub fn r(&self) -> bool {
        self.bits.get_bit(0)
    }
    #[inline]
    pub fn w(&self) -> bool {
        self.bits.get_bit(1)
    }
    #[inline]
    pub fn x(&self) -> bool {
        self.bits.get_bit(2)
    }
    #[inline]
    pub fn a(&self) -> AddressMatching {
        match self.bits.get_bits(3..5) {
            0 => AddressMatching::Off,
            1 => AddressMatching::Tor,
            2 => AddressMatching::Na4,
            3 => AddressMatching::Napot,
            _ => unreachable!(),
        }
    }
    #[inline]
    pub fn l(&self) -> bool {
        self.bits.get_bit(7)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressMatching {
    Off,
    Tor,
    Na4,
    Napot,
}

// 1.12中，L=64；1.11中，L=16。
// 0..16 => pmpcfg[0, 2]
// 0..64 => pmpcfg[0, 2, 4, 6, .., 14]
#[inline]
unsafe fn pmps<const L: usize>() -> [(u8, usize); L] {
    assert!(L < 64, "in pmpxcfg, x should be in [0, 64)");
    let xlen: usize = core::mem::size_of::<usize>() * 8;
    let cfgs_in_pmpcfg: usize = xlen / 8;
    let pmpcfg_max_id: usize = L / cfgs_in_pmpcfg;
    let mut ans = [(0, 0); L];
    for i in (0..pmpcfg_max_id).step_by(xlen / 32) {
        let pmpcfgi = pmpcfg_r(i).to_le_bytes();
        for j in 0..cfgs_in_pmpcfg {
            let pmpaddr_id = i * 4 + j;
            let pmpaddri = pmpaddr_r(pmpaddr_id);
            ans[pmpaddr_id] = (pmpcfgi[j], pmpaddri);
        }
    }
    ans
}

// 1.12版本中，pmpcfg总共有16个，其中64位下只能访问偶数个，32位下可以访问所有寄存器
// 1.11版本中，pmpcfg只有4个。有些模拟器最多只支持4个pmp寄存器，大于4的编号会出错。
#[inline]
unsafe fn pmpcfg_r(pmpcfg_id: usize) -> usize {
    assert!(pmpcfg_id <= 15, "pmpcfg id should be in [0, 15]");
    let ans: usize;
    core::arch::asm!(
    // tmp <- 1的地址；len <- csrr和j指令的长度和
    "la     {tmp}, 1f
    la      {len}, 2f
    sub     {len}, {len}, {tmp}",
    // tmp <- tmp + id * len(csrr + j)
    "mul    {id}, {id}, {len}
    add     {tmp}, {tmp}, {id}
    jr      {tmp}",
"1:  csrr   {ans}, 0x3A0", "j   1f",
"2:  csrr   {ans}, 0x3A1", "j   1f",
    "csrr   {ans}, 0x3A2", "j   1f",
    "csrr   {ans}, 0x3A3", "j   1f",
    "csrr   {ans}, 0x3A4", "j   1f",
    "csrr   {ans}, 0x3A5", "j   1f",
    "csrr   {ans}, 0x3A6", "j   1f",
    "csrr   {ans}, 0x3A7", "j   1f",
    "csrr   {ans}, 0x3A8", "j   1f",
    "csrr   {ans}, 0x3A9", "j   1f",
    "csrr   {ans}, 0x3AA", "j   1f",
    "csrr   {ans}, 0x3AB", "j   1f",
    "csrr   {ans}, 0x3AC", "j   1f",
    "csrr   {ans}, 0x3AD", "j   1f",
    "csrr   {ans}, 0x3AE", "j   1f",
    "csrr   {ans}, 0x3AF", "j   1f",
"1:", 
    id = in(reg) pmpcfg_id, tmp = out(reg) _, len = out(reg) _, ans = out(reg) ans);
    ans
}

// 1.12中有63个，但1.11中只有15个。个别模拟器需要注意，详见上文
#[inline]
unsafe fn pmpaddr_r(pmpaddr_id: usize) -> usize {
    assert!(pmpaddr_id <= 63, "pmpcfg id should be in [0, 63]");
    let ans: usize;
    core::arch::asm!(
    // tmp <- 1的地址；len <- csrr和j指令的长度和
    "la     {tmp}, 1f
    la      {len}, 2f
    sub     {len}, {len}, {tmp}",
    // tmp <- tmp + id * len(csrr + j)
    "mul    {id}, {id}, {len}
    add     {tmp}, {tmp}, {id}
    jr      {tmp}",
"1:  csrr   {ans}, 0x3B0", "j   1f",
"2:  csrr   {ans}, 0x3B1", "j   1f",
    "csrr   {ans}, 0x3B2", "j   1f", "csrr   {ans}, 0x3B3", "j   1f",
    "csrr   {ans}, 0x3B4", "j   1f", "csrr   {ans}, 0x3B5", "j   1f",
    "csrr   {ans}, 0x3B6", "j   1f", "csrr   {ans}, 0x3B7", "j   1f",
    "csrr   {ans}, 0x3B8", "j   1f", "csrr   {ans}, 0x3B9", "j   1f",
    "csrr   {ans}, 0x3BA", "j   1f", "csrr   {ans}, 0x3BB", "j   1f",
    "csrr   {ans}, 0x3BC", "j   1f", "csrr   {ans}, 0x3BD", "j   1f",
    "csrr   {ans}, 0x3BE", "j   1f", "csrr   {ans}, 0x3BF", "j   1f",
    "csrr   {ans}, 0x3C0", "j   1f", "csrr   {ans}, 0x3C1", "j   1f",
    "csrr   {ans}, 0x3C2", "j   1f", "csrr   {ans}, 0x3C3", "j   1f",
    "csrr   {ans}, 0x3C4", "j   1f", "csrr   {ans}, 0x3C5", "j   1f",
    "csrr   {ans}, 0x3C6", "j   1f", "csrr   {ans}, 0x3C7", "j   1f",
    "csrr   {ans}, 0x3C8", "j   1f", "csrr   {ans}, 0x3C9", "j   1f",
    "csrr   {ans}, 0x3CA", "j   1f", "csrr   {ans}, 0x3CB", "j   1f",
    "csrr   {ans}, 0x3CC", "j   1f", "csrr   {ans}, 0x3CD", "j   1f",
    "csrr   {ans}, 0x3CE", "j   1f", "csrr   {ans}, 0x3CF", "j   1f",
    "csrr   {ans}, 0x3D0", "j   1f", "csrr   {ans}, 0x3D1", "j   1f",
    "csrr   {ans}, 0x3D2", "j   1f", "csrr   {ans}, 0x3D3", "j   1f",
    "csrr   {ans}, 0x3D4", "j   1f", "csrr   {ans}, 0x3D5", "j   1f",
    "csrr   {ans}, 0x3D6", "j   1f", "csrr   {ans}, 0x3D7", "j   1f",
    "csrr   {ans}, 0x3D8", "j   1f", "csrr   {ans}, 0x3D9", "j   1f",
    "csrr   {ans}, 0x3DA", "j   1f", "csrr   {ans}, 0x3DB", "j   1f",
    "csrr   {ans}, 0x3DC", "j   1f", "csrr   {ans}, 0x3DD", "j   1f",
    "csrr   {ans}, 0x3DE", "j   1f", "csrr   {ans}, 0x3DF", "j   1f",
    "csrr   {ans}, 0x3E0", "j   1f", "csrr   {ans}, 0x3E1", "j   1f",
    "csrr   {ans}, 0x3E2", "j   1f", "csrr   {ans}, 0x3E3", "j   1f",
    "csrr   {ans}, 0x3E4", "j   1f", "csrr   {ans}, 0x3E5", "j   1f",
    "csrr   {ans}, 0x3E6", "j   1f", "csrr   {ans}, 0x3E7", "j   1f",
    "csrr   {ans}, 0x3E8", "j   1f", "csrr   {ans}, 0x3E9", "j   1f",
    "csrr   {ans}, 0x3EA", "j   1f", "csrr   {ans}, 0x3EB", "j   1f",
    "csrr   {ans}, 0x3EC", "j   1f", "csrr   {ans}, 0x3ED", "j   1f",
    "csrr   {ans}, 0x3EE", "j   1f", "csrr   {ans}, 0x3EF", "j   1f",
"1:", 
    id = in(reg) pmpaddr_id, tmp = out(reg) _, len = out(reg) _, ans = out(reg) ans);
    ans
}
