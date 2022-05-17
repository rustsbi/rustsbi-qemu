pub(crate) fn base_extension() {
    println!(
        "
[test-kernel] Testing base extension"
    );
    let base_version = sbi::probe_extension(sbi::EID_BASE);
    if base_version == 0 {
        panic!(
            "\
[test-kernel] no base extension probed; SBI call returned value '0'
[test-kernel] this SBI implementation may only have legacy extension implemented"
        );
    }

    let spec_version = sbi::get_spec_version();
    println!(
        "\
[test-kernel] Base extension version: {base_version:x}
[test-kernel] SBI specification version: {major}.{minor}
[test-kernel] SBI implementation Id: {impl_id:x}
[test-kernel] SBI implementation version: {impl_version:x}
[test-kernel] Device mvendorid: {mvendorid:x}
[test-kernel] Device marchid: {marchid:x}
[test-kernel] Device mimpid: {mimpid:x}",
        major = (spec_version >> 24) & 0x7F,
        minor = spec_version & 0xFFFFFF,
        impl_id = sbi::get_sbi_impl_id(),
        impl_version = sbi::get_sbi_impl_version(),
        mvendorid = sbi::get_mvendorid(),
        marchid = sbi::get_marchid(),
        mimpid = sbi::get_mimpid(),
    );
}

pub(crate) fn sbi_ins_emulation() {
    use riscv::register::time;

    let time_start = time::read64();
    println!(
        "
[test-kernel] Testing SBI instruction emulation
[test-kernel] Current time: {time_start}"
    );
    let time_end = time::read64();
    if time_end > time_start {
        println!(
            "\
[test-kernel] Time after operation: {time_end}"
        );
    } else {
        panic!(
            "\
[test-kernel] SBI test FAILED due to incorrect time counter"
        );
    }
}
