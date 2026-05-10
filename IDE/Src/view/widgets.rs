// src/view/widgets.rs
use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Element, Length};
use std::path::PathBuf;

// Импортируем свои типы из корня проекта
use crate::MyIde;
use crate::Message;

/// Строит весь интерфейс IDE
pub fn build_widget(ide: &MyIde) -> Element<Message> {


    // Сохранение дока
    let save_button = button("Save")
        .on_press(Message::SaveFiles);

    // 1. Кнопка "Новый файл"
    let new_file_button = button("New File")
        .on_press(Message::CreateNewFile(
            PathBuf::from("db/"),  
            "untitled.txt".to_string(),
        ));

    // 2. Информация о файле
    let file_info_str = match &ide.state.current_file {
        Some(path) => format!("Current File: {}", path.display()),
        None => "No file opened".to_string(),
    };
    let file_info_text = text(&file_info_str).size(14);

    // 3. Сайдбар (левая панель)
    let side_bar = container(
        column![
            new_file_button,
            save_button,
            file_info_text,
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

    // 6. Сейв
    
}