use self_update::cargo_crate_version;

pub fn get_target() -> Result<&'static str, String> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Ok("linux-x86_64")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Ok("linux-aarch64")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Ok("macos-aarch64")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Ok("macos-x86_64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Ok("windows-x86_64")
    } else {
        Err(format!(
            "Unsupported OS/architecture ({}/{}) for self-update",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    }
}

pub fn update() -> Result<bool, Box<dyn std::error::Error>> {
    let target = get_target()?;
    let ext = if cfg!(target_os = "windows") {
        ".zip"
    } else {
        ".tar.gz"
    };

    let status = self_update::backends::github::Update::configure()
        .repo_owner("mderazon")
        .repo_name("mcp-sync")
        .bin_name("mcp-sync")
        .target(target)
        .asset_identifier(ext)
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;

    if status.is_updated() {
        println!("Successfully updated mcp-sync to v{}", status.version());
        Ok(true)
    } else {
        println!("mcp-sync is already up to date (v{})", status.version());
        Ok(false)
    }
}
