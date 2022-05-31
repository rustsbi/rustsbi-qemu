use alloc::{string::String, vec, vec::Vec};
use core::ops::Range;

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
    use dtb_walker::{Dtb, DtbObj, Property, WalkOperation};

    let mut ans = BoardInfo {
        dtb: opaque..opaque,
        model: String::new(),
        smp: 0,
        mem: vec![],
        uart: 0x1000_0000..0x1000_0100,
        test: 0x10_0000..0x10_1000,
        clint: 0x200_0000..0x201_0000,
    };
    let dtb = unsafe { Dtb::from_raw_parts(opaque as _) }.unwrap();
    ans.dtb.end += dtb.total_size();
    dtb.walk(|path, obj| match obj {
        DtbObj::SubNode { name } => {
            if path.last().is_empty() && name == b"cpus" {
                WalkOperation::StepInto
            } else if path.last() == b"cpus" && name.starts_with(b"cpu@") {
                ans.smp += 1;
                WalkOperation::StepOver
            } else if path.last().is_empty() && name.starts_with(b"memory") {
                WalkOperation::StepInto
            } else {
                WalkOperation::StepOver
            }
        }
        DtbObj::Property(Property::Model(model)) if path.last().is_empty() => {
            if let Ok(model) = model.as_str() {
                ans.model = model.into();
            }
            WalkOperation::StepOver
        }
        DtbObj::Property(Property::Reg(reg)) if path.last().starts_with(b"memory") => {
            for region in reg {
                ans.mem.push(region);
            }
            WalkOperation::StepOut
        }
        DtbObj::Property(_) => WalkOperation::StepOver,
    });

    ans
}
