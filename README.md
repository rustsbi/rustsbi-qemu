# QEMU support from RustSBI

RustSBI is designed as a library to craft a bootable binary or ELF file. However, QEMU provides us a way to load ELF
file and implement simple SBI directly, thus RustSBI provides a bootable ELF file for this platform.

## Try it out!

Compile and run with:

```shell
cargo qemu
```

When running `cargo qemu`, the test kernel will build and run. Expected output should be:

```
[rustsbi] RustSBI version 0.2.0, adapting to RISC-V SBI v0.3
.______       __    __      _______.___________.  _______..______   __
|   _  \     |  |  |  |    /       |           | /       ||   _  \ |  |
|  |_)  |    |  |  |  |   |   (----`---|  |----`|   (----`|  |_)  ||  |
|      /     |  |  |  |    \   \       |  |      \   \    |   _  < |  |
|  |\  \----.|  `--'  |.----)   |      |  |  .----)   |   |  |_)  ||  |
| _| `._____| \______/ |_______/       |__|  |_______/    |______/ |__|

[rustsbi] Implementation: RustSBI-QEMU Version 0.1.0
[rustsbi-dtb] Hart count: cluster0 with 8 cores
[rustsbi] misa: RV64ACDFIMSU
[rustsbi] mideleg: ssoft, stimer, sext (0x222)
[rustsbi] medeleg: ima, ia, bkpt, la, sa, uecall, ipage, lpage, spage (0xb1ab)
[rustsbi] pmp0: 0x10000000 ..= 0x10001fff (rw-)
[rustsbi] pmp1: 0x2000000 ..= 0x200ffff (rw-)
[rustsbi] pmp2: 0xc000000 ..= 0xc3fffff (rw-)
[rustsbi] pmp3: 0x80000000 ..= 0x8fffffff (rwx)
[rustsbi] enter supervisor 0x80200000
<< Test-kernel: Hart id = 0, DTB physical address = 0x87000000
>> Test-kernel: Testing base extension
<< Test-kernel: Base extension version: 1
<< Test-kernel: SBI specification version: 3
<< Test-kernel: SBI implementation Id: 4
<< Test-kernel: SBI implementation version: 200
<< Test-kernel: Device mvendorid: 0
<< Test-kernel: Device marchid: 0
<< Test-kernel: Device mimpid: 0
>> Test-kernel: Testing SBI instruction emulation
<< Test-kernel: Current time: 17fc45
<< Test-kernel: Time after operation: 187678
>> Test-kernel: Trigger illegal exception
<< Test-kernel: Value of scause: Exception(IllegalInstruction)
<< Test-kernel: Illegal exception delegate success
>> Stop hart 3, return value 0
>> Hart 0 state return value: 0
>> Hart 1 state return value: 4
>> Hart 2 state return value: 4
>> Hart 3 state return value: 1
<< Test-kernel: test for hart 0 success, wake another hart
>> Wake hart 1, sbi return value 0
>> Start test for hart 1, retentive suspend return value 0
>> Wake hart 2, sbi return value 0
<< The parameter passed to hart 2 resume is: 0x4567890a
>> Start hart 3 with parameter 0x12345678
>> SBI return value: 0
<< The parameter passed to hart 3 start is: 0x12345678
<< Test-kernel: All hart SBI test SUCCESS, shutdown
```

## Run test kernel

### Requirements

You should have `cargo-binutils` and `llvm-tools-preview` installed.

```
cargo install cargo-binutils
rustup component add llvm-tools-preview
```

### Run

Run with:

```shell
cargo test
```

It will run RustSBI-QEMU with a test kernel. The test kernel will test all SBI functions, 
its command emulation and other features. If it succeeds, there would be output like:

```
running 1 test
    Finished dev [unoptimized + debuginfo] target(s) in 0.14s
   Compiling test-kernel v0.1.0 (D:\RustProjects\rustsbi-qemu\test-kernel)
    Finished dev [unoptimized + debuginfo] target(s) in 0.61s
test run_test_kernel ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s
```

## Notes

1. How to enable hypervisor H extension on QEMU?

You should use these following line of parameters:

```rust
    command.args(&["-cpu", "rv64,x-h=true"]);
```

... to enable H extension on QEMU software.

## License 

This project is licensed under Mulan PSL v2.

```text
Copyright (c) 2021-2022 RustSBI Team
RustSBI-QEMU is licensed under Mulan PSL v2.
You can use this software according to the terms and conditions of the Mulan PSL v2.
You may obtain a copy of Mulan PSL v2 at:
         http://license.coscl.org.cn/MulanPSL2
THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY KIND,
EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.
See the Mulan PSL v2 for more details.
```
