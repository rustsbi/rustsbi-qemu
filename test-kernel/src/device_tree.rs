use serde::Deserialize;
use serde_device_tree::{buildin::NodeSeq, from_raw_mut, Dtb, DtbPtr};

pub(crate) fn parse_smp(dtb_pa: usize) -> usize {
    let ptr = DtbPtr::from_raw(dtb_pa as _).unwrap();
    let dtb = Dtb::from(ptr).share();
    let t: Tree = from_raw_mut(&dtb).unwrap();
    t.cpus.cpu.len()
}

#[derive(Deserialize)]
struct Tree<'a> {
    cpus: Cpus<'a>,
}

#[derive(Deserialize)]
struct Cpus<'a> {
    cpu: NodeSeq<'a>,
}
