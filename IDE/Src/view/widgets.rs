use iced::widget::{button, column, container, row, text, text_editor, text_input, Column, scrollable, pane_grid};
use iced::{Element, Length, Color, Border, Alignment, theme, Font};
use std::path::Path;
use std::fs;
use crate::{MyIde, Message, TERMINAL_SCROLL_ID};
use crate::state::Pane;

const LINE_HEIGHT: f32 = 24.0;

pub fn build_widget(ide: &MyIde) -> Element<'_, Message> {
    pane_grid::PaneGrid::new(&ide.state.panes, |_, pane, _| {
        match pane {
            Pane::Explorer => {
                let file_tree = view_directory(ide, &ide.state.db_dir, 0);
                pane_grid::Content::new(
                    container(column![
                        text("EXPLORER").size(12).style(Color::from_rgb(0.5, 0.5, 0.5)),
                        text_input("file.txt", &ide.state.new_file_name)
                            .on_input(Message::SetNewFileName)
                            .on_submit(Message::CreateNewFile(ide.state.new_file_name.clone()))
                            .style(theme::TextInput::Custom(Box::new(InputStyle)))
                            .padding(5),
                        row![
                            button(text("New").size(12)).on_press(Message::CreateNewFile(ide.state.new_file_name.clone())),
                            button(text("Refresh").size(12)).on_press(Message::RefreshFileTree),
                        ].spacing(5),
                        scrollable(file_tree),
                    ].spacing(10).padding(10))
                    .width(Length::Fill).height(Length::Fill)
                    .style(theme::Container::Custom(Box::new(SidebarStyle)))
                )
            }
            Pane::Editor => {
                pane_grid::Content::new(
                    container(row![
                        container(draw_line_numbers(ide)).padding([5, 10, 0, 5]),
                        container(text_editor(&ide.content).on_action(Message::EditorAction).font(Font::MONOSPACE).style(theme::TextEditor::Custom(Box::new(EditorStyle))))
                            .width(Length::Fill)
                    ]).style(theme::Container::Custom(Box::new(EditorBgStyle)))
                )
            }
            Pane::Terminal => {
                let path_str = ide.state.current_dir.to_string_lossy().replace(r"\\?\", "");
                pane_grid::Content::new(
                    container(column![
                        text("TERMINAL").size(11).style(Color::from_rgb(0.5, 0.5, 0.5)),
                        scrollable(text(&ide.state.terminal_output).size(13).font(Font::MONOSPACE).width(Length::Fill))
                            .height(Length::Fill)
                            .id(scrollable::Id::new(TERMINAL_SCROLL_ID)),
                        
                        row![
                            text(format!("{}>", path_str))
                                .size(13)
                                .font(Font::MONOSPACE)
                                .style(Color::WHITE), 
                            
                            text_input("", &ide.state.terminal_input)
                                .on_input(Message::UpdateTerminalInput)
                                .on_submit(Message::ExecuteTerminalCommand)
                                .style(theme::TextInput::Custom(Box::new(TerminalInputStyle))),
                        ].spacing(5).align_items(Alignment::Center),
                    ].spacing(5)).padding(10).style(theme::Container::Custom(Box::new(TerminalStyle)))
                )
            }
        }
    })
    .width(Length::Fill).height(Length::Fill).on_resize(10, Message::Resized).into()
}

fn view_directory<'a>(ide: &'a MyIde, path: &Path, depth: u16) -> Column<'a, Message> {
    let mut col = column![].spacing(2);
    let indent = depth as f32 * 12.0;
    if let Ok(entries) = fs::read_dir(path) {
        let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| (e.path().is_file(), e.file_name()));
        for entry in entries {
            let p = entry.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if p.is_dir() || !name.contains('.') {
                let is_expanded = ide.state.expanded_dirs.contains(&p);
                col = col.push(button(text(format!("{} {}", if is_expanded { "▼" } else { "▶" }, name)).size(14))
                        .on_press(Message::ToggleDir(p.clone())).style(theme::Button::Text).padding([2, 0, 2, indent as u16]));
                if is_expanded && p.is_dir() { col = col.push(view_directory(ide, &p, depth + 1)); }
            } else {
                col = col.push(container(text(format!("  {}", name)).size(14)).padding([2, 0, 2, indent as u16]));
            }
        }
    }
    col
}

fn draw_line_numbers(ide: &MyIde) -> Column<'_, Message> {
    let mut col = column![].spacing(0).align_items(Alignment::End);
    for i in 1..=ide.content.line_count() {
        col = col.push(container(text(i.to_string()).size(14).style(Color::from_rgb(0.4, 0.4, 0.4))).height(LINE_HEIGHT));
    }
    col
}

// --- СТИЛИ ---

struct TerminalInputStyle;
impl text_input::StyleSheet for TerminalInputStyle {
    type Style = theme::Theme;
    fn active(&self, _: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: iced::Background::Color(Color::TRANSPARENT),
            border: Border { color: Color::TRANSPARENT, width: 0.0, radius: 0.0.into() },
            icon_color: Color::WHITE,
        }
    }
    fn focused(&self, s: &Self::Style) -> text_input::Appearance { self.active(s) }
    fn value_color(&self, _: &Self::Style) -> Color { Color::WHITE }
    fn placeholder_color(&self, _: &Self::Style) -> Color { Color::from_rgb(0.3, 0.3, 0.3) }
    fn selection_color(&self, _: &Self::Style) -> Color { Color::from_rgb(0.2, 0.4, 0.6) }
    fn disabled(&self, s: &Self::Style) -> text_input::Appearance { self.active(s) }
    fn disabled_color(&self, _: &Self::Style) -> Color { Color::WHITE }
}

struct InputStyle;
impl text_input::StyleSheet for InputStyle {
    type Style = theme::Theme;
    fn active(&self, _: &Self::Style) -> text_input::Appearance {
        text_input::Appearance {
            background: iced::Background::Color(Color::from_rgb(0.12, 0.12, 0.12)),
            border: Border { color: Color::from_rgb(0.3, 0.3, 0.3), width: 1.0, radius: 2.0.into() },
            icon_color: Color::WHITE,
        }
    }
    fn focused(&self, s: &Self::Style) -> text_input::Appearance {
        let mut a = self.active(s); a.border.color = Color::from_rgb(0.4, 0.4, 0.7); a
    }
    fn value_color(&self, _: &Self::Style) -> Color { Color::WHITE }
    fn placeholder_color(&self, _: &Self::Style) -> Color { Color::from_rgb(0.3, 0.3, 0.3) }
    fn selection_color(&self, _: &Self::Style) -> Color { Color::from_rgb(0.2, 0.4, 0.6) }
    fn disabled(&self, s: &Self::Style) -> text_input::Appearance { self.active(s) }
    fn disabled_color(&self, _: &Self::Style) -> Color { Color::WHITE }
}

struct SidebarStyle;
impl container::StyleSheet for SidebarStyle {
    type Style = theme::Theme;
    fn appearance(&self, _: &Self::Style) -> container::Appearance {
        container::Appearance { background: Some(iced::Background::Color(Color::from_rgb(0.1, 0.1, 0.11))), ..Default::default() }
    }
}

struct EditorBgStyle;
impl container::StyleSheet for EditorBgStyle {
    type Style = theme::Theme;
    fn appearance(&self, _: &Self::Style) -> container::Appearance {
        container::Appearance { background: Some(iced::Background::Color(Color::from_rgb(0.07, 0.07, 0.07))), ..Default::default() }
    }
}

struct TerminalStyle;
impl container::StyleSheet for TerminalStyle {
    type Style = theme::Theme;
    fn appearance(&self, _: &Self::Style) -> container::Appearance {
        container::Appearance { background: Some(iced::Background::Color(Color::from_rgb(0.02, 0.02, 0.02))), ..Default::default() }
    }
}

struct EditorStyle;
impl text_editor::StyleSheet for EditorStyle {
    type Style = theme::Theme;
    fn active(&self, _: &Self::Style) -> text_editor::Appearance {
        text_editor::Appearance { 
            background: iced::Background::Color(Color::from_rgb(0.07, 0.07, 0.07)), 
            border: Border { color: Color::TRANSPARENT, width: 0.0, radius: 0.0.into() } 
        }
    }
    fn focused(&self, s: &Self::Style) -> text_editor::Appearance { self.active(s) }
    fn placeholder_color(&self, _: &Self::Style) -> Color { Color::from_rgb(0.4, 0.4, 0.4) }
    fn value_color(&self, _: &Self::Style) -> Color { Color::WHITE }
    fn selection_color(&self, _: &Self::Style) -> Color { Color::from_rgb(0.2, 0.4, 0.7) }
    fn disabled(&self, s: &Self::Style) -> text_editor::Appearance { self.active(s) }
    fn disabled_color(&self, _: &Self::Style) -> Color { Color::BLACK }
}