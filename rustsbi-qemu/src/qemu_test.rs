use rustsbi::{
    spec::srst::{
        RESET_REASON_NO_REASON, RESET_REASON_SYSTEM_FAILURE, RESET_TYPE_COLD_REBOOT,
        RESET_TYPE_SHUTDOWN, RESET_TYPE_WARM_REBOOT,
    },
    Reset, SbiRet,
};
use sifive_test_device::SifiveTestDevice;
use spin::Once;

pub(crate) struct QemuTest(usize);

static TEST: Once<QemuTest> = Once::new();

pub(crate) fn init(base: usize) {
    TEST.call_once(|| QemuTest(base));
}

pub(crate) fn get() -> &'static QemuTest {
    TEST.wait()
}

impl Reset for QemuTest {
    fn system_reset(&self, reset_type: u32, reset_reason: u32) -> SbiRet {
        let test = unsafe { &*(TEST.wait().0 as *const SifiveTestDevice) };
        match reset_type {
            RESET_TYPE_SHUTDOWN => match reset_reason {
                RESET_REASON_NO_REASON => test.pass(),
                RESET_REASON_SYSTEM_FAILURE => test.fail(-1 as _),
                value => test.fail(value as _),
            },
            RESET_TYPE_COLD_REBOOT | RESET_TYPE_WARM_REBOOT => {
                // test.reset();
                todo!()
            }
            _ => SbiRet::invalid_param(),
        }
    }
}
