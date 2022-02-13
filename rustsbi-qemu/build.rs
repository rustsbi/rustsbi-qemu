fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg=-Trustsbi-qemu/src/linker64.ld");
}
