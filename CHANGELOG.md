# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres
to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Hint spin loop in hart state monitor module

### Modified

- Update sbi-testing to version 0.0.1
- Use crate os-xtask-utils version 0.0.0 in xtask builder

### Fixed

- Xtask will now print error when system does not have qemu installed

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
