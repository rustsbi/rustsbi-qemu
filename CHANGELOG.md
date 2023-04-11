# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Hint spin loop in hart state monitor module
- Add crate *bench-kernel* to workspace for sbi call bench
- Add SBI DBCN extension support

### Modified

- Use instance based RustSBI interface, with separate functions for legacy stdio
- Update sbi-testing to version 0.0.1
- Update sbi-spec to version 0.0.6
- Use crate *fast-trap* version 0.0.1 in rustsbi-qemu for trap handling
- Use crate *rcore-console* version 0.0.0 in rustsbi-qemu and test-kernel for `print!` and `println!`
- Use crate *aclint* version 0.0.0 in rustsbi-qemu for aclint structs
- Use crate *os-xtask-utils* version 0.0.0 in xtask builder
- Use crate *sifive-test-device* version 0.0.0 instead of qemu-exit
- Use crate *uart16550* version 0.0.1 for 16550 definition
- Use `wfi` for suspend and stop without enable mie

### Fixed

- Xtask will now print error when system does not have qemu installed
- Fix dtb parsing for qemu 7.2

## [0.1.1] - 2022-03-23

### Added

- Adapts to RustSBI version 0.2.2, RISC-V SBI version 1.0.0 ratified
- Handle possible failure of deref virtual address by machine trap detection

### Modified

- Use Rust Edition 2021
- Modify test kernel message

## [0.1.0] - 2022-02-13

### Added

- Adapts to RustSBI version 0.2.0
- Implement SBI non-retentive resume procedure
- PMP updates, use stabilized core::arch::asm! macro, thanks to @wyfcyx
- Fixes on usage of CLINT peripheral, thanks to @duskmoon314
- Numerous fixes to HSM module implementation, more documents

[Unreleased]: https://github.com/rustsbi/rustsbi-qemu/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/rustsbi/rustsbi-qemu/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/rustsbi/rustsbi-qemu/releases/tag/v0.1.0
