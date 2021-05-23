use crate::runtime::SupervisorContext;
use crate::clint;

pub fn emulate_rdtime(ctx: &mut SupervisorContext, ins: usize) -> bool {
    if ins & 0xFFFFF07F == 0xC0102073 {
        let rd = ((ins >> 7) & 0b1_1111) as u8;
        let clint = clint::Clint::new(0x2000000 as *mut u8);
        let time_usize = clint.get_mtime() as usize;
        set_register_xi(ctx, rd, time_usize);
        ctx.mepc = ctx.mepc.wrapping_add(4); // 跳过指令
        return true;
    } else {
        return false;
    }
}

#[inline]
fn set_register_xi(ctx: &mut SupervisorContext, i: u8, data: usize) {
    match i {
        10 => ctx.a0 = data,
        11 => ctx.a1 = data,
        12 => ctx.a2 = data,
        13 => ctx.a3 = data,
        14 => ctx.a4 = data,
        15 => ctx.a5 = data,
        16 => ctx.a6 = data,
        17 => ctx.a7 = data,
        5 =>  ctx.t0 = data,
        6 =>  ctx.t1 = data,
        7 =>  ctx.t2 = data,
        28 => ctx.t3 = data,
        29 => ctx.t4 = data,
        30 => ctx.t5 = data,
        31 => ctx.t6 = data,
        _ => panic!("invalid target"),
    }
}
