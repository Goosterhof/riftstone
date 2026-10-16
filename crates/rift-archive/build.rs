//! Link the Windows libraries the UnRAR sources need.
//!
//! The UnRAR C++ code compiled by `unrar_sys` calls registry, process-token and
//! CryptoAPI functions (`RegOpenKeyExW`, `OpenProcessToken`,
//! `CryptAcquireContextW`, `SetFileSecurityW` …) that live in advapi32.
//! `unrar_sys`'s own build script links only powrprof and shell32 — the `unrar`
//! crate used to pull advapi32 in through its winapi features, and rift-archive
//! calls `unrar_sys` directly (see src/unrar_dll.rs). Measured 2026-10-16 on the
//! investor's MSVC toolchain: without this, 13 unresolved externals.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
}
