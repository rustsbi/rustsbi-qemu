use crate::{fast_handler, hart_id, Supervisor, LEN_STACK_PER_HART, NUM_HART_MAX};
use core::{
    mem::{forget, size_of},
    ptr::NonNull,
};
use fast_trap::{FlowContext, FreeTrapStack, TrapStackBlock};
use hsm_cell::{HsmCell, LocalHsmCell, RemoteHsmCell};

/// 栈空间。
#[link_section = ".bss.uninit"]
static mut ROOT_STACK: [Stack; NUM_HART_MAX] = [Stack::ZERO; NUM_HART_MAX];

#[naked]
pub(crate) unsafe extern "C" fn locate() {
    core::arch::asm!(
        "   la   sp, {stack}
            li   t0, {per_hart_stack_size}
            csrr t1, mhartid
            addi t1, t1,  1
         1: add  sp, sp, t0
            addi t1, t1, -1
            bnez t1, 1b
            call t1, {move_stack}
            ret
        ",
        per_hart_stack_size = const LEN_STACK_PER_HART,
        stack               =   sym ROOT_STACK,
        move_stack          =   sym fast_trap::reuse_stack_for_trap,
        options(noreturn),
    )
}

pub(crate) fn load() {
    unsafe { ROOT_STACK.get_unchecked_mut(hart_id()).load_as_stack() };
}

pub(crate) fn local_hsm() -> LocalHsmCell<'static, Supervisor> {
    unsafe {
        ROOT_STACK
            .get_unchecked_mut(hart_id())
            .hart_context()
            .hsm
            .local()
    }
}

pub(crate) fn local_remote_hsm() -> RemoteHsmCell<'static, Supervisor> {
    unsafe {
        ROOT_STACK
            .get_unchecked_mut(hart_id())
            .hart_context()
            .hsm
            .remote()
    }
}

pub(crate) fn remote_hsm(i: usize) -> Option<RemoteHsmCell<'static, Supervisor>> {
    unsafe { ROOT_STACK.get_mut(i).map(|x| x.hart_context().hsm.remote()) }
}

/// 类型化栈。
///
/// 每个硬件线程拥有一个满足这样条件的内存块。
/// 这个内存块的底部放着硬件线程状态 [`HartContext`]，顶部用于陷入处理，中间是这个硬件线程的栈空间。
/// 不需要 M 态线程，每个硬件线程只有这一个栈。
#[repr(C, align(128))]
struct Stack([u8; LEN_STACK_PER_HART]);

impl Stack {
    /// 零初始化以避免加载。
    pub const ZERO: Self = Self([0; LEN_STACK_PER_HART]);

    /// 从栈上取出硬件线程状态。
    #[inline]
    fn hart_context(&mut self) -> &mut HartContext {
        unsafe { &mut *self.0.as_mut_ptr().cast() }
    }

    pub fn load_as_stack(&'static mut self) {
        let hart = self.hart_context();
        hart.hsm = HsmCell::new();
        let ptr = unsafe { NonNull::new_unchecked(&mut hart.trap) };
        forget(
            FreeTrapStack::new(StackRef(self), ptr, fast_handler)
                .unwrap()
                .load(),
        );
    }
}

#[repr(transparent)]
struct StackRef(&'static mut Stack);

impl AsRef<[u8]> for StackRef {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0 .0[size_of::<HartContext>()..]
    }
}

impl AsMut<[u8]> for StackRef {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0 .0[size_of::<HartContext>()..]
    }
}

impl TrapStackBlock for StackRef {}

impl Drop for StackRef {
    fn drop(&mut self) {
        panic!("Root stack cannot be dropped")
    }
}

/// 硬件线程上下文。
#[repr(C)]
struct HartContext {
    /// 陷入上下文。
    trap: FlowContext,
    hsm: HsmCell<Supervisor>,
}
