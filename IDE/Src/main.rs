use iced::widget::{text_editor, scrollable, pane_grid};
use iced::{Application, Command, Settings, Subscription, keyboard, Theme, executor};
use std::path::PathBuf;
use std::fs;
use std::process::Command as SysCommand;

mod view;
mod state;

use crate::view::widgets;
use crate::state::AppState;

pub const TERMINAL_SCROLL_ID: &str = "terminal_scroll";

pub struct MyIde {
    pub content: text_editor::Content,
    pub state: AppState,
}

#[derive(Debug, Clone)]
pub enum Message {
    EditorAction(text_editor::Action),
    SetNewFileName(String),
    CreateNewFile(String),
    RefreshFileTree,
    ToggleDir(PathBuf),
    UpdateTerminalInput(String),
    ExecuteTerminalCommand,
    DoTab,
    Resized(pane_grid::ResizeEvent),
}

impl Application for MyIde {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                content: text_editor::Content::new(),
                state: AppState::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String { String::from("IDE-Git-Ru") }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::EditorAction(action) => { self.content.perform(action); }
            Message::DoTab => {
                for _ in 0..4 { self.content.perform(text_editor::Action::Edit(text_editor::Edit::Insert(' '))); }
            }
            Message::UpdateTerminalInput(s) => self.state.terminal_input = s,
            Message::ExecuteTerminalCommand => {
                if !self.state.terminal_input.is_empty() {
                    let cmd_full = self.state.terminal_input.trim().to_string();
                    let folder_name = self.state.current_dir
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "root".into());

                    self.state.terminal_output.push_str(&format!("{}> {}\n", folder_name, cmd_full));

                    let parts: Vec<&str> = cmd_full.split_whitespace().collect();
                    if parts.is_empty() { return Command::none(); }

                    match parts[0] {
                        "cd" => {
                            if parts.len() > 1 {
                                let new_path = if parts[1] == ".." {
                                    self.state.current_dir.parent().unwrap_or(&self.state.current_dir).to_path_buf()
                                } else {
                                    self.state.current_dir.join(parts[1])
                                };

                                if new_path.exists() && new_path.is_dir() {
                                    let canonical = fs::canonicalize(new_path).unwrap_or(self.state.current_dir.clone());
                                    let clean_path = canonical.to_string_lossy().replace(r"\\?\", "");
                                    self.state.current_dir = PathBuf::from(clean_path);
                                } else {
                                    self.state.terminal_output.push_str("Error: Directory not found.\n");
                                }
                            }
                        }
                        _ => {
                            let output = SysCommand::new("powershell")
                                .arg("-NoProfile")
                                .arg("-Command")
                                .arg(format!("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {} | Out-String", cmd_full))
                                .current_dir(&self.state.current_dir)
                                .output();

                            if let Ok(out) = output {
                                self.state.terminal_output.push_str(&String::from_utf8_lossy(&out.stdout));
                                self.state.terminal_output.push_str(&String::from_utf8_lossy(&out.stderr));
                            }
                        }
                    }
                    self.state.terminal_input.clear();
                    return scrollable::snap_to(scrollable::Id::new(TERMINAL_SCROLL_ID), scrollable::RelativeOffset::END);
                }
            }
            Message::CreateNewFile(name) => {
                if !name.is_empty() {
                    let full_path = self.state.db_dir.join(&name);
                    if !name.contains('.') { let _ = fs::create_dir_all(&full_path); }
                    else {
                        if let Some(parent) = full_path.parent() { let _ = fs::create_dir_all(parent); }
                        let _ = fs::File::create(&full_path);
                    }
                    self.state.new_file_name.clear();
                }
            }
            Message::SetNewFileName(s) => self.state.new_file_name = s,
            Message::ToggleDir(p) => {
                if self.state.expanded_dirs.contains(&p) { self.state.expanded_dirs.remove(&p); }
                else { self.state.expanded_dirs.insert(p); }
            }
            Message::RefreshFileTree => {}
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.state.panes.resize(split, ratio);
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<Message> { widgets::build_widget(self) }
    fn theme(&self) -> Self::Theme { Theme::Dark }
    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, _| {
            if let keyboard::Key::Named(keyboard::key::Named::Tab) = key { Some(Message::DoTab) } else { None }
        })
    }
}

pub fn main() -> iced::Result {
    MyIde::run(Settings { default_font: iced::Font::MONOSPACE, ..Settings::default() })
}