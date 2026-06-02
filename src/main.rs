mod ai;
mod app;
mod clipboard;
mod git;

use anyhow::Result;
use app::{App, AppEvent};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (tx, rx) = mpsc::channel::<AppEvent>();
    let mut app = App::new();

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

    Ok(())
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
                    match key.code {
                        KeyCode::Char('q' | 'Q') => break,
                        KeyCode::F(1) => {
                            app.show_file_list = !app.show_file_list;
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
