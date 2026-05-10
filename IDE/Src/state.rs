use std::path::PathBuf;

pub struct EditorState {
    pub content: String, // Текст в редакторе
    pub is_dirty: bool, // Флаг, показывающий, были ли изменения
    pub current_file: Option<PathBuf>, // Путь к открытому файлу (если есть)
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            is_dirty: false,
            current_file: None,
        }
    }
}