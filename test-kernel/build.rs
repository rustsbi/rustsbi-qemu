fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(target_pointer_width = "64")]
    println!("cargo:rustc-link-arg=-Ttest-kernel/src/linker64.ld");
    #[cfg(target_pointer_width = "32")]
    println!("cargo:rustc-link-arg=-Ttest-kernel/src/linker32.ld");
}
