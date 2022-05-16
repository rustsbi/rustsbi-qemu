use core::ops::Range;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use serde::Deserialize;
use serde_device_tree::{
    buildin::{NodeSeq, Reg, StrSeq},
    from_raw_mut, Dtb, DtbPtr,
};
use spin::Once;

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

static BOARD: Once<BoardInfo> = Once::new();

pub(crate) fn init(opaque: usize) {
    BOARD.call_once(|| {
        let ptr = DtbPtr::from_raw(opaque as _).unwrap();
        let dtb = Dtb::from(ptr).share();
        let t: Tree = from_raw_mut(&dtb).unwrap();

        BoardInfo {
            model: t.model.iter().map(|m| m.to_string()).collect(),
            smp: t.cpus.cpu.len(),
            memory: t
                .memory
                .iter()
                .map(|m| m.deserialize::<Memory>())
                .find(|m| m.device_type.iter().any(|t| t == "memory"))
                .map(|m| m.reg.iter().next().unwrap().0.clone())
                .unwrap(),
            rtc: take_one_peripheral(&t.soc.rtc, "google,goldfish-rtc"),
            uart: take_one_peripheral(&t.soc.uart, "ns16550a"),
            test: take_one_peripheral(&t.soc.test, "syscon"),
            pci: take_one_peripheral(&t.soc.pci, "pci-host-ecam-generic"),
            clint: take_one_peripheral(&t.soc.clint, "riscv,clint0"),
            plic: take_one_peripheral(&t.soc.plic, "riscv,plic0"),
        }
    });
}

pub(crate) fn get() -> &'static BoardInfo {
    BOARD.wait()
}

#[derive(Deserialize)]
struct Tree<'a> {
    model: StrSeq<'a>,
    cpus: Cpus<'a>,
    memory: NodeSeq<'a>,
    soc: Soc<'a>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Cpus<'a> {
    cpu: NodeSeq<'a>,
}

#[derive(Deserialize)]
struct Soc<'a> {
    rtc: NodeSeq<'a>,
    uart: NodeSeq<'a>,
    test: NodeSeq<'a>,
    pci: NodeSeq<'a>,
    clint: NodeSeq<'a>,
    plic: NodeSeq<'a>,
}

#[derive(Deserialize)]
struct Memory<'a> {
    device_type: StrSeq<'a>,
    reg: Reg<'a>,
}

#[derive(Deserialize)]
struct Peripheral<'a> {
    compatible: StrSeq<'a>,
    reg: Reg<'a>,
}

fn take_one_peripheral(nodes: &NodeSeq<'_>, compatible: &str) -> Range<usize> {
    nodes
        .iter()
        .map(|u| u.deserialize::<Peripheral>())
        .find(|u| u.compatible.iter().any(|s| s == compatible))
        .map(|u| u.reg.iter().next().unwrap().0.clone())
        .unwrap()
}
