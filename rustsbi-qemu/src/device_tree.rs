use alloc::{string::String, vec, vec::Vec};
use core::ops::Range;

pub(crate) struct BoardInfo {
    pub model: Vec<String>,
    pub smp: usize,
    pub uart: Range<usize>,
    pub test: Range<usize>,
    pub clint: Range<usize>,
}

pub(crate) fn parse(_opaque: usize) -> BoardInfo {
    BoardInfo {
        model: vec![String::from("riscv-virtio,qemu")],
        smp: 4,
        uart: 0x1000_0000..0x1000_0100,
        test: 0x10_0000..0x10_1000,
        clint: 0x200_0000..0x201_0000,
    }
}
