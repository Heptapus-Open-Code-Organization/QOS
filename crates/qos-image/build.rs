use std::path::PathBuf;

fn main() {
    // Path to the compiled kernel ELF, provided by the artifact dependency.
    let kernel = std::env::var_os("CARGO_BIN_FILE_OS_os")
        .or_else(|| std::env::var_os("CARGO_BIN_FILE_OS"))
        .expect("missing CARGO_BIN_FILE_OS_os (is [unstable] bindeps enabled?)");
    let kernel = PathBuf::from(kernel);

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    // Verification must never replace the interactive image used by the launcher/releases.
    let image_name = if std::env::var_os("CARGO_FEATURE_VERIFY").is_some() {
        "qos-uefi-verify.img"
    } else {
        "qos-uefi.img"
    };
    let uefi = out_dir.join(image_name);

    bootloader::UefiBoot::new(&kernel)
        .create_disk_image(&uefi)
        .expect("failed to create UEFI disk image");

    // Copy the image to dist/ at build time so `cargo build -p qos-image` alone produces it
    // (CI uploads dist/qos-uefi.img; no separate run step needed).
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let dist = manifest.join("../../dist");
    let _ = std::fs::create_dir_all(&dist);
    std::fs::copy(&uefi, dist.join(image_name)).expect("failed to copy UEFI image to dist/");

    // Expose the image path to the binary via env!().
    println!("cargo:rustc-env=QOS_UEFI_IMAGE={}", uefi.display());
    println!("cargo:rerun-if-changed=build.rs");
}
