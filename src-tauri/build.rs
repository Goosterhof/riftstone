//! Tauri's build step, plus the Windows manifest for the test binaries.
//!
//! tauri-build links its application manifest (a dependency on Common Controls
//! v6) into the app binary only. A test binary that builds a Tauri app, even
//! on the mock runtime (`tests/spine_ipc.rs`), imports `TaskDialogIndirect`
//! from comctl32 v6. Without the manifest Windows will not load it:
//! `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139), measured 2026-10-16 on the
//! investor's MSVC toolchain. `windows-test-manifest.xml` is tauri-build's
//! default manifest, embedded into test targets by the MSVC linker.

fn main() {
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows_msvc {
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-test-manifest.xml");
        println!("cargo:rerun-if-changed=windows-test-manifest.xml");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
    tauri_build::build()
}
