use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use async_recursion::async_recursion;
use aura_control::{
    object::{self, Blob, Commit, ObjectKind, Tree},
    refs, AuraError, ChangeKind, ChangeStats, DiffLine, PullStatus, Repository,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
struct Cli {
    repo: Option<PathBuf>,
    command: Command,
}

#[derive(Debug)]
enum Command {
    Init { path: Option<PathBuf> },
    Add { paths: Vec<PathBuf>, all: bool },
    Commit { message: String },
    Status,
    Log,
    Checkout { branch: String },
    Branch { name: Option<String> },
    RemoteAdd { name: String, target: String },
    Push { remote: Option<String>, branch: Option<String> },
    Pull { remote: Option<String>, branch: Option<String> },
    Diff,
    Help,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransportObject {
    oid: String,
    kind: String,
    body_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransportPack {
    repo_id: String,
    branch: String,
    oid: String,
    objects: Vec<TransportObject>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
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
            let root = path
                .map(|value| resolve_from_base(&current_dir, &value))
                .or_else(|| cli.repo.as_ref().map(|value| resolve_from_base(&current_dir, value)))
                .unwrap_or(current_dir.clone());
            Repository::init(&root).await?;
            println!("Initialized Aura repository at {}", root.display());
        }
        Command::Add { paths, all } => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
            let written = if all {
                repo.add_all().await?
            } else {
                let command_dir = resolve_command_dir(cli.repo.as_ref(), &current_dir);
                let resolved_paths = paths
                    .into_iter()
                    .map(|path| resolve_from_base(&command_dir, &path))
                    .collect::<Vec<_>>();
                repo.add(resolved_paths).await?
            };
            println!("Indexed {} file object(s)", written.len());
        }
        Command::Commit { message } => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
            let summary = repo.commit_with_summary(message).await?;
            print_commit_summary(&summary);
        }
        Command::Status => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
            let status = repo.status().await?;
            print_status("Repository status", &status);
        }
        Command::Log => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
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
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
            match repo.checkout(&branch).await {
                Ok(()) => {}
                Err(AuraError::WorkingTreeNotClean) => {
                    return Err(
                        "Перед `aura checkout`/`aura switch` закоммить или убери staged/unstaged изменения."
                            .into(),
                    );
                }
                Err(AuraError::UntrackedWouldBeOverwritten(path)) => {
                    return Err(format!(
                        "Не могу переключить ветку: untracked файл `{path}` будет перезаписан."
                    )
                    .into());
                }
                Err(error) => return Err(error.into()),
            }
            println!("Checked out branch `{branch}`");
        }
        Command::Branch { name } => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
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
        Command::RemoteAdd { name, target } => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
            let stored_target = normalize_remote_target(cli.repo.as_ref(), &current_dir, &target);
            let stored = repo.add_remote(&name, stored_target).await?;
            println!("Configured remote `{name}` -> {}", stored.display());
        }
        Command::Push { remote, branch } => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
            let (remote, branch) = resolve_push_target(&repo, remote, branch).await?;
            let remote_name = remote.clone().unwrap_or_else(|| "origin".to_string());
            if let Some(target) = refs::read_remote(repo.filesystem(), &repo.path, &remote_name).await? {
                if is_http_remote(&target) {
                    let summary = http_push(&repo, &remote_name, &target, branch.as_deref()).await?;
                    println!(
                        "Pushed branch `{}` to `{}` at {} ({})",
                        summary.branch,
                        summary.remote,
                        summary.oid,
                        summary.target.display()
                    );
                    return Ok(());
                }
            }
            let summary = match repo.push(remote.as_deref(), branch.as_deref()).await {
                Ok(summary) => summary,
                Err(AuraError::RemoteNotFound(name)) => {
                    return Err(format!(
                        "Remote `{name}` не настроен в репозитории `{}`. Сначала выполни `aura remote add {name} <PATH>` из этого репозитория или через `aura -C <PATH> remote add {name} <PATH>`.",
                        repo.path.display()
                    )
                    .into());
                }
                Err(error) => return Err(error.into()),
            };
            println!(
                "Pushed branch `{}` to `{}` at {} ({})",
                summary.branch,
                summary.remote,
                summary.oid,
                summary.target.display()
            );
        }
        Command::Pull { remote, branch } => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
            let remote_name = remote.clone().unwrap_or_else(|| "origin".to_string());
            if let Some(target) = refs::read_remote(repo.filesystem(), &repo.path, &remote_name).await? {
                if is_http_remote(&target) {
                    let summary = http_pull(&repo, &remote_name, &target, branch.as_deref()).await?;
                    print_pull_summary(&summary);
                    return Ok(());
                }
            }
            let summary = match repo.pull(remote.as_deref(), branch.as_deref()).await {
                Ok(summary) => summary,
                Err(AuraError::RemoteNotFound(name)) => {
                    return Err(format!(
                        "Remote `{name}` не найден. Сначала настрой его командой `aura remote add {name} <PATH>`."
                    )
                    .into());
                }
                Err(AuraError::RemoteBranchNotFound { remote, branch }) => {
                    return Err(format!("В remote `{remote}` нет ветки `{branch}`.").into());
                }
                Err(AuraError::WorkingTreeNotClean) => {
                    return Err("Перед `aura pull` закоммить или убери staged/unstaged изменения.".into());
                }
                Err(AuraError::UntrackedWouldBeOverwritten(path)) => {
                    return Err(format!(
                        "Не могу выполнить `aura pull`: untracked файл `{path}` будет перезаписан."
                    )
                    .into());
                }
                Err(AuraError::DivergedBranches { local, remote }) => {
                    return Err(format!(
                        "Локальная ветка `{local}` и `{remote}` разошлись. Авто-merge пока не реализован, поддержан только fast-forward pull."
                    )
                    .into());
                }
                Err(error) => return Err(error.into()),
            };
            print_pull_summary(&summary);
        }
        Command::Diff => {
            let repo = open_repo(cli.repo.as_ref(), &current_dir).await?;
            let diffs = repo.diff().await?;
            print_diff("Repository diff", &diffs);
        }
        Command::Help => {}
    }

    Ok(())
}

async fn open_repo(
    repo: Option<&PathBuf>,
    current_dir: &Path,
) -> Result<Repository, Box<dyn std::error::Error>> {
    let start = resolve_command_dir(repo, current_dir);
    let root = discover_repo_root(&start).ok_or_else(|| {
        format!(
            "Aura-репозиторий не найден в `{}` и его родительских каталогах.\n\
             Запусти `aura init`, перейди в репозиторий или используй `aura -C <PATH> <command>`.",
            start.display()
        )
    })?;
    Ok(Repository::new(root))
}

async fn resolve_push_target(
    repo: &Repository,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<(Option<String>, Option<String>), Box<dyn std::error::Error>> {
    if branch.is_some() || remote.is_none() {
        return Ok((remote, branch));
    }

    let value = remote.expect("remote is checked above");
    if refs::read_remote(repo.filesystem(), &repo.path, &value)
        .await?
        .is_some()
    {
        return Ok((Some(value), None));
    }

    if refs::read_branch(repo.filesystem(), &repo.path, &value)
        .await?
        .is_some()
    {
        return Ok((Some("origin".to_string()), Some(value)));
    }

    Ok((Some(value), None))
}

fn is_http_remote(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://")
}

fn normalize_remote_target(repo: Option<&PathBuf>, current_dir: &Path, target: &str) -> String {
    if is_http_remote(target) {
        return target.trim_end_matches('/').to_string();
    }

    let command_dir = resolve_command_dir(repo, current_dir);
    resolve_from_base(&command_dir, Path::new(target))
        .to_string_lossy()
        .to_string()
}

async fn http_push(
    repo: &Repository,
    remote_name: &str,
    target: &str,
    branch: Option<&str>,
) -> Result<aura_control::PushSummary, Box<dyn std::error::Error>> {
    let branch = match branch {
        Some(branch) => branch.to_string(),
        None => refs::current_branch(repo.filesystem(), &repo.path)
            .await?
            .ok_or("Невозможно выполнить push из detached HEAD.")?,
    };
    let oid = refs::read_branch(repo.filesystem(), &repo.path, &branch)
        .await?
        .ok_or_else(|| format!("В локальном репозитории нет ветки `{branch}`"))?;
    let repo_id = remote_repo_id(target)?;
    let pack = TransportPack {
        repo_id: repo_id.clone(),
        branch: branch.clone(),
        oid: oid.clone(),
        objects: collect_pack(repo, &oid).await?,
    };

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{target}/push"))
        .json(&pack)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("HTTP push failed: {}", response.text().await?).into());
    }

    Ok(aura_control::PushSummary {
        remote: remote_name.to_string(),
        branch,
        oid,
        target: PathBuf::from(target),
    })
}

async fn http_pull(
    repo: &Repository,
    remote_name: &str,
    target: &str,
    branch: Option<&str>,
) -> Result<aura_control::PullSummary, Box<dyn std::error::Error>> {
    let requested_branch = match branch {
        Some(branch) => branch.to_string(),
        None => refs::current_branch(repo.filesystem(), &repo.path)
            .await?
            .ok_or("Невозможно выполнить pull в detached HEAD.")?,
    };

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{target}/export"))
        .query(&[("branch", requested_branch.as_str())])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("HTTP pull failed: {}", response.text().await?).into());
    }

    let pack: TransportPack = response.json().await?;
    let temp_remote_path = std::env::temp_dir().join(format!(
        "aura-http-remote-{}-{}",
        pack.repo_id,
        std::process::id()
    ));
    let temp_remote = Repository::init(&temp_remote_path).await?;
    for object in &pack.objects {
        write_transport_object(&temp_remote, object).await?;
    }
    refs::write_branch(
        temp_remote.filesystem(),
        &temp_remote.path,
        &pack.branch,
        &pack.oid,
    )
    .await?;
    temp_remote.materialize_branch(&pack.branch).await?;

    let temp_remote_name = format!("http-{remote_name}");
    repo.add_remote(&temp_remote_name, &temp_remote_path).await?;
    let summary = repo
        .pull(Some(temp_remote_name.as_str()), Some(pack.branch.as_str()))
        .await?;
    let _ = tokio::fs::remove_file(refs::remote_path(&repo.path, &temp_remote_name)).await;
    let _ = tokio::fs::remove_dir_all(&temp_remote_path).await;

    Ok(aura_control::PullSummary {
        remote: remote_name.to_string(),
        source_branch: summary.source_branch,
        local_branch: summary.local_branch,
        oid: summary.oid,
        previous_oid: summary.previous_oid,
        status: summary.status,
        stats: summary.stats,
        target: PathBuf::from(target),
    })
}

fn remote_repo_id(target: &str) -> Result<String, Box<dyn std::error::Error>> {
    target
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| "Не удалось определить id remote-репозитория из URL".into())
}

async fn collect_pack(
    repo: &Repository,
    oid: &str,
) -> Result<Vec<TransportObject>, Box<dyn std::error::Error>> {
    let mut seen = BTreeSet::new();
    let mut objects = Vec::new();
    collect_commit_objects(repo, oid, &mut seen, &mut objects).await?;
    Ok(objects)
}

#[async_recursion]
async fn collect_commit_objects(
    repo: &Repository,
    oid: &str,
    seen: &mut BTreeSet<String>,
    objects: &mut Vec<TransportObject>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !seen.insert(oid.to_string()) {
        return Ok(());
    }

    let commit = object::read_commit(repo.filesystem(), &repo.path, oid).await?;
    objects.push(TransportObject {
        oid: oid.to_string(),
        kind: "commit".to_string(),
        body_base64: STANDARD.encode(commit.serialize()),
    });
    collect_tree_objects(repo, &commit.tree, seen, objects).await?;
    for parent in &commit.parents {
        collect_commit_objects(repo, parent, seen, objects).await?;
    }
    Ok(())
}

#[async_recursion]
async fn collect_tree_objects(
    repo: &Repository,
    oid: &str,
    seen: &mut BTreeSet<String>,
    objects: &mut Vec<TransportObject>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !seen.insert(oid.to_string()) {
        return Ok(());
    }

    let tree = object::read_tree(repo.filesystem(), &repo.path, oid).await?;
    objects.push(TransportObject {
        oid: oid.to_string(),
        kind: "tree".to_string(),
        body_base64: STANDARD.encode(tree.serialize()?),
    });
    for entry in &tree.entries {
        if entry.mode == "40000" {
            collect_tree_objects(repo, &entry.oid, seen, objects).await?;
        } else {
            collect_blob_object(repo, &entry.oid, seen, objects).await?;
        }
    }
    Ok(())
}

async fn collect_blob_object(
    repo: &Repository,
    oid: &str,
    seen: &mut BTreeSet<String>,
    objects: &mut Vec<TransportObject>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !seen.insert(oid.to_string()) {
        return Ok(());
    }

    let blob = object::read_blob(repo.filesystem(), &repo.path, oid).await?;
    objects.push(TransportObject {
        oid: oid.to_string(),
        kind: "blob".to_string(),
        body_base64: STANDARD.encode(blob.serialize()),
    });
    Ok(())
}

async fn write_transport_object(
    repo: &Repository,
    packed: &TransportObject,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = STANDARD.decode(&packed.body_base64)?;
    let kind = ObjectKind::from_str(&packed.kind)?;

    match kind {
        ObjectKind::Blob => {
            object::write_blob(repo.filesystem(), &repo.path, &Blob::deserialize(&body)).await?;
        }
        ObjectKind::Tree => {
            let tree = Tree::deserialize(&body)?;
            object::write_tree(repo.filesystem(), &repo.path, &tree).await?;
        }
        ObjectKind::Commit => {
            let commit = Commit::deserialize(&body)?;
            object::write_commit(repo.filesystem(), &repo.path, &commit).await?;
        }
    }

    Ok(())
}

fn resolve_command_dir(repo: Option<&PathBuf>, current_dir: &Path) -> PathBuf {
    repo.map(|path| resolve_from_base(current_dir, path))
        .unwrap_or_else(|| current_dir.to_path_buf())
}

fn resolve_from_base(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn discover_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = start;
    loop {
        if current.join(".aura").is_dir() {
            return Some(current.to_path_buf());
        }

        current = current.parent()?;
    }
}

fn parse_cli() -> Result<Cli, String> {
    let mut args = std::env::args_os().skip(1).peekable();
    let mut repo = None::<PathBuf>;

    while let Some(arg) = args.peek() {
        if arg == "--repo" || arg == "-C" {
            args.next();
            let value = args
                .next()
                .ok_or_else(|| "Флаг `--repo`/`-C` требует путь после себя".to_string())?;
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
                let parts = args
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                if parts.is_empty() {
                    return Err("Команда `add` требует путь или флаг `-A`".to_string());
                }
                if parts.len() == 1 && matches!(parts[0].as_str(), "-A" | "--all") {
                    Command::Add {
                        paths: Vec::new(),
                        all: true,
                    }
                } else {
                    if parts.iter().any(|part| matches!(part.as_str(), "-A" | "--all")) {
                        return Err("Используй либо `aura add -A`, либо `aura add <PATH>...`".to_string());
                    }
                    Command::Add {
                        paths: parts.into_iter().map(PathBuf::from).collect::<Vec<_>>(),
                        all: false,
                    }
                }
            }
            "commit" => {
                let parts = args
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                if parts.is_empty() {
                    return Err("Команда `commit` требует сообщение".to_string());
                }
                if matches!(parts.first().map(String::as_str), Some("-m" | "--message")) {
                    if parts.len() == 1 {
                        return Err("Флаг `commit -m` требует сообщение после себя".to_string());
                    }
                    return Ok(Cli {
                        repo,
                        command: Command::Commit {
                            message: parts[1..].join(" "),
                        },
                    });
                }
                if parts
                    .first()
                    .is_some_and(|part| part.starts_with('-') && part != "-m" && part != "--message")
                {
                    return Err(format!("Неизвестный флаг для `commit`: `{}`", parts[0]));
                }
                Command::Commit {
                    message: parts.join(" "),
                }
            }
            "status" => Command::Status,
            "log" => Command::Log,
            "checkout" | "switch" => {
                let branch = args
                    .next()
                    .ok_or_else(|| "Команда `checkout`/`switch` требует имя ветки".to_string())?;
                Command::Checkout {
                    branch: branch.to_string_lossy().into_owned(),
                }
            }
            "branch" => Command::Branch {
                name: args.next().map(|arg| arg.to_string_lossy().into_owned()),
            },
            "remote" => {
                let action = args
                    .next()
                    .ok_or_else(|| "Команда `remote` требует подкоманду, например `add`".to_string())?;
                match action.to_string_lossy().as_ref() {
                    "add" => {
                        let name = args
                            .next()
                            .ok_or_else(|| "Команда `remote add` требует имя remote".to_string())?;
                        let target = args
                            .next()
                            .ok_or_else(|| "Команда `remote add` требует путь к remote-репозиторию".to_string())?;
                        if args.next().is_some() {
                            return Err("Команда `remote add` принимает только имя и путь".to_string());
                        }
                        Command::RemoteAdd {
                            name: name.to_string_lossy().into_owned(),
                            target: target.to_string_lossy().into_owned(),
                        }
                    }
                    other => return Err(format!("Неизвестная подкоманда `remote {other}`")),
                }
            }
            "push" => {
                let parts = args
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                match parts.len() {
                    0 => Command::Push {
                        remote: None,
                        branch: None,
                    },
                    1 => Command::Push {
                        remote: Some(parts[0].clone()),
                        branch: None,
                    },
                    2 => Command::Push {
                        remote: Some(parts[0].clone()),
                        branch: Some(parts[1].clone()),
                    },
                    _ => {
                        return Err(
                            "Команда `push` поддерживает только формы `aura push` и `aura push <REMOTE> <BRANCH>`"
                                .to_string(),
                        )
                    }
                }
            }
            "pull" => {
                let parts = args
                    .map(|arg| arg.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                match parts.len() {
                    0 => Command::Pull {
                        remote: None,
                        branch: None,
                    },
                    1 => Command::Pull {
                        remote: Some(parts[0].clone()),
                        branch: None,
                    },
                    2 => Command::Pull {
                        remote: Some(parts[0].clone()),
                        branch: Some(parts[1].clone()),
                    },
                    _ => {
                        return Err(
                            "Команда `pull` поддерживает только формы `aura pull`, `aura pull <REMOTE>` и `aura pull <REMOTE> <BRANCH>`"
                                .to_string(),
                        )
                    }
                }
            }
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
    println!("  aura [-C PATH | --repo PATH] <command> [args]");
    println!();
    println!("Commands:");
    println!("  init [PATH]              Initialize a new Aura repository");
    println!("  add <PATH>...            Add files or directories to the index");
    println!("  add -A                   Stage the full working tree, including deletions");
    println!("  commit [-m] <MESSAGE>    Create a commit from the current index");
    println!("  status                   Show staged, unstaged and untracked files");
    println!("  log                      Show commit history");
    println!("  branch [NAME]            List branches or create a new branch");
    println!("  checkout <NAME>          Switch to an existing branch");
    println!("  switch <NAME>            Alias for checkout");
    println!("  remote add <N> <TARGET>  Configure a local path or HTTP Aura remote");
    println!("  push [REMOTE] [BRANCH]   Push to a configured remote");
    println!("  pull [REMOTE] [BRANCH]   Pull from a configured remote");
    println!("  diff                     Show worktree diff against the index");
    println!("  help                     Show this help");
    println!();
    println!("Aura automatically searches for `.aura` in the current directory and its parents.");
    println!("Relative paths for `add` are resolved from the current directory or from `-C PATH`.");
    println!();
    println!("Examples:");
    println!("  aura init ..\\demo-repo");
    println!("  aura -C ..\\demo-repo add hello.txt src");
    println!("  aura add -A");
    println!("  aura -C ..\\demo-repo commit -m initial snapshot");
    println!("  aura --repo ..\\demo-repo status");
    println!("  aura branch feature");
    println!("  aura switch feature");
    println!("  aura remote add origin ..\\demo-remote");
    println!("  aura push");
    println!("  aura push main");
    println!("  aura push origin main");
    println!("  aura pull");
    println!("  aura pull origin main");
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

fn print_commit_summary(summary: &aura_control::CommitOutcome) {
    let head = match summary.branch.as_deref() {
        Some(branch) => branch.to_string(),
        None => "detached HEAD".to_string(),
    };
    println!(
        "[{} {}] {}",
        head,
        short_oid(&summary.oid),
        summary.message.replace('\n', " ")
    );
    print_change_stats(&summary.stats);
}

fn print_pull_summary(summary: &aura_control::PullSummary) {
    match summary.status {
        PullStatus::AlreadyUpToDate => {
            println!("Already up to date.");
        }
        PullStatus::FastForward => {
            println!("From {} ({})", summary.remote, summary.target.display());
            let from = summary.previous_oid.as_deref().map(short_oid).unwrap_or("(empty)");
            println!("Updating {}..{}", from, short_oid(&summary.oid));
            println!("Fast-forward");
            if let Some(stats) = &summary.stats {
                print_change_stats(stats);
            }
        }
    }
}

fn print_change_stats(stats: &ChangeStats) {
    for file in &stats.files {
        let total = file.insertions + file.deletions;
        let marks = format_change_marks(file.insertions, file.deletions);
        if marks.is_empty() {
            println!(" {} | {}", file.path.display(), total);
        } else {
            println!(" {} | {} {}", file.path.display(), total, marks);
        }
    }

    let mut parts = vec![format!(
        " {} {} changed",
        stats.files_changed,
        pluralize(stats.files_changed, "file", "files")
    )];
    if stats.insertions > 0 {
        parts.push(format!(
            "{} {}(+)",
            stats.insertions,
            pluralize(stats.insertions, "insertion", "insertions")
        ));
    }
    if stats.deletions > 0 {
        parts.push(format!(
            "{} {}(-)",
            stats.deletions,
            pluralize(stats.deletions, "deletion", "deletions")
        ));
    }
    println!("{}", parts.join(", "));
}

fn short_oid(oid: &str) -> &str {
    let end = oid.len().min(7);
    &oid[..end]
}

fn format_change_marks(insertions: usize, deletions: usize) -> String {
    let insertions = insertions.min(20);
    let deletions = deletions.min(20);
    format!("{}{}", "+".repeat(insertions), "-".repeat(deletions))
}

fn pluralize<'a>(value: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if value == 1 {
        singular
    } else {
        plural
    }
}
