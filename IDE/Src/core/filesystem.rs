use std::path::{Path, PathBuf};


pub fn CreateNewFile(path: &Path) -> Result<PathBuf, std::io::Error> {
    // Логика создания нового файла
    std::fs::File::create(path)?; // Создаем файл на диске, если он уже существует, будет ошибка
    Ok(path.to_path_buf()) // Возвращаем путь к новому файлу
}

pub fn CreateFileInDir(dir: &Path, filename: &str) -> Result<PathBuf, std::io::Error> {
    let file_path = dir.join(filename);

    // Получаем родительскую папку
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?; // Создаст недостающие папки
    }
    
    std::fs::create_dir_all(dir)?; // Убедимся, что директория существует
    let file_path = dir.join(filename);
    std::fs::File::create(&file_path)?; // Создаем файл
    Ok(file_path) // Возвращаем путь к новому файлу
}

pub fn SaveFiles(path: &Path, content: &str) -> Result<(), std::io::Error> {
    std::fs::write(path, content)
}

pub fn ListFilesInDir(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        entries.push(PathBuf::from(file_name));
    }
    Ok(entries)
}

pub fn is_dir(path: &Path) -> bool {
    path.is_dir() 
}