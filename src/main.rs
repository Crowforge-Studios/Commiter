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
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn parse_args() {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--model" if i + 1 < args.len() => {
                i += 1;
                std::env::set_var("OPENCODE_MODEL", &args[i]);
            }
            "--diff-cutoff" if i + 1 < args.len() => {
                i += 1;
                std::env::set_var("COMMITER_DIFF_CUTOFF", &args[i]);
            }
            "--version" | "-V" => {
                println!("Commiter v{}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "--help" | "-h" => {
                println!("Commiter — AI-powered commit message generator");
                println!();
                println!("USAGE:");
                println!("  commiter [OPTIONS]");
                println!();
                println!("OPTIONS:");
                println!("  --model <model>        AI model (default: opencode/big-pickle)");
                println!("  --diff-cutoff <bytes>  Max diff bytes (default: 8192)");
                println!("  --version, -V         Print version and exit");
                println!("  --help, -h             Print this help");
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }
}

fn main() -> Result<()> {
    parse_args();
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (tx, rx) = mpsc::channel::<AppEvent>();
    let mut app = App::new();

    start_pregen(&mut app, &tx);

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

fn start_pregen(app: &mut App, tx: &mpsc::Sender<AppEvent>) {
    let Some(ref info) = app.repo_info else { return };
    if !info.has_changes {
        return;
    }

    app.state = AppState::PreGenerating;
    app.status_line = "Pre-generating commit message...".to_string();
    app.spinner_start = Instant::now();

    spawn_ai_task(
        info.combined_diff.clone(),
        info.truncated,
        app.current_version.clone(),
        tx,
        false,
    );
}

fn spawn_ai_task(
    diff: String,
    truncated: bool,
    current_version: Option<String>,
    tx: &mpsc::Sender<AppEvent>,
    copy_to_clip: bool,
) {
    let tx = tx.clone();
    thread::spawn(move || {
        let result =
            ai::generate_commit_message(&diff, truncated, current_version.as_deref());
        match result {
            Ok(result) => {
                if copy_to_clip {
                    match clipboard::copy_to_clipboard(&result.message) {
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
                            tx.send(AppEvent::Error(format!("Clipboard error: {}", e)))
                                .ok();
                        }
                    }
                } else {
                    tx.send(AppEvent::PreGenerated {
                        message: result.message,
                        suggested_version: result.suggested_version,
                    })
                    .ok();
                }
            }
            Err(e) => {
                tx.send(AppEvent::Error(format!("{}", e))).ok();
            }
        }
    });
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
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if app.editing {
                    match key.code {
                        KeyCode::Esc => {
                            app.stop_editing();
                        }
                        KeyCode::Enter => {
                            app.commit_message.insert(app.cursor_pos, '\n');
                            app.cursor_pos += 1;
                        }
                        KeyCode::Backspace => {
                            app.delete_backward();
                        }
                        KeyCode::Delete => {
                            app.delete_forward();
                        }
                        KeyCode::Left => {
                            app.cursor_left();
                        }
                        KeyCode::Right => {
                            app.cursor_right();
                        }
                        KeyCode::Home => {
                            app.cursor_home();
                        }
                        KeyCode::End => {
                            app.cursor_end();
                        }
                        KeyCode::Char(c) => {
                            app.insert_char(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q' | 'Q') => break,
                    KeyCode::F(1) => {
                        app.show_file_list = !app.show_file_list;
                    }
                    KeyCode::Char('e' | 'E')
                        if app.state == AppState::Ready && !app.commit_message.is_empty() =>
                    {
                        app.start_editing();
                    }
                    KeyCode::Char('r' | 'R') => {
                        let allowed = matches!(
                            app.state,
                            AppState::PreGenerated | AppState::Ready
                        );
                        if allowed {
                            app.pregen_message = None;
                            app.pregen_suggested_version = None;
                            app.commit_message.clear();
                            app.suggested_version = None;
                            app.commit_hash = None;
                            app.start_generating();
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
                            spawn_ai_task(diff, truncated, app.current_version.clone(), tx, true);
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
                                    app.state = AppState::Ready;
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
                            spawn_ai_task(diff, truncated, app.current_version.clone(), tx, true);
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
    Ok(())
}
