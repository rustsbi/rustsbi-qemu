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
    pub memory: Vec<Range<usize>>,
    pub smp: usize,
}

static BOARD: Once<BoardInfo> = Once::new();

pub(crate) fn init(opaque: usize) {
    BOARD.call_once(|| {
        let ptr = DtbPtr::from_raw(opaque as _).unwrap();
        let dtb = Dtb::from(ptr).share();
        let t: Tree = from_raw_mut(&dtb).unwrap();
        BoardInfo {
            model: t.model.iter().map(|m| m.to_string()).collect(),
            memory: t
                .memory
                .iter()
                .map(|m| m.deserialize::<Memory>())
                .filter(|m| m.device_type.iter().any(|t| t == "memory"))
                .flat_map(|m| m.reg.iter().map(|r| r.0).collect::<Vec<_>>())
                .collect(),
            smp: t.cpus.cpu.len(),
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
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Cpus<'a> {
    cpu: NodeSeq<'a>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct Cpu<'a> {
    compatible: StrSeq<'a>,
    device_type: StrSeq<'a>,
    status: StrSeq<'a>,
    #[serde(rename = "riscv,isa")]
    isa: StrSeq<'a>,
    #[serde(rename = "mmu-type")]
    mmu: StrSeq<'a>,
}

#[derive(Deserialize)]
struct Memory<'a> {
    device_type: StrSeq<'a>,
    reg: Reg<'a>,
}
