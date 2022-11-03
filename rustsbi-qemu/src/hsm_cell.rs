use core::{
    cell::UnsafeCell,
    hint::spin_loop,
    sync::atomic::{AtomicUsize, Ordering},
};
use rustsbi::spec::hsm as spec;

pub struct HsmCell<T> {
    state: AtomicUsize,
    value: UnsafeCell<Option<T>>,
}

unsafe impl<T: Send> Sync for HsmCell<T> {}
unsafe impl<T: Send> Send for HsmCell<T> {}

const HART_STATE_START_PENDING_EXT: usize = usize::MAX;

#[allow(unused)]
impl<T> HsmCell<T> {
    #[inline]
    pub fn put(&self, t: T) -> bool {
        if self
            .state
            .compare_exchange(
                spec::HART_STATE_STOPPED,
                HART_STATE_START_PENDING_EXT,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            unsafe { *self.value.get() = Some(t) };
            self.state
                .store(spec::HART_STATE_START_PENDING, Ordering::Release);
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn take(&self) -> Result<T, usize> {
        loop {
            match self.state.compare_exchange(
                spec::HART_STATE_START_PENDING,
                spec::HART_START,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => break Ok(unsafe { (*self.value.get()).take().unwrap() }),
                Err(HART_STATE_START_PENDING_EXT) => spin_loop(),
                Err(s) => break Err(s),
            }
        }
    }

    #[inline]
    pub fn sbi_get_status(&self) -> usize {
        match self.state.load(Ordering::Acquire) {
            HART_STATE_START_PENDING_EXT => spec::HART_STATE_START_PENDING,
            normal => normal,
        }
    }
}
