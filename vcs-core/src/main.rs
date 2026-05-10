use std::path::PathBuf;

use aura_control::{refs, ChangeKind, DiffLine, Repository};

#[derive(Debug)]
struct Cli {
    repo: Option<PathBuf>,
    command: Command,
}

#[derive(Debug)]
enum Command {
    Init { path: Option<PathBuf> },
    Add { paths: Vec<PathBuf> },
    Commit { message: String },
    Status,
    Log,
    Checkout { branch: String },
    Branch { name: Option<String> },
    Diff,
    Help,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = match parse_cli() {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("{message}\n");
            print_usage();
            std::process::exit(2);
        }
    };

    if matches!(cli.command, Command::Help) {
        print_usage();
        return Ok(());
    }

    match cli.command {
        Command::Init { path } => {
            let root = path.or(cli.repo).unwrap_or(std::env::current_dir()?);
            Repository::init(&root).await?;
            println!("Initialized Aura repository at {}", root.display());
        }
        Command::Add { paths } => {
            let repo = open_repo(cli.repo).await?;
            let written = repo.add(paths).await?;
            println!("Indexed {} file object(s)", written.len());
        }
        Command::Commit { message } => {
            let repo = open_repo(cli.repo).await?;
            let oid = repo.commit(message).await?;
            println!("Created commit {oid}");
        }
        Command::Status => {
            let repo = open_repo(cli.repo).await?;
            let status = repo.status().await?;
            print_status("Repository status", &status);
        }
        Command::Log => {
            let repo = open_repo(cli.repo).await?;
            let log = repo.log().await?;
            if log.is_empty() {
                println!("No commits yet");
            } else {
                for entry in log {
                    println!(
                        "{} {} {}",
                        entry.oid,
                        entry.timestamp,
                        entry.message.replace('\n', " ")
                    );
                }
            }
        }
        Command::Checkout { branch } => {
            let repo = open_repo(cli.repo).await?;
            repo.checkout(&branch).await?;
            println!("Checked out branch `{branch}`");
        }
        Command::Branch { name } => {
            let repo = open_repo(cli.repo).await?;
            if let Some(name) = name {
                repo.branch(&name).await?;
                println!("Created branch `{name}`");
            } else {
                let current = refs::current_branch(repo.filesystem(), &repo.path).await?;
                let branches = refs::list_branches(repo.filesystem(), &repo.path).await?;
                if branches.is_empty() {
                    println!("No branches found");
                } else {
                    for branch in branches {
                        let marker = if current.as_deref() == Some(branch.as_str()) {
                            "*"
                        } else {
                            " "
                        };
                        println!("{marker} {branch}");
                    }
                }
            }
        }
        Command::Diff => {
            let repo = open_repo(cli.repo).await?;
            let diffs = repo.diff().await?;
            print_diff("Repository diff", &diffs);
        }
        Command::Help => {}
    }

    Ok(())
}

async fn open_repo(repo: Option<PathBuf>) -> Result<Repository, Box<dyn std::error::Error>> {
    let root = repo.unwrap_or(std::env::current_dir()?);
    Ok(Repository::new(root))
}

fn parse_cli() -> Result<Cli, String> {
    let mut args = std::env::args_os().skip(1).peekable();
    let mut repo = None::<PathBuf>;

    while let Some(arg) = args.peek() {
        if arg == "--repo" {
            args.next();
            let value = args
                .next()
                .ok_or_else(|| "Флаг `--repo` требует путь после себя".to_string())?;
            repo = Some(PathBuf::from(value));
            continue;
        }
        break;
    }

    let command = match args.next() {
        None => Command::Help,
        Some(command) => match command.to_string_lossy().as_ref() {
            "init" => Command::Init {
                path: args.next().map(PathBuf::from),
            },
            "add" => {
                let paths = args.map(PathBuf::from).collect::<Vec<_>>();
                if paths.is_empty() {
                    return Err("Команда `add` требует хотя бы один путь".to_string());
                }
                Command::Add { paths }
            }
            "commit" => {
                let parts = args
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                if parts.is_empty() {
                    return Err("Команда `commit` требует сообщение".to_string());
                }
                Command::Commit {
                    message: parts.join(" "),
                }
            }
            "status" => Command::Status,
            "log" => Command::Log,
            "checkout" => {
                let branch = args
                    .next()
                    .ok_or_else(|| "Команда `checkout` требует имя ветки".to_string())?;
                Command::Checkout {
                    branch: branch.to_string_lossy().into_owned(),
                }
            }
            "branch" => Command::Branch {
                name: args.next().map(|arg| arg.to_string_lossy().into_owned()),
            },
            "diff" => Command::Diff,
            "help" | "--help" | "-h" => Command::Help,
            other => {
                return Err(format!("Неизвестная команда `{other}`"));
            }
        },
    };

    Ok(Cli { repo, command })
}

fn print_usage() {
    println!("Aura Control CLI");
    println!();
    println!("Usage:");
    println!("  cargo run -- [--repo PATH] <command> [args]");
    println!();
    println!("Commands:");
    println!("  init [PATH]          Initialize a new Aura repository");
    println!("  add <PATH>...        Add files or directories to the index");
    println!("  commit <MESSAGE>     Create a commit from the current index");
    println!("  status               Show staged, unstaged and untracked files");
    println!("  log                  Show commit history");
    println!("  branch [NAME]        List branches or create a new branch");
    println!("  checkout <NAME>      Switch to an existing branch");
    println!("  diff                 Show worktree diff against the index");
    println!("  help                 Show this help");
    println!();
    println!("Examples:");
    println!("  cargo run -- init ..\\demo-repo");
    println!("  cargo run -- --repo ..\\demo-repo add hello.txt src");
    println!("  cargo run -- --repo ..\\demo-repo commit initial snapshot");
    println!("  cargo run -- --repo ..\\demo-repo status");
    println!("  cargo run -- --repo ..\\demo-repo branch feature");
    println!("  cargo run -- --repo ..\\demo-repo checkout feature");
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
