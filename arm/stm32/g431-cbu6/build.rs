pub fn main() {
    println!("cargo:rerun-if-env-changed=DEFMT_LOG");
    println!("cargo:rustc-link-args-bins=-Tdefmt.x");
    println!("cargo:rustc-link-args-bins=-nmagic");
    println!("cargo:rustc-link-args-bins=-Tlink.x");
}
