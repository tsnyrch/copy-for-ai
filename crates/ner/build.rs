fn main() {
    if std::env::var_os("CARGO_FEATURE_COREML").is_none()
        || std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
    {
        return;
    }
    let output = std::process::Command::new("xcrun")
        .args(["clang", "--print-runtime-dir"])
        .output()
        .expect("CoreML requires the macOS compiler runtime");
    assert!(
        output.status.success(),
        "macOS compiler runtime unavailable"
    );
    let directory = std::str::from_utf8(&output.stdout)
        .expect("compiler runtime path must be UTF-8")
        .trim();
    // ORT's CoreML objects use Apple's platform-version check from the Clang runtime.
    println!("cargo:rustc-link-search=native={directory}");
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
