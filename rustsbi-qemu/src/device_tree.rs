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
    use dtb_walker::{Dtb, DtbObj, HeaderError as E, Property, Str, WalkOperation::*};
    const CPUS: &str = "cpus";
    const MEMORY: &str = "memory";
    const SOC: &str = "soc";
    const UART: &str = "uart";
    const SERIAL: &str = "serial";
    const TEST: &str = "test";
    const CLINT: &str = "clint";

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
            matches!(e, E::Misaligned(4) | E::LastCompVersion(_))
        })
    }
    .unwrap();
    ans.dtb.end += dtb.total_size();
    dtb.walk(|ctx, obj| match obj {
        DtbObj::SubNode { name } => {
            let current = ctx.name();
            if ctx.is_root() {
                if name == Str::from(CPUS) || name == Str::from(SOC) || name.starts_with(MEMORY) {
                    StepInto
                } else {
                    StepOver
                }
            } else if current == Str::from(SOC) {
                if name.starts_with(UART)
                    || name.starts_with(SERIAL)
                    || name.starts_with(TEST)
                    || name.starts_with(CLINT)
                {
                    StepInto
                } else {
                    StepOver
                }
            } else {
                if current == Str::from(CPUS) && name.starts_with("cpu@") {
                    ans.smp += 1;
                }
                StepOver
            }
        }
        DtbObj::Property(Property::Model(model)) if ctx.is_root() => {
            ans.model.0 = model.as_bytes().len();
            ans.model.1[..ans.model.0].copy_from_slice(model.as_bytes());
            StepOver
        }
        DtbObj::Property(Property::Reg(mut reg)) => {
            let node = ctx.name();
            if node.starts_with(UART) || node.starts_with(SERIAL) {
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
