use iced::widget::{
    button, column, container, row, scrollable, text, text_editor, text_input, Column,
};
use iced::{executor, Application, Command, Element, Length, Settings, Theme};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

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
    SourceControlLoaded(SourceControlSnapshot),
    Commit,
    Push,
    Pull,
    AuraActionFinished(String),
}

#[derive(Debug, Clone, Default)]
struct SourceControlSnapshot {
    status: String,
    log: String,
    diff: String,
}

struct AuraIde {
    workspace: PathBuf,
    active_file: Option<PathBuf>,
    editor: text_editor::Content,
    sidebar_tab: SidebarTab,
    expanded_dirs: BTreeSet<PathBuf>,
    new_file_name: String,
    terminal_input: String,
    terminal_output: String,
    source_control: SourceControlSnapshot,
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
            sidebar_tab: SidebarTab::Explorer,
            expanded_dirs: BTreeSet::new(),
            new_file_name: String::new(),
            terminal_input: String::new(),
            terminal_output: String::from("Aura IDE terminal\n"),
            source_control: SourceControlSnapshot::default(),
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
            Message::EditorAction(action) => self.editor.perform(action),
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
                }
            }
            Message::OpenFile(path) => {
                if let Ok(contents) = fs::read_to_string(&path) {
                    self.editor = text_editor::Content::with_text(&contents);
                    self.active_file = Some(path);
                }
            }
            Message::SaveFile => {
                if let Some(path) = &self.active_file {
                    let _ = fs::write(path, self.editor.text());
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
            Message::SourceControlLoaded(snapshot) => {
                self.source_control = snapshot;
                self.source_busy = false;
            }
            Message::Commit => {
                let message = self.commit_message.trim().to_string();
                if message.is_empty() {
                    return Command::none();
                }
                self.source_busy = true;
                self.commit_message.clear();
                return Command::perform(
                    run_aura_sequence(
                        self.workspace.clone(),
                        vec![
                            vec!["add".into(), "-A".into()],
                            vec!["commit".into(), "-m".into(), message],
                        ],
                    ),
                    Message::AuraActionFinished,
                );
            }
            Message::Push => {
                self.source_busy = true;
                return Command::perform(
                    run_aura_sequence(self.workspace.clone(), vec![vec!["push".into()]]),
                    Message::AuraActionFinished,
                );
            }
            Message::Pull => {
                self.source_busy = true;
                return Command::perform(
                    run_aura_sequence(self.workspace.clone(), vec![vec!["pull".into()]]),
                    Message::AuraActionFinished,
                );
            }
            Message::AuraActionFinished(output) => {
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
        .width(Length::Fixed(320.0))
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
                    button("Save").on_press(Message::SaveFile),
                ]
                .spacing(12),
                text_editor(&self.editor)
                    .on_action(Message::EditorAction)
                    .height(Length::Fill),
                container(column![
                    text("Terminal").size(12),
                    scrollable(text(&self.terminal_output).size(13)).height(Length::Fixed(150.0)),
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
        let busy = if self.source_busy {
            "Syncing..."
        } else {
            "Ready"
        };
        column![
            row![
                text(busy).size(12),
                button("Refresh").on_press(Message::RefreshSourceControl),
            ]
            .spacing(8),
            text_input("Commit message", &self.commit_message)
                .on_input(Message::CommitMessageChanged)
                .on_submit(Message::Commit),
            row![
                button("Commit").on_press(Message::Commit),
                button("Pull").on_press(Message::Pull),
                button("Push").on_press(Message::Push),
            ]
            .spacing(8),
            text("Status").size(12),
            scrollable(text(&self.source_control.status).size(12)).height(Length::Fixed(150.0)),
            text("Diff").size(12),
            scrollable(text(&self.source_control.diff).size(12)).height(Length::Fixed(180.0)),
            text("Commit Graph").size(12),
            scrollable(text(format_commit_graph(&self.source_control.log)).size(12))
                .height(Length::Fill),
        ]
        .spacing(8)
        .into()
    }
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
            column = column
                .push(button(text(label)).on_press(Message::ToggleDirectory(entry_path.clone())));
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

async fn load_source_control(workspace: PathBuf) -> SourceControlSnapshot {
    SourceControlSnapshot {
        status: run_aura(workspace.clone(), vec!["status".into()]).await,
        log: run_aura(workspace.clone(), vec!["log".into()]).await,
        diff: run_aura(workspace, vec!["diff".into()]).await,
    }
}

async fn run_aura_sequence(workspace: PathBuf, commands: Vec<Vec<String>>) -> String {
    let mut output = String::new();
    for args in commands {
        output.push_str(&run_aura(workspace.clone(), args).await);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

async fn run_aura(workspace: PathBuf, args: Vec<String>) -> String {
    let aura = aura_binary();
    let output = ProcessCommand::new(aura)
        .arg("-C")
        .arg(&workspace)
        .args(args)
        .output();

    match output {
        Ok(output) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&output.stdout));
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            combined
        }
        Err(error) => format!("Aura command failed: {error}\n"),
    }
}

async fn run_shell_command(workspace: PathBuf, command: String) -> String {
    let output = if cfg!(target_os = "windows") {
        ProcessCommand::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(command)
            .current_dir(workspace)
            .output()
    } else {
        ProcessCommand::new("sh")
            .arg("-lc")
            .arg(command)
            .current_dir(workspace)
            .output()
    };

    match output {
        Ok(output) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&output.stdout));
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            combined
        }
        Err(error) => format!("Command failed: {error}\n"),
    }
}

fn aura_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("AURA_BIN") {
        return PathBuf::from(path);
    }

    let debug = PathBuf::from("../target/debug/aura");
    if debug.exists() {
        return debug;
    }

    let release = PathBuf::from("../target/release/aura");
    if release.exists() {
        return release;
    }

    PathBuf::from("aura")
}

fn format_commit_graph(log: &str) -> String {
    if log.trim().is_empty() || log.contains("No commits yet") {
        return String::from("No commits yet\n");
    }

    log.lines()
        .map(|line| format!("* {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn main() -> iced::Result {
    AuraIde::run(Settings {
        default_font: iced::Font::MONOSPACE,
        ..Settings::default()
    })
}
