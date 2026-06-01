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
use std::time::Duration;

fn main() -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let (tx, rx) = mpsc::channel::<AppEvent>();
    let mut app = App::new();

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
                        KeyCode::Enter => {
                            if app.can_generate() {
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
                                app.start_generating();

                                let tx = tx.clone();
                                thread::spawn(move || {
                                    match ai::generate_commit_message(&diff, truncated) {
                                        Ok(msg) => {
                                            match clipboard::copy_to_clipboard(&msg) {
                                                Ok(()) => {
                                                    tx.send(AppEvent::Generated(msg)).ok();
                                                }
                                                Err(e) => {
                                                    tx.send(AppEvent::Error(format!(
                                                        "Clipboard error: {}",
                                                        e
                                                    )))
                                                    .ok();
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tx.send(AppEvent::Error(format!("{}", e))).ok();
                                        }
                                    }
                                });
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
