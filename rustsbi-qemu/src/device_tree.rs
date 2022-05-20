use alloc::{string::String, vec, vec::Vec};
use core::ops::Range;

pub(crate) struct BoardInfo {
    pub model: Vec<String>,
    pub smp: usize,
    pub memory: Range<usize>,
    pub rtc: Range<usize>,
    pub uart: Range<usize>,
    pub test: Range<usize>,
    pub pci: Range<usize>,
    pub clint: Range<usize>,
    pub plic: Range<usize>,
}

pub(crate) fn parse(_opaque: usize) -> BoardInfo {
    BoardInfo {
        model: vec![String::from("riscv-virtio,qemu")],
        smp: 8,
        memory: 0x8000_0000..0x8800_0000,
        rtc: 0x101000..0x102000,
        uart: 0x1000_0000..0x1000_0100,
        test: 0x10_0000..0x10_1000,
        pci: 0x3000_0000..0x4000_0000,
        clint: 0x200_0000..0x201_0000,
        plic: 0xc00_0000..0xc21_0000,
    }
}
