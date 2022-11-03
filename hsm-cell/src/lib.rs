//! 硬件线程状态和受状态保护的线程间共享数据。

#![no_std]
#![deny(warnings, missing_docs)]

use core::{
    cell::UnsafeCell,
    hint::spin_loop,
    sync::atomic::{AtomicUsize, Ordering},
};
use sbi_spec::hsm::*;

/// 硬件线程状态和受状态保护的线程间共享数据。
pub struct HsmCell<T> {
    status: AtomicUsize,
    val: UnsafeCell<Option<T>>,
}

/// 当前硬件线程的共享对象。
pub struct LocalHsmCell<'a, T>(&'a HsmCell<T>);

/// 任意硬件线程的共享对象。
pub struct RemoteHsmCell<'a, T>(&'a HsmCell<T>);

unsafe impl<T: Send> Sync for HsmCell<T> {}
unsafe impl<T: Send> Send for HsmCell<T> {}

const HART_STATE_START_PENDING_EXT: usize = usize::MAX;

impl<T> HsmCell<T> {
    /// 从当前硬件线程的状态中获取线程间共享对象。
    ///
    /// # Safety
    ///
    /// 用户需要确保对象属于当前硬件线程。
    #[inline]
    pub unsafe fn local(&self) -> LocalHsmCell<'_, T> {
        LocalHsmCell(self)
    }

    /// 取出共享对象。
    #[inline]
    pub fn remote(&self) -> RemoteHsmCell<'_, T> {
        RemoteHsmCell(self)
    }
}

impl<T> LocalHsmCell<'_, T> {
    /// 从启动挂起状态的硬件线程取出共享数据，并将其状态设置为启动，如果成功返回取出的数据，否则返回当前状态。
    #[inline]
    pub fn start(&self) -> Result<T, usize> {
        loop {
            match self.0.status.compare_exchange(
                HART_STATE_START_PENDING,
                HART_STATE_STARTED,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => break Ok(unsafe { (*self.0.val.get()).take().unwrap() }),
                Err(HART_STATE_START_PENDING_EXT) => spin_loop(),
                Err(s) => break Err(s),
            }
        }
    }
}

impl<T> RemoteHsmCell<'_, T> {
    /// 向关闭状态的硬件线程传入共享数据，并将其状态设置为启动挂起，返回是否放入成功。
    #[inline]
    pub fn start(self, t: T) -> bool {
        if self
            .0
            .status
            .compare_exchange(
                HART_STATE_STOPPED,
                HART_STATE_START_PENDING_EXT,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            unsafe { *self.0.val.get() = Some(t) };
            self.0
                .status
                .store(HART_STATE_START_PENDING, Ordering::Release);
            true
        } else {
            false
        }
    }

    /// 取出当前状态。
    #[inline]
    pub fn sbi_get_status(&self) -> usize {
        match self.0.status.load(Ordering::Acquire) {
            HART_STATE_START_PENDING_EXT => HART_STATE_START_PENDING,
            normal => normal,
        }
    }
}
