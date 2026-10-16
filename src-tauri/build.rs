fn main() {
    println!("cargo::rustc-check-cfg=cfg(foundation_models)");
    build_foundation_models_bridge();
    tauri_build::build()
}

/// Sets `cfg(foundation_models)` only when the Swift bridge is linked.
fn build_foundation_models_bridge() {
    use std::env;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const SOURCE: &str = "swift/foundation_models.swift";
    const REQUIRE_VAR: &str = "SAYTYPE_REQUIRE_FOUNDATION_MODELS";
    println!("cargo:rerun-if-changed={SOURCE}");
    println!("cargo:rerun-if-env-changed={REQUIRE_VAR}");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("aarch64")
    {
        return;
    }

    // Watching xcode-select's link makes cargo scan all of Xcode, so run cargo clean after a switch
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");

    let xcrun = |args: &[&str]| -> Option<String> {
        let output = Command::new("xcrun").args(args).output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let sdk = xcrun(&["--sdk", "macosx", "--show-sdk-path"]);
    let has_framework = sdk.as_ref().is_some_and(|sdk| {
        Path::new(sdk)
            .join("System/Library/Frameworks/FoundationModels.framework")
            .exists()
    });
    let (Some(sdk), true) = (sdk, has_framework) else {
        let message = "macOS SDK has no FoundationModels, post-processing is disabled. Use Xcode 26 or later.";
        if env::var(REQUIRE_VAR).as_deref() == Ok("1") {
            panic!("{message}");
        }
        println!("cargo:warning={message}");
        return;
    };
    let swiftc = xcrun(&["--find", "swiftc"]).expect("swiftc not found");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let object = out_dir.join("foundation_models.o");
    let library = out_dir.join("libfoundation_models.a");
    let deployment_target = env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or("11.0".into());

    let status = Command::new(&swiftc)
        // Without it a single file compiles in script mode and emits a competing _main
        .arg("-parse-as-library")
        .args(["-target", &format!("arm64-apple-macosx{deployment_target}")])
        .args(["-sdk", &sdk, "-O", "-c", SOURCE, "-o"])
        .arg(&object)
        .status()
        .expect("failed to run swiftc");
    assert!(status.success(), "swiftc failed on {SOURCE}");

    let status = Command::new("libtool")
        .args(["-static", "-o"])
        .arg(&library)
        .arg(&object)
        .status()
        .expect("failed to run libtool");
    assert!(status.success(), "libtool failed on {SOURCE}");

    let toolchain_swift = Path::new(&swiftc)
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("lib/swift/macosx");
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=foundation_models");
    println!(
        "cargo:rustc-link-search=native={}",
        toolchain_swift.display()
    );
    println!("cargo:rustc-link-search=native={sdk}/usr/lib/swift");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    // Weak, so the app still launches on macOS versions without the framework
    println!("cargo:rustc-link-arg=-weak_framework");
    println!("cargo:rustc-link-arg=FoundationModels");
    println!("cargo:rustc-cfg=foundation_models");
}
