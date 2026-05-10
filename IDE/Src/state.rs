use std::collections::HashSet;
use std::path::PathBuf;
use std::fs;
use iced::widget::pane_grid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Explorer,
    Editor,
    Terminal,
}

pub struct AppState {
    pub expanded_dirs: HashSet<PathBuf>,
    pub db_dir: PathBuf,
    pub current_dir: PathBuf,
    pub new_file_name: String,
    pub terminal_input: String,
    pub terminal_output: String,
    pub panes: pane_grid::State<Pane>,
}

impl Default for AppState {
    fn default() -> Self {
        let db_path = PathBuf::from("db");
        let _ = fs::create_dir_all(&db_path);

        let (mut panes, explorer_id) = pane_grid::State::new(Pane::Explorer);
        
        // Вертикальный сплит: Explorer | Editor
        let (editor_id, split) = panes.split(
            pane_grid::Axis::Vertical,
            explorer_id,
            Pane::Editor,
        ).expect("Split Vertical failed");

        // ИСПОЛЬЗУЕМ resize вместо set_ratio
        panes.resize(split, 0.2);

        // Горизонтальный сплит внутри Editor для Terminal
        let _ = panes.split(
            pane_grid::Axis::Horizontal,
            editor_id,
            Pane::Terminal,
        );

        Self {
            expanded_dirs: HashSet::new(),
            db_dir: db_path.clone(),
            current_dir: fs::canonicalize(&db_path)
                .map(|p| PathBuf::from(p.to_string_lossy().replace(r"\\?\", "")))
                .unwrap_or(db_path),
            new_file_name: String::new(),
            terminal_input: String::new(),
            terminal_output: String::from("Welcome to IDE-Git-Ru\n"),
            panes,
        }
    }
}