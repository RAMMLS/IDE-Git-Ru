use serde::Serialize;
use std::path::Path;
use anyhow::Result;

pub struct Repository {
    // mock fields
}

#[derive(Serialize)]
pub struct RepoInfo {
    pub current_branch: String,
    pub commit_count: usize,
}

#[derive(Serialize)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub time: u64,
}

#[derive(Serialize)]
pub struct FileInfo {
    pub path: String,
    pub is_dir: bool,
}

impl Repository {
    pub fn open<P: AsRef<Path>>(_path: P) -> Result<Self> {
        Ok(Repository {})
    }

    pub fn get_info(&self) -> Result<RepoInfo> {
        Ok(RepoInfo {
            current_branch: "main".to_string(),
            commit_count: 0,
        })
    }

    pub fn get_log(&self, _branch: &str) -> Result<Vec<CommitInfo>> {
        Ok(vec![])
    }

    pub fn get_files(&self, _commit: &str, _path: &str) -> Result<Vec<FileInfo>> {
        Ok(vec![])
    }

    pub fn get_diff(&self, _commit: &str) -> Result<String> {
        Ok("mock diff".to_string())
    }

    pub fn add(&mut self, _paths: &[String]) -> Result<()> {
        Ok(())
    }

    pub fn commit(&mut self, _message: &str) -> Result<String> {
        Ok("mockhash".to_string())
    }

    pub fn checkout(&mut self, _target: &str) -> Result<()> {
        Ok(())
    }

    pub fn create_branch(&mut self, _name: &str) -> Result<()> {
        Ok(())
    }
}
