use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::time::Instant;

use crate::git::RepoInfo;

#[derive(Clone, Copy, PartialEq)]
pub enum AppState {
    Idle,
    PreGenerating,
    PreGenerated,
    Loading,
    Ready,
    Committing,
}

/// Events produced by background threads and consumed by the main loop.
pub enum AppEvent {
    Generated(String),
    GeneratedWithVersion {
        message: String,
        suggested_version: Option<String>,
    },
    PreGenerated {
        message: String,
        suggested_version: Option<String>,
    },
    Committed(String),
    Error(String),
}

pub struct App {
    pub state: AppState,
    pub repo_info: Option<RepoInfo>,
    pub init_error: Option<String>,
    pub commit_message: String,
    pub suggested_version: Option<String>,
    pub pregen_message: Option<String>,
    pub pregen_suggested_version: Option<String>,
    pub status_line: String,
    pub commit_hash: Option<String>,
    pub spinner_start: Instant,
    pub show_file_list: bool,
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
                let current_version = crate::git::detect_current_version();
                let status = if let Some(ref ver) = current_version {
                    if status.is_empty() {
                        format!("Current version: {} | Press Enter to generate a message", ver)
                    } else {
                        format!("{} | Current version: {}", status, ver)
                    }
                } else {
                    if status.is_empty() {
                        "Press Enter to generate a commit message".to_string()
                    } else {
                        status
                    }
                };
                Self {
                    state: AppState::Idle,
                    repo_info: Some(info),
                    init_error: None,
                    commit_message: String::new(),
                    suggested_version: None,
                    pregen_message: None,
                    pregen_suggested_version: None,
                    status_line: status,
                    commit_hash: None,
                    spinner_start: Instant::now(),
                    show_file_list: true,
                }
            }
            Err(e) => Self {
                state: AppState::Idle,
                repo_info: None,
                init_error: Some(format!("{} — press 'q' to quit", e)),
                commit_message: String::new(),
                suggested_version: None,
                pregen_message: None,
                pregen_suggested_version: None,
                status_line: String::new(),
                commit_hash: None,
                spinner_start: Instant::now(),
                show_file_list: true,
            },
        }
    }

    pub fn can_generate(&self) -> bool {
        self.state == AppState::Idle
            && self.init_error.is_none()
            && self.repo_info.as_ref().is_some_and(|r| r.has_changes)
    }

    pub fn can_commit(&self) -> bool {
        self.state == AppState::Ready
            && !self.commit_message.is_empty()
            && self.commit_hash.is_none()
    }

    pub fn can_copy_pregen(&self) -> bool {
        self.state == AppState::PreGenerated
            && self.pregen_message.is_some()
    }

    pub fn start_generating(&mut self) {
        self.state = AppState::Loading;
        self.spinner_start = Instant::now();
        self.status_line = "Generating commit message...".to_string();
    }

    pub fn start_committing(&mut self) {
        self.state = AppState::Committing;
        self.spinner_start = Instant::now();
        self.status_line = "Creating commit...".to_string();
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Generated(msg) => {
                self.commit_message = msg;
                self.state = AppState::Ready;
                self.status_line = "✓ Copied to clipboard".to_string();
            }
            AppEvent::GeneratedWithVersion {
                message,
                suggested_version,
            } => {
                self.commit_message = message;
                self.suggested_version = suggested_version;
                self.state = AppState::Ready;
                self.status_line = "✓ Copied to clipboard".to_string();
            }
            AppEvent::PreGenerated {
                message,
                suggested_version,
            } => {
                self.pregen_message = Some(message);
                self.pregen_suggested_version = suggested_version;
                self.state = AppState::PreGenerated;
                self.status_line = "✓ Ready — Press Enter".to_string();
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

        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(2),
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Length(1),
        ]);
        let [title_area, info_area, version_area, file_area, msg_area, action_area, status_area] =
            vertical.areas(size);

        self.render_title_bar(f, title_area);
        self.render_info_line(f, info_area);
        self.render_version_or_spinner(f, version_area);
        self.render_file_list(f, file_area);
        self.render_message(f, msg_area);
        self.render_actions(f, action_area);
        self.render_status_bar(f, status_area);
    }

    // ---- rendering helpers --------------------------------------------------

    fn render_title_bar(&self, f: &mut Frame, area: Rect) {
        let branch = self
            .repo_info
            .as_ref()
            .map(|r| r.branch.as_str())
            .unwrap_or("?");
        let path = self
            .repo_info
            .as_ref()
            .map(|r| r.repo_path.as_str())
            .unwrap_or("");

        let line = Line::from(vec![
            Span::raw(" Commiter "),
            Span::styled(
                format!("[{}]", branch),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw("   "),
            Span::styled(path, Style::default().dim()),
            Span::raw("  "),
        ]);

        let block = Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(Paragraph::new(line), inner);
    }

    fn render_info_line(&self, f: &mut Frame, area: Rect) {
        let info = match self.repo_info {
            Some(ref info) => {
                let mut spans = vec![
                    Span::styled(
                        format!("Staged: {} {}", info.staged_count, plural("file", info.staged_count)),
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
                ];
                if info.truncated {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        "[diff truncated]",
                        Style::default().fg(Color::Red).bold(),
                    ));
                }
                Line::from(spans)
            }
            None => Line::from(""),
        };
        f.render_widget(Paragraph::new(info), area);
    }

    fn render_version_or_spinner(&self, f: &mut Frame, area: Rect) {
        match self.state {
            AppState::PreGenerating | AppState::Loading | AppState::Committing => {
                let elapsed = self.spinner_start.elapsed();
                let frame = (elapsed.as_millis() / 100) as usize;
                let spinner = spinner_chars();
                let c = spinner[frame % spinner.len()];
                let label = match self.state {
                    AppState::PreGenerating => " Pre-generating commit message... ",
                    AppState::Loading => " Generating commit message... ",
                    AppState::Committing => " Creating commit... ",
                    _ => unreachable!(),
                };
                let line = Line::from(vec![
                    Span::styled(format!(" {} ", c), Style::default().fg(Color::Cyan).bold()),
                    Span::styled(label, Style::default().fg(Color::Cyan)),
                ]);
                f.render_widget(Paragraph::new(line), area);
            }
            _ => {
                // Show suggested version info if available
                if let Some(ref ver) = self.suggested_version {
                    let line = Line::from(vec![
                        Span::styled("Suggested version: ", Style::default().fg(Color::Magenta)),
                        Span::styled(ver, Style::default().fg(Color::Magenta).bold()),
                    ]);
                    f.render_widget(Paragraph::new(line), area);
                } else if self.repo_info.is_some() {
                    // Show version if detected
                    let version = crate::git::detect_current_version();
                    if let Some(ref ver) = version {
                        let line = Line::from(vec![
                            Span::styled("Version: ", Style::default().dim()),
                            Span::styled(ver, Style::default().fg(Color::Blue)),
                        ]);
                        f.render_widget(Paragraph::new(line), area);
                    }
                }
            }
        }
    }

    fn render_file_list(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Changed Files ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let files = self
            .repo_info
            .as_ref()
            .map(|r| r.changed_files.as_slice())
            .unwrap_or(&[]);

        if !self.show_file_list || files.is_empty() {
            return;
        }

        let lines: Vec<Line> = files
            .iter()
            .map(|f| {
                Line::from(Span::styled(
                    format!("  {}", f),
                    Style::default().fg(Color::White),
                ))
            })
            .collect();

        let p = Paragraph::new(lines);
        f.render_widget(p, inner);
    }

    fn render_message(&self, f: &mut Frame, area: Rect) {
        let display_text = match self.state {
            AppState::PreGenerated => self.pregen_message.as_deref().unwrap_or(""),
            _ => self.commit_message.as_str(),
        };

        let border_style = if !display_text.is_empty() {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(" Commit Message ")
            .borders(Borders::ALL)
            .border_style(border_style);
        let inner = block.inner(area);
        f.render_widget(block, area);

        if !display_text.is_empty() {
            let p = Paragraph::new(display_text)
                .wrap(Wrap { trim: false });
            f.render_widget(p, inner);
        }
    }

    fn render_actions(&self, f: &mut Frame, area: Rect) {
        let [btn_area, hints_area] =
            Layout::horizontal([Constraint::Length(28), Constraint::Min(1)]).areas(area);

        let has_msg = !self.commit_message.is_empty();
        let is_pregen = self.state == AppState::PreGenerated;
        let is_pregen_loading = self.state == AppState::PreGenerating;
        let is_loading = self.state == AppState::Loading;
        let is_committing = self.state == AppState::Committing;
        let is_ready = self.state == AppState::Ready;

        let mut buttons: Vec<Paragraph> = Vec::new();

        if is_pregen {
            // Pre-generated message ready — one button
            buttons.push(self.make_button("  Use message ✓  ", true));
        } else if is_pregen_loading {
            buttons.push(self.make_button("  Pre-generating…  ", false));
        } else {
            let gen_label = if is_loading {
                "  Generating…  "
            } else if has_msg {
                "  Regenerate  "
            } else {
                "  Create commit message  "
            };
            let gen_active = self.can_generate() || (is_ready && self.can_generate());
            buttons.push(self.make_button(gen_label, gen_active));

            if has_msg {
                let commit_label = if is_committing {
                    "  Committing…  "
                } else {
                    "  Commit changes  "
                };
                buttons.push(self.make_button(commit_label, self.can_commit()));
            }
        }

        // Render buttons
        if buttons.len() == 2 {
            let [gen, commit] =
                Layout::horizontal([Constraint::Length(24), Constraint::Length(24)])
                    .areas(btn_area);
            f.render_widget(buttons.remove(0), gen);
            f.render_widget(buttons.remove(0), commit);
        } else {
            f.render_widget(buttons.remove(0), btn_area);
        }

        // Key hints
        let hint_enter = if is_pregen {
            " use message  "
        } else if is_ready && has_msg {
            " commit  "
        } else {
            " generate  "
        };
        let hints = Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Blue).bold()),
            Span::raw(hint_enter),
            Span::styled("F1", Style::default().fg(Color::Blue).bold()),
            Span::raw(" toggle files  "),
            Span::styled("q", Style::default().fg(Color::Blue).bold()),
            Span::raw(" quit"),
        ]);
        f.render_widget(Paragraph::new(hints).alignment(Alignment::Right), hints_area);
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let text = &self.status_line;
        if text.is_empty() {
            return;
        }

        let style = if text.starts_with('✓') {
            Style::default().fg(Color::Green).bold()
        } else if text.starts_with('✗') {
            Style::default().fg(Color::Red).bold()
        } else if text.to_lowercase().contains("error") {
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

    fn make_button(&self, label: &str, active: bool) -> Paragraph<'static> {
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
        Paragraph::new(Line::from(Span::styled(label.to_string(), style)))
            .block(block)
            .alignment(Alignment::Center)
    }
}

fn spinner_chars() -> &'static [char] {
    &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
}

fn plural(s: &str, n: usize) -> String {
    if n == 1 {
        s.to_string()
    } else {
        format!("{}s", s)
    }
}
