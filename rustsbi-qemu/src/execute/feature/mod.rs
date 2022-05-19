mod emulate_rdtime;
mod transfer_trap;

pub use emulate_rdtime::emulate_rdtime;
pub use transfer_trap::{do_transfer_trap, should_transfer_trap};
