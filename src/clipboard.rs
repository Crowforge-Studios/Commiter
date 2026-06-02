use anyhow::Result;
use std::process::Command;

/// Copy `text` to the system clipboard using external tools.
///
/// Tries, in order:
/// 1. `wl-copy` (Wayland)
/// 2. `xclip -selection clipboard` (X11)
/// 3. `xsel -b` (X11, fallback)
///
/// This avoids linking against system GUI libraries and keeps the binary
/// fully static when built with musl.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let result = try_wl_copy(text)
        .or_else(|_| try_xclip(text))
        .or_else(|_| try_xsel(text));

    match result {
        Ok(()) => Ok(()),
        Err(_) => anyhow::bail!(
            "Clipboard tool not found. Install wl-copy (Wayland), xclip, or xsel (X11)."
        ),
    }
}

fn try_wl_copy(text: &str) -> Result<()> {
    let mut child = Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| anyhow::anyhow!("wl-copy not found"))?;

    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())?;

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("wl-copy failed"))
    }
}

fn try_xclip(text: &str) -> Result<()> {
    let mut child = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| anyhow::anyhow!("xclip not found"))?;

    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())?;

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("xclip failed"))
    }
}

fn try_xsel(text: &str) -> Result<()> {
    let mut child = Command::new("xsel")
        .args(["-b"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| anyhow::anyhow!("xsel not found"))?;

    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())?;

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("xsel failed"))
    }
}
