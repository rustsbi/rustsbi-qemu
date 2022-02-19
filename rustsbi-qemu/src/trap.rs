// use trap based interrupt handler instead of generator based by now

use core::arch::asm;

#[naked]
#[link_section = ".text"]
unsafe extern "C" fn trap_vector() {
    asm!(
    ".option push
	.option norvc",
    "j	{trap_exception}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{machine_software}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{machine_timer}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{machine_external}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    "j	{interrupt_reserved}",
    ".option pop",
    trap_exception = sym trap_exception,
    machine_software = sym machine_software,
    machine_timer = sym machine_timer,
    machine_external = sym machine_external,
    interrupt_reserved = sym interrupt_reserved,
    options(noreturn))
}

#[naked]
#[link_section = ".text"]
pub extern "C" fn trap_exception() -> ! {
    todo!("any exception")
}

#[naked]
#[link_section = ".text"]
pub extern "C" fn machine_software() -> ! {
    todo!("machine software")
}

#[naked]
#[link_section = ".text"]
pub extern "C" fn machine_timer() -> ! {
    todo!("machine timer")
}

#[naked]
#[link_section = ".text"]
pub extern "C" fn machine_external() -> ! {
    todo!("machine external")
}

#[naked]
#[link_section = ".text"]
pub extern "C" fn interrupt_reserved() -> ! {
    panic!("entered handler for reserved interrupt")
}
