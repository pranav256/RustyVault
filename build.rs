#[cfg(windows)]
fn main() -> std::io::Result<()> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set("ProductName", "RustyVault")
            .set("ProductVersion", env!("CARGO_PKG_VERSION"))
            .compile()?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() {}
