use std::path::PathBuf;

pub struct EditorState {
    pub content: String,
    pub is_dirty: bool,
    pub current_file: Option<PathBuf>,
    pub db_dir: PathBuf,
    pub new_file_name: String,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            is_dirty: false,
            current_file: None,
            db_dir: PathBuf::from("db"),
            new_file_name: String::from("untitled.txt"),
        }
    }
}