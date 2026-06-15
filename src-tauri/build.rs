fn main() {
    println!("cargo:rerun-if-env-changed=IPASTE_OCR_R2_BASE_URL");
    println!("cargo:rerun-if-env-changed=IPASTE_UPDATER_R2_ENDPOINT");
    let updater_r2_endpoint = std::env::var("IPASTE_UPDATER_R2_ENDPOINT").unwrap_or_default();
    let ocr_r2_base_url = std::env::var("IPASTE_OCR_R2_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| derive_ocr_r2_base_url(&updater_r2_endpoint).unwrap_or_default());
    println!(
        "cargo:rustc-env=IPASTE_OCR_R2_BASE_URL={}",
        ocr_r2_base_url
    );
    println!(
        "cargo:rustc-env=IPASTE_UPDATER_R2_ENDPOINT={}",
        updater_r2_endpoint
    );
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-lib=framework=Vision");
    }
    tauri_build::build()
}

fn derive_ocr_r2_base_url(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if !endpoint.starts_with("https://") {
        return None;
    }

    let endpoint = endpoint
        .split(['?', '#'])
        .next()
        .unwrap_or(endpoint)
        .trim_end_matches('/');
    let parent_index = endpoint.rfind('/')?;
    let parent = &endpoint[..parent_index];
    if parent.len() <= "https://".len() {
        return None;
    }

    Some(format!("{parent}/ocr/"))
}
