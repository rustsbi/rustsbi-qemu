# QEMU support from RustSBI

RustSBI is designed as a library to craft a bootable binary or ELF file. However, QEMU provides us a way to load ELF
file and implement simple SBI directly, thus RustSBI provides a bootable ELF file for this platform.

## Try it out!

To prepare for environment, you should have Rust compiler and QEMU installed.
You may install Rust by [rustup](https://rustup.rs) or using vendor provided rustc and cargo packages.
To install QEMU, your may need to use package manager (e.g. apt, dnf etc.) from system distribution
to get a proper QEMU software package.

After environment prepared, compile and run with:

```shell
cargo qemu
```

When running `cargo qemu`, the test kernel will build and run. Expected output should be:

```plaintext
[rustsbi] RustSBI version 0.4.0-alpha.1, adapting to RISC-V SBI v1.0.0
.______       __    __      _______.___________.  _______..______   __
|   _  \     |  |  |  |    /       |           | /       ||   _  \ |  |
|  |_)  |    |  |  |  |   |   (----`---|  |----`|   (----`|  |_)  ||  |
|      /     |  |  |  |    \   \       |  |      \   \    |   _  < |  |
|  |\  \----.|  `--'  |.----)   |      |  |  .----)   |   |  |_)  ||  |
| _| `._____| \______/ |_______/       |__|  |_______/    |______/ |__|
[rustsbi] Implementation     : RustSBI-QEMU Version 0.2.0-alpha.2
[rustsbi] Platform Name      : riscv-virtio,qemu
[rustsbi] Platform SMP       : 8
[rustsbi] Platform Memory    : 0x80000000..0x88000000
[rustsbi] Boot HART          : 6
[rustsbi] Device Tree Region : 0x87e00000..0x87e01a8e
[rustsbi] Firmware Address   : 0x80000000
[rustsbi] Supervisor Address : 0x80200000
[rustsbi] pmp01: 0x00000000..0x80000000 (-wr)
[rustsbi] pmp02: 0x80000000..0x80200000 (---)
[rustsbi] pmp03: 0x80200000..0x88000000 (xwr)
[rustsbi] pmp04: 0x88000000..0x00000000 (-wr)

 _____         _     _  __                    _
|_   _|__  ___| |_  | |/ /___ _ __ _ __   ___| |
  | |/ _ \/ __| __| | ' // _ \ '__| '_ \ / _ \ |
  | |  __/\__ \ |_  | . \  __/ |  | | | |  __/ |
  |_|\___||___/\__| |_|\_\___|_|  |_| |_|\___|_|
================================================
| boot hart id          |                    6 |
| smp                   |                    8 |
| timebase frequency    |          10000000 Hz |
| dtb physical address  |           0x87e00000 |
------------------------------------------------
[ INFO] Testing `Base`
[ INFO] sbi spec version = 2.0
[ INFO] sbi impl = RustSBI
[ INFO] sbi impl version = 0x400
[ INFO] sbi extensions = [Base, TIME, sPI, HSM, SRST]
[ INFO] mvendor id = 0x0
[ INFO] march id = 0x70200
[ INFO] mimp id = 0x70200
[ INFO] Sbi `Base` test pass
[ INFO] Testing `TIME`
[ INFO] read time register successfuly, set timer +1s
[ INFO] timer interrupt delegate successfuly
[ INFO] Sbi `TIME` test pass
[ INFO] Testing `sPI`
[ INFO] send ipi successfuly
[ INFO] Sbi `sPI` test pass
[ INFO] Testing `HSM`
[ INFO] Testing harts: [0, 1, 2, 3]
[DEBUG] hart 0 started
[DEBUG] hart 0 suspended nonretentive
[DEBUG] hart 1 started
[DEBUG] hart 1 suspended nonretentive
[DEBUG] hart 2 started
[DEBUG] hart 2 suspended nonretentive
[DEBUG] hart 3 started
[DEBUG] hart 3 suspended nonretentive
[DEBUG] hart 0 resumed
[DEBUG] hart 0 suspended retentive
[DEBUG] hart 0 stopped
[DEBUG] hart 1 resumed
[DEBUG] hart 1 suspended retentive
[DEBUG] hart 1 stopped
[DEBUG] hart 2 resumed
[DEBUG] hart 2 suspended retentive
[DEBUG] hart 2 stopped
[DEBUG] hart 3 resumed
[DEBUG] hart 3 suspended retentive
[DEBUG] hart 3 stopped
[ INFO] Testing Pass: [0, 1, 2, 3]
[ INFO] Testing harts: [4, 5, 7]
[DEBUG] hart 4 started
[DEBUG] hart 4 suspended nonretentive
[DEBUG] hart 5 started
[DEBUG] hart 5 suspended nonretentive
[DEBUG] hart 7 started
[DEBUG] hart 7 suspended nonretentive
[DEBUG] hart 4 resumed
[DEBUG] hart 4 suspended retentive
[DEBUG] hart 4 stopped
[DEBUG] hart 5 resumed
[DEBUG] hart 5 suspended retentive
[DEBUG] hart 5 stopped
[DEBUG] hart 7 resumed
[DEBUG] hart 7 suspended retentive
[DEBUG] hart 7 stopped
[ INFO] Testing Pass: [4, 5, 7]
[ INFO] Sbi `HSM` test pass
[ INFO] Testing `DBCN`
Hello, world!
[ INFO] writing slice successfuly
[ INFO] reading 0 bytes from console
[ INFO] Sbi `DBCN` test pass
```

## Run test kernel

### Requirements

You should have `cargo-binutils` installed.

```shell
cargo install cargo-binutils
```

### Run

Run with:

```shell
cargo test
```

It will run RustSBI-QEMU with a test kernel. The test kernel will test all SBI functions,
its command emulation and other features. If it succeeds, there would be output like:

```plaintext
running 1 test
    Finished dev [unoptimized + debuginfo] target(s) in 0.14s
   Compiling test-kernel v0.1.0 (D:\RustProjects\rustsbi-qemu\test-kernel)
    Finished dev [unoptimized + debuginfo] target(s) in 0.61s
test run_test_kernel ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s
```

## Notes

1. What kind of kernel does this project support?

   The rustsbi-qemu project supports raw binary kernels for educational or
   competition use. This project itself is only a showcase example illustrating how
   implementations should use RustSBI, it does not include a Linux boot support.
   You may visit downstream bootloader projects for a Linux capable bootloader.

2. How to enable hypervisor H extension on QEMU?

   You should use these following line of parameters:

   ```rust
       command.args(&["-cpu", "rv64,x-h=true"]);
   ```

   ... to enable H extension on QEMU software.

   The H extension is enabled by default when QEMU version >= 7.0.0.

3. What is the minimum supported Rust version of this package?

   You should build RustSBI-QEMU on nightly at least `rustc 1.66.0-nightly (a24a020e6 2022-10-18)`.

## License

This project is licensed under Mulan PSL v2.

```plaintext
Copyright (c) 2021-2023 RustSBI Team
RustSBI-QEMU is licensed under Mulan PSL v2.
You can use this software according to the terms and conditions of the Mulan PSL v2.
You may obtain a copy of Mulan PSL v2 at:
         http://license.coscl.org.cn/MulanPSL2
THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY KIND,
EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.
See the Mulan PSL v2 for more details.
```
