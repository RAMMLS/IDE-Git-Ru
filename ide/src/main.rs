use aura_control::{
    AuraError, ChangeKind, CommitOutcome, CommitSummary, DiffLine, FileDiff, PullStatus,
    PullSummary, PushSummary, Repository,
};
use iced::highlighter;
use iced::widget::{
    button, column, container, row, scrollable, text, text_editor, text_input, Column,
};
use iced::{executor, Application, Color, Command, Element, Font, Length, Settings, Theme};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use syntect::parsing::SyntaxSet;
use tokio::process::Command as TokioCommand;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarTab {
    Explorer,
    SourceControl,
}

#[derive(Debug, Clone)]
enum Message {
    EditorAction(text_editor::Action),
    SelectTab(SidebarTab),
    NewFileNameChanged(String),
    CreateFile,
    OpenFile(PathBuf),
    SaveFile,
    ToggleDirectory(PathBuf),
    TerminalInputChanged(String),
    RunTerminal,
    TerminalFinished(String),
    CommitMessageChanged(String),
    RefreshSourceControl,
    SourceControlLoaded(Result<SourceControlSnapshot, String>),
    SelectChange(PathBuf),
    Commit,
    Push,
    Pull,
    SourceActionFinished(String),
}

#[derive(Debug, Clone)]
struct ChangeItem {
    path: PathBuf,
    kind: ChangeKind,
}

#[derive(Debug, Clone, Default)]
struct SourceControlSnapshot {
    branch: Option<String>,
    head: Option<String>,
    staged: Vec<ChangeItem>,
    unstaged: Vec<ChangeItem>,
    untracked: Vec<PathBuf>,
    diffs: Vec<FileDiff>,
    log: Vec<CommitSummary>,
}

impl SourceControlSnapshot {
    fn first_change_path(&self) -> Option<PathBuf> {
        self.staged
            .first()
            .map(|entry| entry.path.clone())
            .or_else(|| self.unstaged.first().map(|entry| entry.path.clone()))
            .or_else(|| self.untracked.first().cloned())
            .or_else(|| self.diffs.first().map(|entry| entry.path.clone()))
    }

    fn contains_path(&self, path: &Path) -> bool {
        self.staged.iter().any(|entry| entry.path == path)
            || self.unstaged.iter().any(|entry| entry.path == path)
            || self.untracked.iter().any(|entry| entry == path)
            || self.diffs.iter().any(|entry| entry.path == path)
    }

    fn diff_for(&self, path: &Path) -> Option<&FileDiff> {
        self.diffs.iter().find(|diff| diff.path == path)
    }
}

#[derive(Debug, Clone)]
enum SourceAction {
    Commit(String),
    Push,
    Pull,
}

struct AuraIde {
    workspace: PathBuf,
    active_file: Option<PathBuf>,
    editor: text_editor::Content,
    editor_syntax: String,
    editor_theme: highlighter::Theme,
    sidebar_tab: SidebarTab,
    expanded_dirs: BTreeSet<PathBuf>,
    new_file_name: String,
    terminal_input: String,
    terminal_output: String,
    source_control: SourceControlSnapshot,
    selected_change: Option<PathBuf>,
    commit_message: String,
    source_busy: bool,
}

impl Default for AuraIde {
    fn default() -> Self {
        let workspace = std::env::var_os("AURA_IDE_WORKSPACE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("db"));
        let _ = fs::create_dir_all(&workspace);
        let workspace = fs::canonicalize(&workspace).unwrap_or(workspace);

        Self {
            workspace,
            active_file: None,
            editor: text_editor::Content::new(),
            editor_syntax: String::from("txt"),
            editor_theme: highlighter::Theme::SolarizedDark,
            sidebar_tab: SidebarTab::Explorer,
            expanded_dirs: BTreeSet::new(),
            new_file_name: String::new(),
            terminal_input: String::new(),
            terminal_output: String::from("Aura IDE terminal\n"),
            source_control: SourceControlSnapshot::default(),
            selected_change: None,
            commit_message: String::new(),
            source_busy: false,
        }
    }
}

impl Application for AuraIde {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let app = AuraIde::default();
        let workspace = app.workspace.clone();
        (
            app,
            Command::perform(load_source_control(workspace), Message::SourceControlLoaded),
        )
    }

    fn title(&self) -> String {
        String::from("Aura IDE")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::EditorAction(action) => {
                self.editor.perform(action);
            }
            Message::SelectTab(tab) => self.sidebar_tab = tab,
            Message::NewFileNameChanged(value) => self.new_file_name = value,
            Message::CreateFile => {
                let name = self.new_file_name.trim();
                if !name.is_empty() {
                    let path = self.workspace.join(name);
                    if name.ends_with('/') || !name.contains('.') {
                        let _ = fs::create_dir_all(&path);
                    } else {
                        if let Some(parent) = path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let _ = fs::OpenOptions::new()
                            .create(true)
                            .write(true)
                            .truncate(false)
                            .open(&path);
                    }
                    self.new_file_name.clear();
                    self.source_busy = true;
                    return Command::perform(
                        load_source_control(self.workspace.clone()),
                        Message::SourceControlLoaded,
                    );
                }
            }
            Message::OpenFile(path) => {
                if let Ok(contents) = fs::read_to_string(&path) {
                    self.editor = text_editor::Content::with_text(&contents);
                    self.editor_syntax = syntax_token_for_path(Some(&path));
                    self.active_file = Some(path);
                }
            }
            Message::SaveFile => {
                if let Some(path) = &self.active_file {
                    let _ = fs::write(path, self.editor.text());
                    self.source_busy = true;
                    return Command::perform(
                        load_source_control(self.workspace.clone()),
                        Message::SourceControlLoaded,
                    );
                }
            }
            Message::ToggleDirectory(path) => {
                if !self.expanded_dirs.remove(&path) {
                    self.expanded_dirs.insert(path);
                }
            }
            Message::TerminalInputChanged(value) => self.terminal_input = value,
            Message::RunTerminal => {
                let command = self.terminal_input.trim().to_string();
                if command.is_empty() {
                    return Command::none();
                }
                self.terminal_output.push_str(&format!(
                    "{}> {}\n",
                    self.workspace.display(),
                    command
                ));
                self.terminal_input.clear();
                return Command::perform(
                    run_shell_command(self.workspace.clone(), command),
                    Message::TerminalFinished,
                );
            }
            Message::TerminalFinished(output) => self.terminal_output.push_str(&output),
            Message::CommitMessageChanged(value) => self.commit_message = value,
            Message::RefreshSourceControl => {
                self.source_busy = true;
                return Command::perform(
                    load_source_control(self.workspace.clone()),
                    Message::SourceControlLoaded,
                );
            }
            Message::SourceControlLoaded(result) => {
                self.source_busy = false;
                match result {
                    Ok(snapshot) => {
                        let should_reset = self
                            .selected_change
                            .as_ref()
                            .map_or(true, |path| !snapshot.contains_path(path));
                        if should_reset {
                            self.selected_change = snapshot.first_change_path();
                        }
                        self.source_control = snapshot;
                    }
                    Err(error) => {
                        self.source_control = SourceControlSnapshot::default();
                        self.selected_change = None;
                        self.terminal_output
                            .push_str(&format!("Source control refresh failed: {error}\n"));
                    }
                }
            }
            Message::SelectChange(path) => {
                self.selected_change = Some(path.clone());
                let absolute = self.workspace.join(&path);
                if absolute.is_file() {
                    if let Ok(contents) = fs::read_to_string(&absolute) {
                        self.editor = text_editor::Content::with_text(&contents);
                        self.editor_syntax = syntax_token_for_path(Some(&absolute));
                        self.active_file = Some(absolute);
                    }
                }
            }
            Message::Commit => {
                let message = self.commit_message.trim().to_string();
                if message.is_empty() || self.source_busy {
                    return Command::none();
                }
                self.source_busy = true;
                self.commit_message.clear();
                return Command::perform(
                    run_source_action(self.workspace.clone(), SourceAction::Commit(message)),
                    Message::SourceActionFinished,
                );
            }
            Message::Push => {
                if self.source_busy {
                    return Command::none();
                }
                self.source_busy = true;
                return Command::perform(
                    run_source_action(self.workspace.clone(), SourceAction::Push),
                    Message::SourceActionFinished,
                );
            }
            Message::Pull => {
                if self.source_busy {
                    return Command::none();
                }
                self.source_busy = true;
                return Command::perform(
                    run_source_action(self.workspace.clone(), SourceAction::Pull),
                    Message::SourceActionFinished,
                );
            }
            Message::SourceActionFinished(output) => {
                self.terminal_output.push_str(&output);
                self.source_busy = true;
                return Command::perform(
                    load_source_control(self.workspace.clone()),
                    Message::SourceControlLoaded,
                );
            }
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = container(
            column![
                row![
                    button("Explorer").on_press(Message::SelectTab(SidebarTab::Explorer)),
                    button("Source Control")
                        .on_press(Message::SelectTab(SidebarTab::SourceControl)),
                ]
                .spacing(8),
                match self.sidebar_tab {
                    SidebarTab::Explorer => self.explorer_view(),
                    SidebarTab::SourceControl => self.source_control_view(),
                }
            ]
            .spacing(12)
            .padding(12),
        )
        .width(Length::Fixed(360.0))
        .height(Length::Fill);

        let editor_title = self
            .active_file
            .as_ref()
            .and_then(|path| path.strip_prefix(&self.workspace).ok())
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| String::from("No file selected"));

        let editor = container(
            column![
                row![
                    text(editor_title).size(14),
                    text(format!("Syntax: {}", self.editor_syntax)).size(12),
                    button("Save").on_press(Message::SaveFile),
                ]
                .spacing(12),
                text_editor(&self.editor)
                    .font(Font::MONOSPACE)
                    .highlight::<highlighter::Highlighter>(
                        highlighter::Settings {
                            theme: self.editor_theme,
                            extension: self.editor_syntax.clone(),
                        },
                        |highlight, _theme| highlight.to_format(),
                    )
                    .on_action(Message::EditorAction)
                    .height(Length::FillPortion(3)),
                container(column![
                    text("Terminal").size(12),
                    scrollable(text(&self.terminal_output).size(13).font(Font::MONOSPACE))
                        .height(Length::Fixed(160.0)),
                    text_input("command", &self.terminal_input)
                        .on_input(Message::TerminalInputChanged)
                        .on_submit(Message::RunTerminal),
                ])
            ]
            .spacing(10)
            .padding(12),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        row![sidebar, editor].height(Length::Fill).into()
    }
}

impl AuraIde {
    fn explorer_view(&self) -> Element<'_, Message> {
        column![
            text(self.workspace.display().to_string()).size(12),
            text_input("file.rs or src/", &self.new_file_name)
                .on_input(Message::NewFileNameChanged)
                .on_submit(Message::CreateFile),
            button("Create").on_press(Message::CreateFile),
            scrollable(view_directory(self, &self.workspace, 0)).height(Length::Fill),
        ]
        .spacing(8)
        .into()
    }

    fn source_control_view(&self) -> Element<'_, Message> {
        let busy = if self.source_busy { "Syncing..." } else { "Ready" };
        let branch = self
            .source_control
            .branch
            .clone()
            .unwrap_or_else(|| String::from("detached"));
        let head = self
            .source_control
            .head
            .as_deref()
            .map(short_oid)
            .unwrap_or("empty");

        column![
            row![
                text(format!("{busy}  Branch: {branch}  HEAD: {head}")).size(12),
                action_button("Refresh", !self.source_busy, Message::RefreshSourceControl),
            ]
            .spacing(8),
            text_input("Commit message", &self.commit_message)
                .on_input(Message::CommitMessageChanged)
                .on_submit(Message::Commit),
            row![
                action_button("Commit", !self.source_busy, Message::Commit),
                action_button("Pull", !self.source_busy, Message::Pull),
                action_button("Push", !self.source_busy, Message::Push),
            ]
            .spacing(8),
            text("Changes").size(12),
            scrollable(self.change_sections()).height(Length::Fixed(210.0)),
            text(self.selected_diff_title()).size(12),
            scrollable(self.diff_view()).height(Length::Fixed(220.0)),
            text("Commit Graph").size(12),
            scrollable(self.commit_graph_view()).height(Length::Fill),
        ]
        .spacing(8)
        .into()
    }

    fn change_sections(&self) -> Column<'_, Message> {
        column![
            self.change_section(
                "Staged",
                &self.source_control.staged,
                Some(Color::from_rgb(0.45, 0.85, 0.55)),
                "No staged changes",
            ),
            self.change_section(
                "Unstaged",
                &self.source_control.unstaged,
                Some(Color::from_rgb(0.98, 0.73, 0.32)),
                "No unstaged changes",
            ),
            self.untracked_section(),
        ]
        .spacing(10)
    }

    fn change_section(
        &self,
        title: &'static str,
        entries: &[ChangeItem],
        color: Option<Color>,
        empty_label: &'static str,
    ) -> Element<'_, Message> {
        let mut content = Column::new().spacing(4).push(text(title).size(12));

        if entries.is_empty() {
            content = content.push(
                text(empty_label)
                    .size(11)
                    .style(iced::theme::Text::Color(Color::from_rgb(0.55, 0.55, 0.6))),
            );
        } else {
            for entry in entries {
                let selected = self
                    .selected_change
                    .as_ref()
                    .is_some_and(|path| path == &entry.path);
                let marker = if selected { ">" } else { " " };
                let label = format!(
                    "{marker} {} {}",
                    change_marker(entry.kind),
                    entry.path.display()
                );
                let button_text = text(label)
                    .size(12)
                    .font(Font::MONOSPACE)
                    .style(iced::theme::Text::Color(
                        color.unwrap_or(Color::from_rgb(1.0, 1.0, 1.0)),
                    ));
                content = content
                    .push(button(button_text).on_press(Message::SelectChange(entry.path.clone())));
            }
        }

        content.into()
    }

    fn untracked_section(&self) -> Element<'_, Message> {
        let mut content = Column::new().spacing(4).push(text("Untracked").size(12));

        if self.source_control.untracked.is_empty() {
            content = content.push(
                text("No untracked files")
                    .size(11)
                    .style(iced::theme::Text::Color(Color::from_rgb(0.55, 0.55, 0.6))),
            );
        } else {
            for path in &self.source_control.untracked {
                let selected = self
                    .selected_change
                    .as_ref()
                    .is_some_and(|selected| selected == path);
                let marker = if selected { ">" } else { " " };
                let button_text = text(format!("{marker} ? {}", path.display()))
                    .size(12)
                    .font(Font::MONOSPACE)
                    .style(iced::theme::Text::Color(Color::from_rgb(0.85, 0.75, 0.35)));
                content =
                    content.push(button(button_text).on_press(Message::SelectChange(path.clone())));
            }
        }

        content.into()
    }

    fn selected_diff_title(&self) -> String {
        self.selected_change
            .as_ref()
            .map(|path| format!("Diff: {}", path.display()))
            .unwrap_or_else(|| String::from("Diff"))
    }

    fn diff_view(&self) -> Column<'_, Message> {
        let mut column = Column::new().spacing(2);
        let selected = self
            .selected_change
            .as_ref()
            .cloned()
            .or_else(|| self.source_control.first_change_path());

        let Some(path) = selected else {
            return column.push(text("No pending changes").size(12));
        };

        let Some(diff) = self.source_control.diff_for(&path) else {
            if self.source_control.untracked.iter().any(|entry| entry == &path) {
                return column.push(
                    text("Untracked file: open it in the editor to inspect contents.")
                        .size(12)
                        .style(iced::theme::Text::Color(Color::from_rgb(0.75, 0.75, 0.8))),
                );
            }

            return column.push(
                text("No diff preview available for this path yet.")
                    .size(12)
                    .style(iced::theme::Text::Color(Color::from_rgb(0.75, 0.75, 0.8))),
            );
        };

        for line in &diff.lines {
            let (prefix, value, color) = match line {
                DiffLine::Context(value) => (
                    " ",
                    render_diff_text(value),
                    Color::from_rgb(0.82, 0.82, 0.85),
                ),
                DiffLine::Addition(value) => (
                    "+",
                    render_diff_text(value),
                    Color::from_rgb(0.52, 0.9, 0.58),
                ),
                DiffLine::Deletion(value) => (
                    "-",
                    render_diff_text(value),
                    Color::from_rgb(0.95, 0.48, 0.48),
                ),
            };
            column = column.push(
                container(
                    text(format!("{prefix} {value}"))
                        .size(12)
                        .font(Font::MONOSPACE)
                        .style(iced::theme::Text::Color(color)),
                )
                .padding([2, 6]),
            );
        }

        column
    }

    fn commit_graph_view(&self) -> Column<'_, Message> {
        let mut column = Column::new().spacing(6);

        if self.source_control.log.is_empty() {
            return column.push(text("No commits yet").size(12));
        }

        for commit in &self.source_control.log {
            let parent_info = if commit.parents.is_empty() {
                String::from("root")
            } else {
                format!("parent {}", short_oid(&commit.parents[0]))
            };
            column = column
                .push(
                    text(format!(
                        "* {}  {}  {}",
                        short_oid(&commit.oid),
                        commit.timestamp.format("%Y-%m-%d %H:%M"),
                        commit.author
                    ))
                    .size(12)
                    .font(Font::MONOSPACE),
                )
                .push(
                    text(format!("  {}", commit.message.replace('\n', " ")))
                        .size(12)
                        .style(iced::theme::Text::Color(Color::from_rgb(0.85, 0.85, 0.9))),
                )
                .push(
                    text(format!("  {}", parent_info))
                        .size(11)
                        .style(iced::theme::Text::Color(Color::from_rgb(0.55, 0.55, 0.6))),
                );
        }

        column
    }
}

fn action_button(
    label: &'static str,
    enabled: bool,
    message: Message,
) -> Element<'static, Message> {
    let button = if enabled {
        button(label).on_press(message)
    } else {
        button(label)
    };
    button.into()
}

fn view_directory<'a>(ide: &'a AuraIde, path: &Path, depth: usize) -> Column<'a, Message> {
    let mut column = Column::new().spacing(3);
    let mut entries = match fs::read_dir(path) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return column,
    };
    entries.sort_by_key(|entry| (entry.path().is_file(), entry.file_name()));

    for entry in entries {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".aura" || name == "target" || name == "node_modules" {
            continue;
        }

        let indent = "  ".repeat(depth);
        if entry_path.is_dir() {
            let expanded = ide.expanded_dirs.contains(&entry_path);
            let label = format!("{indent}{} {name}", if expanded { "v" } else { ">" });
            column =
                column.push(button(text(label)).on_press(Message::ToggleDirectory(entry_path.clone())));
            if expanded {
                column = column.push(view_directory(ide, &entry_path, depth + 1));
            }
        } else {
            column = column.push(
                button(text(format!("{indent}  {name}"))).on_press(Message::OpenFile(entry_path)),
            );
        }
    }

    column
}

async fn load_source_control(workspace: PathBuf) -> Result<SourceControlSnapshot, String> {
    let repo = Repository::new(workspace);
    let status = match repo.status().await {
        Ok(status) => status,
        Err(AuraError::NotRepository(_)) => return Ok(SourceControlSnapshot::default()),
        Err(error) => return Err(error.to_string()),
    };

    let diffs = repo
        .diff_head_to_worktree()
        .await
        .map_err(|error| error.to_string())?;
    let log = repo.log().await.map_err(|error| error.to_string())?;

    Ok(SourceControlSnapshot {
        branch: status.branch,
        head: status.head,
        staged: status
            .staged
            .into_iter()
            .map(|entry| ChangeItem {
                path: entry.path,
                kind: entry.kind,
            })
            .collect(),
        unstaged: status
            .unstaged
            .into_iter()
            .map(|entry| ChangeItem {
                path: entry.path,
                kind: entry.kind,
            })
            .collect(),
        untracked: status.untracked,
        diffs,
        log,
    })
}

async fn run_source_action(workspace: PathBuf, action: SourceAction) -> String {
    let repo = Repository::new(workspace);
    let result = match action {
        SourceAction::Commit(message) => async move {
            repo.add_all().await?;
            let summary = repo.commit_with_summary(message).await?;
            Ok::<String, AuraError>(format_commit_outcome(&summary))
        }
        .await,
        SourceAction::Push => repo.push(None, None).await.map(|summary| format_push_summary(&summary)),
        SourceAction::Pull => repo.pull(None, None).await.map(|summary| format_pull_summary(&summary)),
    };

    match result {
        Ok(output) => ensure_trailing_newline(output),
        Err(error) => format!("Aura action failed: {error}\n"),
    }
}

async fn run_shell_command(workspace: PathBuf, command: String) -> String {
    let mut process = if cfg!(target_os = "windows") {
        let mut command_process = TokioCommand::new("powershell");
        command_process.arg("-NoProfile").arg("-Command").arg(command);
        command_process
    } else {
        let mut command_process = TokioCommand::new("sh");
        command_process.arg("-lc").arg(command);
        command_process
    };

    let output = process
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(output) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&output.stdout));
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            ensure_trailing_newline(combined)
        }
        Err(error) => format!("Command failed: {error}\n"),
    }
}

fn syntax_token_for_path(path: Option<&Path>) -> String {
    let Some(path) = path else {
        return String::from("txt");
    };

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("txt");
    if SYNTAX_SET.find_syntax_by_extension(extension).is_some() {
        extension.to_string()
    } else {
        String::from("txt")
    }
}

fn short_oid(oid: &str) -> &str {
    &oid[..oid.len().min(7)]
}

fn change_marker(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "A",
        ChangeKind::Modified => "M",
        ChangeKind::Deleted => "D",
    }
}

fn render_diff_text(value: &str) -> String {
    let trimmed = value.trim_end_matches('\n');
    if trimmed.is_empty() {
        String::from(" ")
    } else {
        trimmed.replace('\t', "    ")
    }
}

fn ensure_trailing_newline(mut value: String) -> String {
    if !value.ends_with('\n') {
        value.push('\n');
    }
    value
}

fn format_commit_outcome(summary: &CommitOutcome) -> String {
    let branch = summary
        .branch
        .clone()
        .unwrap_or_else(|| String::from("detached HEAD"));
    format!(
        "[{} {}] {}\n{}",
        branch,
        short_oid(&summary.oid),
        summary.message.replace('\n', " "),
        format_change_stats(&summary.stats)
    )
}

fn format_push_summary(summary: &PushSummary) -> String {
    format!(
        "Pushed branch `{}` to `{}` at {} ({})",
        summary.branch,
        summary.remote,
        summary.oid,
        summary.target.display()
    )
}

fn format_pull_summary(summary: &PullSummary) -> String {
    match summary.status {
        PullStatus::AlreadyUpToDate => String::from("Already up to date."),
        PullStatus::FastForward => {
            let from = summary
                .previous_oid
                .as_deref()
                .map(short_oid)
                .unwrap_or("empty");
            let stats = summary
                .stats
                .as_ref()
                .map(format_change_stats)
                .unwrap_or_default();
            if stats.is_empty() {
                format!(
                    "From {} ({})\nUpdating {}..{}\nFast-forward",
                    summary.remote,
                    summary.target.display(),
                    from,
                    short_oid(&summary.oid)
                )
            } else {
                format!(
                    "From {} ({})\nUpdating {}..{}\nFast-forward\n{}",
                    summary.remote,
                    summary.target.display(),
                    from,
                    short_oid(&summary.oid),
                    stats
                )
            }
        }
    }
}

fn format_change_stats(stats: &aura_control::ChangeStats) -> String {
    if stats.files.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    for file in &stats.files {
        let total = file.insertions + file.deletions;
        let marks = format!(
            "{}{}",
            "+".repeat(file.insertions.min(20)),
            "-".repeat(file.deletions.min(20))
        );
        if marks.is_empty() {
            lines.push(format!(" {} | {}", file.path.display(), total));
        } else {
            lines.push(format!(" {} | {} {}", file.path.display(), total, marks));
        }
    }

    let mut summary = vec![format!(" {} file(s) changed", stats.files_changed)];
    if stats.insertions > 0 {
        summary.push(format!("{} insertion(s)(+)", stats.insertions));
    }
    if stats.deletions > 0 {
        summary.push(format!("{} deletion(s)(-)", stats.deletions));
    }
    lines.push(summary.join(", "));
    lines.join("\n")
}

pub fn main() -> iced::Result {
    AuraIde::run(Settings {
        default_font: Font::MONOSPACE,
        ..Settings::default()
    })
}
