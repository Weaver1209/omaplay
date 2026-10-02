fn main() {
    println!("cargo:rustc-link-lib=mpv");
    println!("cargo:rustc-link-lib=EGL");
    println!("cargo:rustc-link-lib=GL");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
}
