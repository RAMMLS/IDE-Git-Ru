// src/view/widgets.rs
use iced::widget::{button, column, container, row, text, text_editor, text_input, Space};
use iced::{Element, Length};
use std::path::PathBuf;
use crate::MyIde;
use crate::Message;
use crate::core::filesystem;


/// Строит весь интерфейс IDE
pub fn build_widget(ide: &MyIde) -> Element<Message> {


    // Сохранение дока
    let save_button = button("Save")
        .on_press(Message::SaveFiles);

    // 1. Кнопка "Новый файл"
    let new_file_button = button("New File")
        .on_press(Message::CreateNewFile(
            ide.state.db_dir.clone(),
            ide.state.new_file_name.clone(),
        ));

    // 2. Информация о файле
    let file_info_str = match &ide.state.current_file {
        Some(path) => format!("Current File: {}", path.display()),
        None => "No file opened".to_string(),
    };
    let file_info_text: Element<Message> = text(&file_info_str).size(14).into();

   let mut file_list = column![].spacing(4);
   for file_name in &ide.state.file_tree {
    let full_path = ide.state.db_dir.join(file_name);
    if full_path.is_dir() {
        // Папка
        let folder_btn: Element<Message> = button(text(file_name.display().to_string()).size(14))
            .on_press(Message::ToggleDir(file_name.clone()))
            .into();
        file_list = file_list.push(folder_btn);

        if ide.state.expanded_dirs.contains(file_name) {
            // Если раскрыта показываем содержимое
            if let Ok(children) = filesystem::ListFilesInDir(&full_path) {
                for child_name in children {
                    let child_full = full_path.join(&child_name);
                    let child_text: Element<Message> = text(child_name.display().to_string())
                        .size(13)
                        .into();

                    let intended = container(child_text).padding(20);
                    file_list = file_list.push(intended);
                }
            }
        }
    } else {
        // Файл
        let file_text: Element<Message> = text(file_name.display().to_string())
            .size(14)
            .into();
        file_list = file_list.push(file_text);
    }
   }


    // Новое поле ввода в сайдбар
    let name_input = text_input("File name", &ide.state.new_file_name)
        .on_input(Message::SetNewFileName);

    // Кнопка обновления
    let refresh_button: Element<Message> = button("Refresh")
        .on_press(Message::RefreshFileTree)
        .into();
    // 3. Сайдбар (левая панель)
    let side_bar = container(
        column![
            text("Files in db:"),
            name_input,
            new_file_button,
            save_button,
            file_list,
        ]
        .spacing(10)
    )
    .width(200)
    .padding(10)
    .height(Length::Fill);

    // 4. Редактор кода (правая панель)
    let editor = text_editor(&ide.content)
        .on_action(Message::EditorAction);
    let editor_area = container(editor)
        .padding(5)
        .width(Length::Fill)
        .height(Length::Fill);

    // 5. Собираем финальный ряд
    row![
        side_bar,
        editor_area,
    ]
    .into()   // преобразуем в Element


    
}