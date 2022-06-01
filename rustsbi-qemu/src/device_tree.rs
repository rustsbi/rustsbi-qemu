use alloc::{string::String, vec, vec::Vec};
use core::ops::Range;

#[derive(Debug)]
pub(crate) struct BoardInfo {
    pub dtb: Range<usize>,
    pub model: String,
    pub smp: usize,
    pub mem: Vec<Range<usize>>,
    pub uart: Range<usize>,
    pub test: Range<usize>,
    pub clint: Range<usize>,
}

pub(crate) fn parse(opaque: usize) -> BoardInfo {
    use dtb_walker::{Dtb, DtbObj, Property, WalkOperation::*};
    const CPUS: &[u8] = b"cpus";
    const MEMORY: &[u8] = b"memory";
    const SOC: &[u8] = b"soc";
    const UART: &[u8] = b"uart";
    const TEST: &[u8] = b"test";
    const CLINT: &[u8] = b"clint";

    let mut ans = BoardInfo {
        dtb: opaque..opaque,
        model: String::new(),
        smp: 0,
        mem: vec![],
        uart: 0..0,
        test: 0..0,
        clint: 0..0,
    };
    let dtb = unsafe { Dtb::from_raw_parts(opaque as _) }.unwrap();
    ans.dtb.end += dtb.total_size();
    dtb.walk(|path, obj| match obj {
        DtbObj::SubNode { name } => {
            let current = path.last();
            if current.is_empty() {
                if name == CPUS || name == SOC || name.starts_with(MEMORY) {
                    StepInto
                } else {
                    StepOver
                }
            } else if current == SOC {
                if name.starts_with(UART) || name.starts_with(TEST) || name.starts_with(CLINT) {
                    StepInto
                } else {
                    StepOver
                }
            } else {
                if current == CPUS && name.starts_with(b"cpu@") {
                    ans.smp += 1;
                }
                StepOver
            }
        }
        DtbObj::Property(Property::Model(model)) if path.last().is_empty() => {
            if let Ok(model) = model.as_str() {
                ans.model = model.into();
            }
            StepOver
        }
        DtbObj::Property(Property::Reg(mut reg)) => {
            let node = path.last();
            if node.starts_with(UART) {
                ans.uart = reg.next().unwrap();
                StepOut
            } else if node.starts_with(TEST) {
                ans.test = reg.next().unwrap();
                StepOut
            } else if node.starts_with(CLINT) {
                ans.clint = reg.next().unwrap();
                StepOut
            } else if node.starts_with(MEMORY) {
                ans.mem = reg.into_iter().collect();
                StepOut
            } else {
                StepOver
            }
        }
        DtbObj::Property(_) => StepOver,
    });

    ans
}
