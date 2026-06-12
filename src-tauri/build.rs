fn main() {
    println!("cargo:rerun-if-env-changed=IPASTE_OCR_R2_BASE_URL");
    println!(
        "cargo:rustc-env=IPASTE_OCR_R2_BASE_URL={}",
        std::env::var("IPASTE_OCR_R2_BASE_URL").unwrap_or_default()
    );
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=framework=Vision");
    }
    tauri_build::build()
}
