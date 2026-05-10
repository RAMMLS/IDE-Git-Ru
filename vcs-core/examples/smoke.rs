use std::path::PathBuf;

use aura_control::{ChangeKind, DiffLine, Repository};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = demo_root()?;
    let repo = Repository::init(&root).await?;

    println!("Aura smoke repository: {}", root.display());

    let hello = root.join("hello.txt");
    tokio::fs::write(&hello, b"hello from aura\n").await?;
    repo.add(["hello.txt"]).await?;
    let first_commit = repo.commit("initial demo commit").await?;
    println!("Created initial commit: {first_commit}");

    if let Err(error) = repo.branch("feature").await {
        println!("Branch feature already exists or could not be created: {error}");
    }

    repo.checkout("feature").await?;
    println!("Checked out branch: feature");

    tokio::fs::write(&hello, b"hello from aura\nfeature branch line\n").await?;
    let status_before_add = repo.status().await?;
    print_status("Status before add()", &status_before_add);

    let diff_before_add = repo.diff().await?;
    print_diff("Diff before add()", &diff_before_add);

    repo.add(["hello.txt"]).await?;
    let second_commit = repo.commit("update hello on feature").await?;
    println!("Created feature commit: {second_commit}");

    let log = repo.log().await?;
    println!("Log entries: {}", log.len());
    for entry in &log {
        println!(
            "- {} | {} | {}",
            entry.oid,
            entry.timestamp,
            entry.message.replace('\n', " ")
        );
    }

    repo.checkout("main").await?;
    println!("Checked out branch: main");

    let final_status = repo.status().await?;
    print_status("Final status on main", &final_status);
    println!(
        "Current file contents on main:\n{}",
        tokio::fs::read_to_string(&hello).await?
    );

    println!("\nSmoke run complete.");
    Ok(())
}

fn demo_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut args = std::env::args_os();
    let _program = args.next();

    let root = if let Some(path) = args.next() {
        PathBuf::from(path)
    } else {
        std::env::current_dir()?.join("aura-demo-repo")
    };

    Ok(root)
}

fn print_status(title: &str, status: &aura_control::RepositoryStatus) {
    println!("\n{title}");
    println!("branch: {:?}", status.branch);
    println!("head: {:?}", status.head);
    print_changes("staged", &status.staged);
    print_changes("unstaged", &status.unstaged);

    if status.untracked.is_empty() {
        println!("untracked: none");
    } else {
        println!("untracked:");
        for path in &status.untracked {
            println!("  - {}", path.display());
        }
    }
}

fn print_changes(label: &str, entries: &[aura_control::StatusEntry]) {
    if entries.is_empty() {
        println!("{label}: none");
        return;
    }

    println!("{label}:");
    for entry in entries {
        let kind = match entry.kind {
            ChangeKind::Added => "added",
            ChangeKind::Modified => "modified",
            ChangeKind::Deleted => "deleted",
        };
        println!("  - {kind}: {}", entry.path.display());
    }
}

fn print_diff(title: &str, diffs: &[aura_control::FileDiff]) {
    println!("\n{title}");
    if diffs.is_empty() {
        println!("no diff");
        return;
    }

    for diff in diffs {
        println!("file: {}", diff.path.display());
        for line in &diff.lines {
            match line {
                DiffLine::Context(value) => print!(" {}", value),
                DiffLine::Addition(value) => print!("+{}", value),
                DiffLine::Deletion(value) => print!("-{}", value),
            }
        }
        if !diff
            .lines
            .last()
            .is_some_and(|line| matches!(line, DiffLine::Context(value) | DiffLine::Addition(value) | DiffLine::Deletion(value) if value.ends_with('\n')))
        {
            println!();
        }
    }
}
