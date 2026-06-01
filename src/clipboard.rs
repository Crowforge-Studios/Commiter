use anyhow::Result;

/// Copy `text` to the system clipboard.
///
/// Linux requires the `x11` feature of `arboard` (which needs `libxcb` dev
/// packages at build time). Windows uses `arboard` with its native Win32 API.
/// Other platforms are not supported at compile time.
#[cfg(target_os = "linux")]
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut ctx =
        arboard::Clipboard::new().map_err(|e| anyhow::anyhow!("Clipboard error: {}", e))?;
    ctx.set_text(text)
        .map_err(|e| anyhow::anyhow!("Clipboard error: {}", e))?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut ctx =
        arboard::Clipboard::new().map_err(|e| anyhow::anyhow!("Clipboard error: {}", e))?;
    ctx.set_text(text)
        .map_err(|e| anyhow::anyhow!("Clipboard error: {}", e))?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn copy_to_clipboard(_text: &str) -> Result<()> {
    anyhow::bail!("Clipboard is not supported on this platform")
}
