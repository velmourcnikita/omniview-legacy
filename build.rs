use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-env-changed=OMNIVIEW_BUILD_COMMIT");
    if let Ok(commit) = env::var("OMNIVIEW_BUILD_COMMIT") {
        println!("cargo:rustc-env=OMNIVIEW_BUILD_COMMIT={commit}");
    }
    println!("cargo:rerun-if-changed=assets/omniview.rc");
    let target = env::var("TARGET").unwrap();
    if !target.contains("windows") {
        return;
    }
    let object = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("omniview-resource.o");
    let arch = if target.starts_with("i686") {
        "i686"
    } else {
        "x86_64"
    };
    let tool =
        env::var("OMNIVIEW_WINDRES").unwrap_or_else(|_| format!("{arch}-w64-mingw32-windres"));
    let status = Command::new(&tool)
        .args([
            "--codepage=65001",
            "-i",
            "assets/omniview.rc",
            "-O",
            "coff",
            "-o",
        ])
        .arg(&object)
        .status()
        .expect("Install LLVM-MinGW (windres is required for Windows version metadata)");
    assert!(
        status.success(),
        "Could not compile Windows version metadata"
    );
    println!("cargo:rustc-link-arg={}", object.display());
}
