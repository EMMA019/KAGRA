fn main() {
    let rev = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=KAGRA_GIT_REV={rev}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");

    // cdylib-only, and only for Android. Host/Python builds must not inherit
    // this SONAME. cargo:rustc-cdylib-link-arg is passed even when RUSTFLAGS
    // replaces .cargo/config.toml rustflags (cargo-ndk does not set those).
    // Without -soname, CMake's absolute IMPORTED_LOCATION becomes DT_NEEDED
    // and dlopen looks for the CI path on device.
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("android") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-z,max-page-size=16384");
        println!("cargo:rustc-cdylib-link-arg=-Wl,-z,common-page-size=16384");
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libkagra_shared.so");
    }
}
