use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::git::RepoInfo;

#[derive(Clone, Copy, PartialEq)]
pub enum AppState {
    Idle,
    Loading,
    Ready,
    Committing,
}

/// Events produced by background threads and consumed by the main loop.
pub enum AppEvent {
    Generated(String),
    Committed(String),
    Error(String),
}

pub struct App {
    pub state: AppState,
    pub repo_info: Option<RepoInfo>,
    pub init_error: Option<String>,
    pub commit_message: String,
    pub status_line: String,
    pub commit_hash: Option<String>,
}

impl App {
    pub fn new() -> Self {
        match crate::git::get_repo_info() {
            Ok(info) => {
                let status = if !info.has_changes {
                    "No changes detected. Make some changes to generate a message.".to_string()
                } else {
                    String::new()
                };
                Self {
                    state: AppState::Idle,
                    repo_info: Some(info),
                    init_error: None,
                    commit_message: String::new(),
                    status_line: status,
                    commit_hash: None,
                }
            }
            Err(e) => Self {
                state: AppState::Idle,
                repo_info: None,
                init_error: Some(format!("{} — press 'q' to quit", e)),
                commit_message: String::new(),
                status_line: String::new(),
                commit_hash: None,
            },
        }
    }

    pub fn can_generate(&self) -> bool {
        self.state == AppState::Idle
            && self.init_error.is_none()
            && self.repo_info.as_ref().map_or(false, |r| r.has_changes)
    }

    pub fn can_commit(&self) -> bool {
        self.state == AppState::Ready
            && !self.commit_message.is_empty()
            && self.commit_hash.is_none()
    }

    pub fn start_generating(&mut self) {
        self.state = AppState::Loading;
        self.status_line = "Generating commit message...".to_string();
    }

    pub fn start_committing(&mut self) {
        self.state = AppState::Committing;
        self.status_line = "Creating commit...".to_string();
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Generated(msg) => {
                self.commit_message = msg;
                self.state = AppState::Ready;
                self.status_line = "✓ Copied to clipboard".to_string();
            }
            AppEvent::Committed(hash) => {
                let short = if hash.len() > 7 {
                    hash[..7].to_string()
                } else {
                    hash.clone()
                };
                self.commit_hash = Some(hash);
                self.state = AppState::Ready;
                self.status_line = format!("✓ Committed: {}", short);
            }
            AppEvent::Error(err) => {
                self.status_line = format!("✗ {}", err);
                // Return to Idle so the user can retry.
                self.state = AppState::Idle;
            }
        }
    }

    pub fn draw(&self, f: &mut Frame) {
        let size = f.area();

        // Full-screen init error.
        if let Some(ref err) = self.init_error {
            let block = Block::default()
                .title(" Commiter — Error ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red));
            let p = Paragraph::new(Line::from(Span::styled(
                err,
                Style::default().fg(Color::Red).bold(),
            )))
            .block(block)
            .alignment(Alignment::Center);
            f.render_widget(p, size);
            return;
        }

        let show_commit = matches!(self.state, AppState::Ready | AppState::Committing);

        let chunks = if show_commit {
            Layout::vertical([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(size)
        } else {
            Layout::vertical([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(1),
            ])
            .split(size)
        };

        self.render_title(f, chunks[0]);
        self.render_info(f, chunks[1]);
        self.render_generate_button(f, chunks[2]);
        self.render_message(f, chunks[3]);

        if show_commit {
            self.render_commit_button(f, chunks[4]);
            self.render_status(f, chunks[5]);
        } else {
            self.render_status(f, chunks[4]);
        }
    }

    // ---- rendering helpers --------------------------------------------------

    fn render_title(&self, f: &mut Frame, area: Rect) {
        let line = Line::from(vec![
            "Commiter".bold().fg(Color::Cyan),
            Span::raw(" — "),
            "Generate commit message from git diffs".dim(),
        ]);
        f.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
    }

    fn render_info(&self, f: &mut Frame, area: Rect) {
        let lines = match self.repo_info {
            Some(ref info) => vec![
                Line::from(Span::styled(
                    format!("Path: {}", info.repo_path),
                    Style::default().dim(),
                )),
                Line::from(vec![
                    Span::styled(
                        format!(
                            "Staged: {} {}",
                            info.staged_count,
                            plural("file", info.staged_count),
                        ),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!(
                            "Unstaged: {} {}",
                            info.unstaged_count,
                            plural("file", info.unstaged_count),
                        ),
                        Style::default().fg(Color::Yellow),
                    ),
                    if info.truncated {
                        Span::styled("  [diff truncated]", Style::default().fg(Color::Red))
                    } else {
                        Span::raw("")
                    },
                ]),
            ],
            None => vec![Line::from("")],
        };
        f.render_widget(Paragraph::new(lines), area);
    }

    fn render_generate_button(&self, f: &mut Frame, area: Rect) {
        let active = self.can_generate();
        let label = match self.state {
            AppState::Loading => "  Generating…  ",
            _ => "  Create commit message  ",
        };
        render_button(f, area, label, active);
    }

    fn render_commit_button(&self, f: &mut Frame, area: Rect) {
        let label = match self.state {
            AppState::Committing => "  Committing…  ",
            _ => "  Commit changes  ",
        };
        let active = self.can_commit();
        render_button(f, area, label, active);
    }

    fn render_message(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Commit Message ")
            .border_style(Style::default());

        if self.commit_message.is_empty() {
            f.render_widget(block, area);
        } else {
            let inner = block.inner(area);
            f.render_widget(block, area);
            let p = Paragraph::new(self.commit_message.as_str())
                .wrap(Wrap { trim: false });
            f.render_widget(p, inner);
        }
    }

    fn render_status(&self, f: &mut Frame, area: Rect) {
        let text = &self.status_line;
        if text.is_empty() {
            return;
        }

        let style = if text.starts_with('✓') {
            Style::default().fg(Color::Green)
        } else if text.starts_with('✗') {
            Style::default().fg(Color::Red)
        } else if text.contains("error") || text.contains("Error") {
            Style::default().fg(Color::Red)
        } else if text.contains("truncated") {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(text, style))),
            area,
        );
    }
}

fn render_button(f: &mut Frame, area: Rect, label: &str, active: bool) {
    let style = if active {
        Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let border_style = if active {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(label, style))).alignment(Alignment::Center),
        inner,
    );
}

fn plural(s: &str, n: usize) -> String {
    if n == 1 { s.to_string() } else { format!("{}s", s) }
}
