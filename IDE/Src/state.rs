use std::path::PathBuf;
use std::collections::HashSet;



pub struct EditorState {
    pub content: String,
    pub is_dirty: bool,
    pub current_file: Option<PathBuf>,
    pub db_dir: PathBuf,
    pub new_file_name: String,
    pub file_tree: Vec<PathBuf>,
    pub expanded_dirs: HashSet<PathBuf>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            is_dirty: false,
            current_file: None,
            db_dir: PathBuf::from("db"),
            new_file_name: String::from("untitled.txt"),
            file_tree: Vec::new(),
            expanded_dirs: HashSet::new(),
        }
    }
}