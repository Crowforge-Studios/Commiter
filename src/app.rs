use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use std::time::Instant;

use crate::git::RepoInfo;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    Idle,
    PreGenerating,
    PreGenerated,
    Loading,
    Ready,
    Committing,
}

#[derive(Debug)]
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
    pub editing: bool,
    pub cursor_pos: usize,
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
                    editing: false,
                    cursor_pos: 0,
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
                editing: false,
                cursor_pos: 0,
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

    pub fn start_editing(&mut self) {
        self.editing = true;
        self.cursor_pos = self.commit_message.len();
        self.status_line = "Editing message — Esc to finish".to_string();
    }

    pub fn stop_editing(&mut self) {
        self.editing = false;
        self.status_line = "✓ Message set — Enter to commit".to_string();
    }

    pub fn insert_char(&mut self, c: char) {
        self.commit_message.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn delete_backward(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.commit_message[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, c)| (i, c.len_utf8()))
                .unwrap_or((0, 0));
            self.commit_message.drain(prev.0..self.cursor_pos);
            self.cursor_pos = prev.0;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor_pos < self.commit_message.len() {
            let next = self.commit_message[self.cursor_pos..]
                .char_indices()
                .next()
                .map(|(_, c)| c.len_utf8())
                .unwrap_or(0);
            self.commit_message.drain(self.cursor_pos..self.cursor_pos + next);
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.commit_message[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.cursor_pos = prev;
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.commit_message.len() {
            let next = self.commit_message[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor_pos + i)
                .unwrap_or(self.commit_message.len());
            self.cursor_pos = next;
        }
    }

    pub fn cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cursor_pos = self.commit_message.len();
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

        if let Some(ref err) = self.init_error {
            let block = Block::default()
                .title(" Commiter — Error ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
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
            Span::styled(" Commiter ", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" "),
            Span::styled(
                format!(" {}", branch),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw("  "),
            Span::styled(path, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
        ]);

        let block = Block::default()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::Rounded)
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
                        format!(" +{} ", info.staged_count),
                        Style::default().fg(Color::Green).bold(),
                    ),
                    Span::styled(" staged ", Style::default().fg(Color::Green)),
                    Span::raw("  "),
                    Span::styled(
                        format!(" ~{} ", info.unstaged_count),
                        Style::default().fg(Color::Yellow).bold(),
                    ),
                    Span::styled(" unstaged ", Style::default().fg(Color::Yellow)),
                ];
                if info.truncated {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        " [truncated] ",
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
                    Span::styled(label, Style::default().fg(Color::Cyan).bold()),
                ]);
                f.render_widget(Paragraph::new(line), area);
            }
            _ => {
                if let Some(ref ver) = self.suggested_version {
                    let line = Line::from(vec![
                        Span::styled("Suggested version: ", Style::default().fg(Color::Magenta)),
                        Span::styled(ver, Style::default().fg(Color::Magenta).bold()),
                    ]);
                    f.render_widget(Paragraph::new(line), area);
                } else if self.repo_info.is_some() {
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
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let (files, statuses) = self
            .repo_info
            .as_ref()
            .map(|r| (r.changed_files.as_slice(), r.changed_statuses.as_slice()))
            .unwrap_or((&[], &[]));

        if !self.show_file_list || files.is_empty() {
            return;
        }

        let lines: Vec<Line> = files
            .iter()
            .zip(statuses.iter())
            .map(|(f, s)| {
                let (icon, color) = match s.as_str() {
                    "A" => ("+", Color::Green),
                    "M" => ("~", Color::Yellow),
                    "D" => ("-", Color::Red),
                    "R" => (">", Color::Cyan),
                    _ => (" ", Color::White),
                };
                Line::from(vec![
                    Span::styled(format!(" {} ", icon), Style::default().fg(color).bold()),
                    Span::styled(f.clone(), Style::default().fg(Color::White)),
                ])
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

        let border_style = if self.editing {
            Style::default().fg(Color::Yellow)
        } else if !display_text.is_empty() {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let title = if self.editing {
            " Commit Message [EDITING] "
        } else {
            " Commit Message "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);
        let inner = block.inner(area);
        f.render_widget(block, area);

        if !display_text.is_empty() || self.editing {
            let p = Paragraph::new(display_text)
                .wrap(Wrap { trim: false });
            f.render_widget(p, inner);

            if self.editing && inner.width > 0 {
                let text_before = &self.commit_message[..self.cursor_pos.min(self.commit_message.len())];
                let mut visual_y = 0u16;
                for line in text_before.split('\n') {
                    if line.is_empty() {
                        visual_y += 1;
                    } else {
                        visual_y += (line.len() as u16 + inner.width - 1) / inner.width;
                    }
                }
                visual_y = visual_y.saturating_sub(1);
                let last_line = text_before.split('\n').last().unwrap_or("");
                let visual_x = if inner.width > 0 {
                    (last_line.len() as u16) % inner.width
                } else {
                    0
                };
                f.set_cursor_position((
                    inner.x + visual_x.min(inner.width.saturating_sub(1)),
                    inner.y + visual_y.min(inner.height.saturating_sub(1)),
                ));
            }
        }
    }

    fn render_actions(&self, f: &mut Frame, area: Rect) {
        let [btn_area, hints_area] =
            Layout::horizontal([Constraint::Length(28), Constraint::Min(1)]).areas(area);

        if self.editing {
            let btn = self.make_button("  Finish editing [Esc]  ", true);
            f.render_widget(btn, btn_area);

            let hints = Line::from(vec![
                Span::styled("Esc", Style::default().fg(Color::Blue).bold()),
                Span::raw(" finish  "),
                Span::styled("F1", Style::default().fg(Color::Blue).bold()),
                Span::raw(" files  "),
                Span::styled("q", Style::default().fg(Color::Blue).bold()),
                Span::raw(" quit"),
            ]);
            f.render_widget(Paragraph::new(hints).alignment(Alignment::Right), hints_area);
            return;
        }

        let has_msg = !self.commit_message.is_empty();
        let is_pregen = self.state == AppState::PreGenerated;
        let is_pregen_loading = self.state == AppState::PreGenerating;
        let is_loading = self.state == AppState::Loading;
        let is_committing = self.state == AppState::Committing;
        let is_ready = self.state == AppState::Ready;

        let mut buttons: Vec<Paragraph> = Vec::new();

        if is_pregen {
            buttons.push(self.make_button("  Use message ✓  ", true));
        } else if is_pregen_loading {
            buttons.push(self.make_button("  Pre-generating…  ", false));
        } else if is_loading || is_committing {
            let label = if is_loading { "  Generating…  " } else { "  Committing…  " };
            buttons.push(self.make_button(label, false));
        } else {
            let can_gen = self.can_generate();
            let gen_active = can_gen || (is_ready && has_msg);
            buttons.push(self.make_button(
                if can_gen { "  Create commit message  " }
                else if has_msg { "  Regenerate  " }
                else { "  Create commit message  " },
                gen_active,
            ));

            if has_msg {
                buttons.push(self.make_button("  Commit changes  ", self.can_commit()));
            }
        }

        if buttons.len() == 2 {
            let [gen, commit] =
                Layout::horizontal([Constraint::Length(24), Constraint::Length(24)])
                    .areas(btn_area);
            f.render_widget(buttons.remove(0), gen);
            f.render_widget(buttons.remove(0), commit);
        } else if buttons.len() == 1 {
            f.render_widget(buttons.remove(0), btn_area);
        }

        let is_busy = is_loading || is_committing || is_pregen_loading;
        let mut hints = vec![];

        if is_pregen {
            hints.push(Span::styled("Enter", Style::default().fg(Color::Blue).bold()));
            hints.push(Span::raw(" use message  "));
        } else if is_ready && has_msg {
            hints.push(Span::styled("Enter", Style::default().fg(Color::Blue).bold()));
            hints.push(Span::raw(" commit  "));
        } else if !is_busy {
            hints.push(Span::styled("Enter", Style::default().fg(Color::Blue).bold()));
            hints.push(Span::raw(" generate  "));
        }

        if has_msg && is_ready {
            hints.push(Span::styled("e", Style::default().fg(Color::Blue).bold()));
            hints.push(Span::raw(" edit  "));
        }
        if has_msg {
            hints.push(Span::styled("r", Style::default().fg(Color::Blue).bold()));
            hints.push(Span::raw(" regen  "));
        }
        hints.push(Span::styled("F1", Style::default().fg(Color::Blue).bold()));
        hints.push(Span::raw(" files  "));
        hints.push(Span::styled("q", Style::default().fg(Color::Blue).bold()));
        hints.push(Span::raw(" quit"));

        let hints_line = Line::from(hints);
        f.render_widget(Paragraph::new(hints_line).alignment(Alignment::Right), hints_area);
    }

    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let text = &self.status_line;
        if text.is_empty() {
            return;
        }

        let (fg, bg) = if text.starts_with('✓') {
            (Color::Green, Color::Reset)
        } else if text.starts_with('✗') || text.to_lowercase().contains("error") {
            (Color::Red, Color::Reset)
        } else if text.contains("truncated") {
            (Color::Yellow, Color::Reset)
        } else {
            (Color::White, Color::Reset)
        };
        let style = Style::default().fg(fg).bg(bg).bold();

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
            .border_type(BorderType::Rounded)
            .border_style(border_style);
        Paragraph::new(Line::from(Span::styled(label.to_string(), style)))
            .block(block)
            .alignment(Alignment::Center)
    }
}

fn spinner_chars() -> &'static [char] {
    &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn make_app() -> App {
        App {
            state: AppState::Idle,
            repo_info: None,
            init_error: None,
            commit_message: String::new(),
            suggested_version: None,
            pregen_message: None,
            pregen_suggested_version: None,
            status_line: String::new(),
            commit_hash: None,
            spinner_start: Instant::now(),
            show_file_list: true,
            editing: false,
            cursor_pos: 0,
        }
    }

    #[test]
    fn test_can_generate_idle_no_changes() {
        let app = make_app();
        assert!(!app.can_generate());
    }

    #[test]
    fn test_can_generate_idle_with_changes() {
        let mut app = make_app();
        app.repo_info = Some(crate::git::RepoInfo {
            repo_path: ".".into(),
            branch: "main".into(),
            staged_count: 1,
            unstaged_count: 0,
            changed_files: vec!["foo.rs".into()],
            changed_statuses: vec!["M".into()],
            combined_diff: "diff --git a/foo.rs b/foo.rs".into(),
            truncated: false,
            has_changes: true,
        });
        assert!(app.can_generate());
    }

    #[test]
    fn test_can_generate_wrong_state() {
        let mut app = make_app();
        app.state = AppState::Ready;
        assert!(!app.can_generate());
    }

    #[test]
    fn test_can_commit_ready_with_message() {
        let mut app = make_app();
        app.state = AppState::Ready;
        app.commit_message = "feat: add feature".into();
        assert!(app.can_commit());
    }

    #[test]
    fn test_can_commit_already_committed() {
        let mut app = make_app();
        app.state = AppState::Ready;
        app.commit_message = "feat: add feature".into();
        app.commit_hash = Some("abc1234".into());
        assert!(!app.can_commit());
    }

    #[test]
    fn test_can_commit_empty_message() {
        let mut app = make_app();
        app.state = AppState::Ready;
        assert!(!app.can_commit());
    }

    #[test]
    fn test_can_copy_pregen() {
        let mut app = make_app();
        app.state = AppState::PreGenerated;
        app.pregen_message = Some("feat: add".into());
        assert!(app.can_copy_pregen());
    }

    #[test]
    fn test_can_copy_pregen_no_message() {
        let mut app = make_app();
        app.state = AppState::PreGenerated;
        assert!(!app.can_copy_pregen());
    }

    #[test]
    fn test_handle_event_generated() {
        let mut app = make_app();
        app.handle_event(AppEvent::Generated("feat: x".into()));
        assert_eq!(app.commit_message, "feat: x");
        assert_eq!(app.state, AppState::Ready);
    }

    #[test]
    fn test_handle_event_committed() {
        let mut app = make_app();
        app.handle_event(AppEvent::Committed("deadbeef1234567".into()));
        assert_eq!(app.commit_hash, Some("deadbeef1234567".into()));
        assert_eq!(app.state, AppState::Ready);
    }

    #[test]
    fn test_handle_event_error() {
        let mut app = make_app();
        app.handle_event(AppEvent::Error("something went wrong".into()));
        assert_eq!(app.state, AppState::Idle);
        assert!(app.status_line.contains("something went wrong"));
    }

    #[test]
    fn test_insert_char() {
        let mut app = make_app();
        app.commit_message = "helo".into();
        app.cursor_pos = 3;
        app.insert_char('l');
        assert_eq!(app.commit_message, "hello");
        assert_eq!(app.cursor_pos, 4);
    }

    #[test]
    fn test_insert_char_at_end() {
        let mut app = make_app();
        app.commit_message = "hello".into();
        app.cursor_pos = 5;
        app.insert_char('!');
        assert_eq!(app.commit_message, "hello!");
        assert_eq!(app.cursor_pos, 6);
    }

    #[test]
    fn test_delete_backward() {
        let mut app = make_app();
        app.commit_message = "hello".into();
        app.cursor_pos = 5;
        app.delete_backward();
        assert_eq!(app.commit_message, "hell");
        assert_eq!(app.cursor_pos, 4);
    }

    #[test]
    fn test_delete_backward_at_start() {
        let mut app = make_app();
        app.commit_message = "hello".into();
        app.cursor_pos = 0;
        app.delete_backward();
        assert_eq!(app.commit_message, "hello");
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn test_delete_forward() {
        let mut app = make_app();
        app.commit_message = "hello".into();
        app.cursor_pos = 0;
        app.delete_forward();
        assert_eq!(app.commit_message, "ello");
        assert_eq!(app.cursor_pos, 0);
    }

    #[test]
    fn test_delete_forward_at_end() {
        let mut app = make_app();
        app.commit_message = "hello".into();
        app.cursor_pos = 5;
        app.delete_forward();
        assert_eq!(app.commit_message, "hello");
        assert_eq!(app.cursor_pos, 5);
    }

    #[test]
    fn test_cursor_left_right() {
        let mut app = make_app();
        app.commit_message = "hello".into();
        app.cursor_pos = 5;
        app.cursor_left();
        assert_eq!(app.cursor_pos, 4);
        app.cursor_left();
        assert_eq!(app.cursor_pos, 3);
        app.cursor_right();
        assert_eq!(app.cursor_pos, 4);
    }

    #[test]
    fn test_cursor_home_end() {
        let mut app = make_app();
        app.commit_message = "hello world".into();
        app.cursor_pos = 5;
        app.cursor_home();
        assert_eq!(app.cursor_pos, 0);
        app.cursor_end();
        assert_eq!(app.cursor_pos, 11);
    }

    #[test]
    fn test_insert_char_utf8() {
        let mut app = make_app();
        app.commit_message = "héllo".into();
        app.cursor_pos = 3; // after 'é' (2-byte char at bytes 1-2)
        app.insert_char('!');
        assert_eq!(app.commit_message, "hé!llo");
        assert_eq!(app.cursor_pos, 4);
    }

    #[test]
    fn test_delete_backward_utf8() {
        let mut app = make_app();
        app.commit_message = "héllo".into();
        app.cursor_pos = 6; // end of string
        app.delete_backward();
        // 'o' removed
        app.delete_backward();
        // 'l' removed
        app.delete_backward();
        // 'l' removed
        app.delete_backward();
        // 'é' removed (2 bytes)
        assert_eq!(app.commit_message, "h");
        assert_eq!(app.cursor_pos, 1);
    }

    #[test]
    fn test_cursor_bounds() {
        let mut app = make_app();
        app.commit_message = "hi".into();
        app.cursor_pos = 0;
        app.cursor_left();
        assert_eq!(app.cursor_pos, 0);
        app.cursor_pos = 2;
        app.cursor_right();
        assert_eq!(app.cursor_pos, 2);
    }

    #[test]
    fn test_start_editing() {
        let mut app = make_app();
        app.commit_message = "feat: add".into();
        app.start_editing();
        assert!(app.editing);
        assert_eq!(app.cursor_pos, 9);
    }

    #[test]
    fn test_stop_editing() {
        let mut app = make_app();
        app.commit_message = "feat: add".into();
        app.editing = true;
        app.stop_editing();
        assert!(!app.editing);
    }
}
