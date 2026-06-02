mod ai;
mod app;
mod clipboard;
mod git;

use anyhow::Result;
use app::{App, AppEvent, AppState};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

static UNINSTALL_MSG: OnceLock<String> = OnceLock::new();

fn main() -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (tx, rx) = mpsc::channel::<AppEvent>();
    let installed = is_installed();
    let mut app = App::new(installed);

    // Start pre-generation immediately if there are changes
    if let Some(ref info) = app.repo_info {
        if info.has_changes {
            let diff = info.combined_diff.clone();
            let truncated = info.truncated;
            let current_version = git::detect_current_version();
            app.state = app::AppState::PreGenerating;
            app.status_line = "Pre-generating commit message...".to_string();
            app.spinner_start = Instant::now();

            let tx = tx.clone();
            thread::spawn(move || {
                match ai::generate_commit_message(
                    &diff,
                    truncated,
                    current_version.as_deref(),
                ) {
                    Ok(result) => {
                        tx.send(AppEvent::PreGenerated {
                            message: result.message,
                            suggested_version: result.suggested_version,
                        })
                        .ok();
                    }
                    Err(e) => {
                        tx.send(AppEvent::Error(format!("{}", e))).ok();
                    }
                }
            });
        }
    }

    // Start update check in background (only if installed)
    if installed {
        let tx = tx.clone();
        thread::spawn(move || {
            check_for_update(&tx);
        });
    }

    let result = run_app(&mut terminal, &mut app, &rx, &tx);

    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::cursor::Show,
    )?;
    crossterm::terminal::disable_raw_mode()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    if let Some(msg) = UNINSTALL_MSG.get() {
        eprintln!("{}", msg);
    }

    Ok(())
}

fn is_installed() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(home) = std::env::var("HOME") {
            let installed = PathBuf::from(home).join(".local").join("bin").join("commiter");
            return exe == installed;
        }
    }
    false
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    rx: &mpsc::Receiver<AppEvent>,
    tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    loop {
        terminal.draw(|f| app.draw(f))?;

        if let Ok(event) = rx.try_recv() {
            app.handle_event(event);
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Handle settings/uninstall confirm mode
                    if app.state == AppState::ConfirmingUninstall {
                        match key.code {
                            KeyCode::Char('y' | 'Y') => {
                                do_uninstall();
                                break;
                            }
                            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                                app.state = AppState::Settings;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if app.state == AppState::Settings {
                        match key.code {
                            KeyCode::Char('q' | 'Q') | KeyCode::Esc => {
                                app.state = app.prev_state();
                            }
                            KeyCode::Char('u' | 'U') => {
                                if app.latest_version.is_some() {
                                    app.update_status = Some("Downloading update...".to_string());
                                    let tx = tx.clone();
                                    let download_url = app.download_url.clone().unwrap_or_default();
                                    thread::spawn(move || {
                                        perform_update(&tx, &download_url);
                                    });
                                }
                            }
                            KeyCode::Char('x' | 'X') => {
                                app.state = AppState::ConfirmingUninstall;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Normal mode keys
                    match key.code {
                        KeyCode::Char('q' | 'Q') => break,
                        KeyCode::F(1) => {
                            app.show_file_list = !app.show_file_list;
                        }
                        KeyCode::Char('s' | 'S') => {
                            if app.is_installed_version && app.init_error.is_none() {
                                app.state = AppState::Settings;
                            }
                        }
                        KeyCode::Char('r' | 'R') => {
                            if matches!(
                                app.state,
                                app::AppState::PreGenerated | app::AppState::Ready
                            ) {
                                app.pregen_message = None;
                                app.pregen_suggested_version = None;
                                app.commit_message.clear();
                                app.suggested_version = None;
                                app.commit_hash = None;
                                app.start_generating();
                                spawn_generate(app, tx);
                            }
                        }
                        KeyCode::Enter => {
                            if app.can_copy_pregen() {
                                let msg = app.pregen_message.take().unwrap();
                                let version = app.pregen_suggested_version.take();
                                match clipboard::copy_to_clipboard(&msg) {
                                    Ok(()) => {
                                        app.commit_message = msg;
                                        app.suggested_version = version;
                                        app.state = app::AppState::Ready;
                                        app.status_line =
                                            "✓ Copied to clipboard".to_string();
                                    }
                                    Err(e) => {
                                        tx.send(AppEvent::Error(format!(
                                            "Clipboard error: {}",
                                            e
                                        )))
                                        .ok();
                                    }
                                }
                            } else if app.can_generate() {
                                app.start_generating();
                                spawn_generate(app, tx);
                            } else if app.can_commit() {
                                let msg = app.commit_message.clone();
                                let repo_path = app
                                    .repo_info
                                    .as_ref()
                                    .map(|r| r.repo_path.clone())
                                    .unwrap_or_default();
                                app.start_committing();

                                let tx = tx.clone();
                                thread::spawn(move || {
                                    match git::stage_all_and_commit(&repo_path, &msg) {
                                        Ok(hash) => {
                                            tx.send(AppEvent::Committed(hash)).ok();
                                        }
                                        Err(e) => {
                                            tx.send(AppEvent::Error(format!(
                                                "Commit error: {}",
                                                e
                                            )))
                                            .ok();
                                        }
                                    }
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

fn spawn_generate(app: &App, tx: &mpsc::Sender<AppEvent>) {
    let diff = app
        .repo_info
        .as_ref()
        .map(|r| r.combined_diff.clone())
        .unwrap_or_default();
    let truncated = app
        .repo_info
        .as_ref()
        .map(|r| r.truncated)
        .unwrap_or(false);
    let current_version = git::detect_current_version();

    let tx = tx.clone();
    thread::spawn(move || {
        match ai::generate_commit_message(&diff, truncated, current_version.as_deref()) {
            Ok(result) => match clipboard::copy_to_clipboard(&result.message) {
                Ok(()) => {
                    if current_version.is_some() {
                        tx.send(AppEvent::GeneratedWithVersion {
                            message: result.message,
                            suggested_version: result.suggested_version,
                        })
                        .ok();
                    } else {
                        tx.send(AppEvent::Generated(result.message)).ok();
                    }
                }
                Err(e) => {
                    tx.send(AppEvent::Error(format!("Clipboard error: {}", e))).ok();
                }
            },
            Err(e) => {
                tx.send(AppEvent::Error(format!("{}", e))).ok();
            }
        }
    });
}

fn check_for_update(tx: &mpsc::Sender<AppEvent>) {
    let output = std::process::Command::new("curl")
        .args([
            "-sIL",
            "-o", "/dev/null",
            "-w", "%{url_effective}",
            "https://github.com/trk/commiter/releases/latest",
        ])
        .output()
        .ok();

    let url = output
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .and_then(|s| {
            if s.is_empty() { None } else { Some(s) }
        });

    match url {
        Some(url) => {
            if let Some(tag) = url.rsplit('/').next() {
                let latest = tag.trim_start_matches('v').to_string();
                let current = env!("CARGO_PKG_VERSION");
                if latest.as_str() != current {
                    let download_url = format!(
                        "https://github.com/trk/commiter/releases/latest/download/commiter"
                    );
                    tx.send(AppEvent::UpdateCheck {
                        latest,
                        download_url,
                    })
                    .ok();
                } else {
                    tx.send(AppEvent::UpToDate).ok();
                }
            }
        }
        None => {
            tx.send(AppEvent::UpdateProgress(
                "Could not check for updates (no network?)".to_string(),
            ))
            .ok();
        }
    }
}

fn perform_update(tx: &mpsc::Sender<AppEvent>, download_url: &str) {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            tx.send(AppEvent::UpdateProgress("Failed to locate binary".to_string()))
                .ok();
            return;
        }
    };

    let new_path = exe.with_extension("new");
    let old_path = exe.with_extension("old");

    // Download new binary
    tx.send(AppEvent::UpdateProgress("Downloading...".to_string())).ok();
    let status = std::process::Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&new_path)
        .arg(download_url)
        .status()
        .ok();

    match status {
        Some(s) if s.success() => {}
        _ => {
            tx.send(AppEvent::UpdateProgress("Download failed".to_string())).ok();
            return;
        }
    }

    use std::fs;
    // Make it executable
    let _ = fs::set_permissions(&new_path, std::os::unix::fs::PermissionsExt::from_mode(0o755));

    // Swap binaries: old → old, new → current
    let _ = fs::rename(&exe, &old_path);
    if fs::rename(&new_path, &exe).is_ok() {
        tx.send(AppEvent::UpdateDone).ok();
        // Spawn new version
        let _ = std::process::Command::new(&exe).spawn();
    } else {
        // Restore if rename failed
        let _ = fs::rename(&old_path, &exe);
        tx.send(AppEvent::UpdateProgress("Update failed".to_string())).ok();
    }
}

fn do_uninstall() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(&exe);
    }
    let _ = UNINSTALL_MSG.set(
        "commiter has been uninstalled.\n\
         You may also want to remove ~/.local/bin from your PATH if no longer needed."
            .to_string(),
    );
}
