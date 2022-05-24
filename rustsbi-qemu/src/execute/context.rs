use crate::{hart_id, Supervisor};

#[repr(C)]
#[derive(Debug)]
pub(super) struct Context {
    msp: usize,
    x: [usize; 31],
    pub mstatus: usize,
    pub mepc: usize,
}

impl Context {
    pub fn new(supervisor: Supervisor) -> Self {
        let mut ctx = Context {
            msp: 0,
            x: [0; 31],
            mstatus: 0,
            mepc: supervisor.start_addr,
        };

        unsafe { core::arch::asm!("csrr {}, mstatus", out(reg) ctx.mstatus) };
        *ctx.a_mut(0) = hart_id();
        *ctx.a_mut(1) = supervisor.opaque;

        ctx
    }

    #[inline]
    pub fn a(&self, n: usize) -> usize {
        self.x[n + 9]
    }

    #[inline]
    pub fn a_mut(&mut self, n: usize) -> &mut usize {
        &mut self.x[n + 9]
    }

    // #[inline]
    // pub fn x(&self, n: usize) -> usize {
    //     self.x[n - 1]
    // }

    // #[inline]
    // pub fn x_mut(&mut self, n: usize) -> &mut usize {
    //     &mut self.x[n - 1]
    // }
}
