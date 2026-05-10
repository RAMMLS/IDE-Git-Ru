mod state;
mod core;
mod view;


use iced::widget::text_editor;
use iced::{executor, Application, Command, Element, Settings, Theme};


// Вот она — наша главная структура (чертеж) приложения
pub struct MyIde {
    content: text_editor::Content, // Содержимое редактора
    state: state::EditorState, // Состояние редактора (например, открытый файл, флаг изменений и т.д.)
}
// Перечисление событий (что может произойти)
#[derive(Debug, Clone)]
pub enum Message {
    // Пользователь что-то сделал в редакторе (нажал клавишу, удалил символ и т.д.)
    EditorAction(text_editor::Action),
    // Команда создать новый файл (можем расширить, добавив параметры для пути и имени файла)
    CreateNewFile(std::path::PathBuf, String),

    // Сохранение
    SaveFiles,

    SetNewFileName(String),
}

// Реализация логики Iced для нашей структуры
impl Application for MyIde {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    // Инициализация (создание) приложения
    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                content: text_editor::Content::new(),
                state: state::EditorState::new(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        if self.state.is_dirty {
            "My IDE - Unsaved Changes".to_string()
        } else {
            "My IDE".to_string()
        }
    }

    // Обновление состояния (реакция на Message)
    fn update(&mut self, message: Message) -> Command<Message> {
    match message {
        Message::EditorAction(action) => {
            self.content.perform(action);
            self.state.is_dirty = true;
        }

        Message::CreateNewFile(dir, filename) => {
            match core::filesystem::CreateFileInDir(&dir, &filename) {
                Ok(path) => {
                    self.state.current_file = Some(path);
                    self.content = text_editor::Content::new();
                    self.state.is_dirty = false;
                }
                Err(e) => {
                    eprintln!("Не удалось создать файл: {}", e);
                }
            }
        }

        Message::SetNewFileName(name) => {
            self.state.new_file_name = name;
        }

        Message::SaveFiles => {
            if let Some(ref path) = self.state.current_file {
                let text = self.content.text();
                // здесь должно быть core::filesystem::save_file (если ты так назвал)
                match core::filesystem::SaveFiles(path, &text) {
                    Ok(()) => {
                        self.state.is_dirty = false;
                    }
                    Err(e) => {
                        eprintln!("Ошибка сохранения: {}", e);
                    }
                }
            } else {
                // Заглушка под "Сохранить как"
            }
        }
    }
    Command::none()
}



    // Отрисовка интерфейса
    fn view(&self) -> Element<Message> {
        view::widgets::build_widget(self) 
    }
}

fn main() -> iced::Result {
    MyIde::run(Settings::default())
}