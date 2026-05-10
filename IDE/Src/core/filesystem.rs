use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

pub fn CreateFileInDir(dir: &Path, filename: &str) -> Result<PathBuf, std::io::Error> {
    fs::create_dir_all(dir)?; 
    let file_path = dir.join(filename);
    fs::File::create(&file_path)?; 
    Ok(file_path)
}

pub fn SaveFiles(path: &Path, content: &str) -> Result<(), std::io::Error> {
    fs::write(path, content)
}

pub fn ListFilesInDir(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut entries = Vec::new();
    if !dir.exists() {
        let _ = fs::create_dir_all(dir);
        return Ok(entries);
    }
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            entries.push(PathBuf::from(entry.file_name()));
        }
    }
    entries.sort_by(|a, b| {
        let a_is_dir = dir.join(a).is_dir();
        let b_is_dir = dir.join(b).is_dir();
        b_is_dir.cmp(&a_is_dir).then(a.cmp(b))
    });
    Ok(entries)
}

pub fn execute_command(input: &str) -> String {
    let output = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args(["-Command", &format!("chcp 65001 >$null; {}", input)])
            .output()
    } else {
        Command::new("sh").arg("-c").arg(input).output()
    };

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            if out.status.success() { 
                if stdout.is_empty() { "Success".to_string() } else { stdout }
            } else { 
                format!("Error: {}", stderr) 
            }
        }
        Err(e) => format!("Failed to execute: {}", e),
    }
}