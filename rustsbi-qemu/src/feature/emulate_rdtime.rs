use crate::clint;
use crate::runtime::SupervisorContext;

#[inline]
pub fn emulate_rdtime(ctx: &mut SupervisorContext, ins: usize) -> bool {
    return if ins & 0xFFFFF07F == 0xC0102073 {
        let rd = ((ins >> 7) & 0b1_1111) as u8;
        let clint = clint::Clint::new(0x2000000 as *mut u8);
        let time_usize = clint.get_mtime() as usize;
        set_register_xi(ctx, rd, time_usize);
        ctx.mepc = ctx.mepc.wrapping_add(4); // skip rdtime instruction
        true
    } else {
        false // is not a rdtime instruction
    };
}

#[inline]
fn set_register_xi(ctx: &mut SupervisorContext, i: u8, data: usize) {
    let registers = unsafe { &mut *(ctx as *mut _ as *mut [usize; 31]) };
    assert!(i <= 31, "i should be valid register target");
    if i == 0 {
        // x0, don't modify
        return;
    }
    registers[(i - 1) as usize] = data;
}
