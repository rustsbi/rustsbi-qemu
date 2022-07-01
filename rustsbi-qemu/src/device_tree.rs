use core::{
    fmt::{Display, Formatter, Result},
    ops::Range,
};

/// 从设备树采集的板信息。
pub(crate) struct BoardInfo {
    pub dtb: Range<usize>,
    pub model: StringInline<128>,
    pub smp: usize,
    pub mem: Range<usize>,
    pub uart: Range<usize>,
    pub test: Range<usize>,
    pub clint: Range<usize>,
}

/// 在栈上存储有限长度字符串。
pub(crate) struct StringInline<const N: usize>(usize, [u8; N]);

impl<const N: usize> Display for StringInline<N> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", unsafe {
            core::str::from_utf8_unchecked(&self.1[..self.0])
        })
    }
}

/// 解析设备树。
pub(crate) fn parse(opaque: usize) -> BoardInfo {
    use dtb_walker::{Dtb, DtbObj, HeaderError as E, Property, WalkOperation::*};
    const CPUS: &[u8] = b"cpus";
    const MEMORY: &[u8] = b"memory";
    const SOC: &[u8] = b"soc";
    const UART: &[u8] = b"uart";
    const TEST: &[u8] = b"test";
    const CLINT: &[u8] = b"clint";

    let mut ans = BoardInfo {
        dtb: opaque..opaque,
        model: StringInline(0, [0u8; 128]),
        smp: 0,
        mem: 0..0,
        uart: 0..0,
        test: 0..0,
        clint: 0..0,
    };
    let dtb = unsafe {
        Dtb::from_raw_parts_filtered(opaque as _, |e| {
            matches!(e, E::Misaligned(4) | E::LastCompVersion(16))
        })
    }
    .unwrap();
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
            ans.model.0 = model.as_bytes().len();
            ans.model.1[..ans.model.0].copy_from_slice(model.as_bytes());
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
                ans.mem = reg.next().unwrap();
                StepOut
            } else {
                StepOver
            }
        }
        DtbObj::Property(_) => StepOver,
    });

    ans
}
