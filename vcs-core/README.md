# aura-control

`aura-control` — это базовый Rust-крейт системы контроля версий Aura. Он реализует хранение объектов, индекс, ссылки, высокоуровневые операции репозитория и утилитарный CLI для ручной проверки.

## Возможности

- скрытая директория репозитория `.aura`;
- объекты `blob`, `tree`, `commit`;
- SHA-1 от формата `type <len>\0data`;
- сжатие объектов через `zlib`;
- хранение объектов в `.aura/objects/xx/yyyy...`;
- ссылки в `.aura/refs/heads/<name>`;
- `HEAD` в формате `ref: refs/heads/<name>`;
- индекс `.aura/index` через `bincode`;
- асинхронный API на `tokio`;
- абстракция файловой системы через трейт `FileSystem`;
- встроенный CLI;
- smoke-пример для end-to-end проверки.

## Структура Крейта

```text
vcs-core/
├── Cargo.toml
├── README.md
├── examples/
│   └── smoke.rs
└── src/
    ├── index.rs
    ├── lib.rs
    ├── main.rs
    ├── object.rs
    ├── refs.rs
    └── repository.rs
```

## Основные Модули

- `src/lib.rs` — публичный API, `Repository`, `FileSystem`, типы ошибок и реэкспорт основных сущностей;
- `src/object.rs` — сериализация и работа с `blob`, `tree`, `commit`;
- `src/refs.rs` — чтение и запись `HEAD`, веток и ссылок;
- `src/index.rs` — бинарный индекс и операции над индексными записями;
- `src/repository.rs` — `init`, `add`, `commit`, `log`, `checkout`, `branch`, `status`, `diff`;
- `src/main.rs` — CLI для ручной работы;
- `examples/smoke.rs` — smoke-сценарий.

## Зависимости

Крейт использует:

- `tokio` для async I/O;
- `sha1` для хеширования объектов;
- `flate2` для `zlib`;
- `serde` и `bincode` для сериализации индекса;
- `chrono` для UTC timestamp;
- `thiserror` для типизированных ошибок.

## Сборка

```powershell
cd C:\Users\RAMMLS\Desktop\IDE-Git-Ru\vcs-core
cargo check
```

## Использование Библиотеки

Минимальный пример:

```rust
use aura_control::Repository;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo = Repository::init("demo-repo").await?;

    tokio::fs::write("demo-repo/hello.txt", b"hello\n").await?;
    repo.add(["hello.txt"]).await?;
    let commit = repo.commit("initial commit").await?;

    println!("commit: {commit}");
    Ok(())
}
```

## CLI

Запуск:

```powershell
cargo run -- help
```

Общий формат:

```powershell
cargo run -- [--repo PATH] <command> [args]
```

Поддержанные команды:

- `init [PATH]`
- `add <PATH>...`
- `commit <MESSAGE>`
- `status`
- `log`
- `branch [NAME]`
- `checkout <NAME>`
- `diff`

Примеры:

```powershell
cargo run -- init ..\demo-repo

Set-Content ..\demo-repo\hello.txt "hello from aura"
New-Item -ItemType Directory ..\demo-repo\src -Force
Set-Content ..\demo-repo\src\lib.txt "demo source"

cargo run -- --repo ..\demo-repo add hello.txt src
cargo run -- --repo ..\demo-repo status
cargo run -- --repo ..\demo-repo commit initial snapshot
cargo run -- --repo ..\demo-repo branch feature
cargo run -- --repo ..\demo-repo checkout feature
cargo run -- --repo ..\demo-repo log
```

## Smoke Проверка

Пример `examples/smoke.rs` нужен для быстрого прогона типового сценария репозитория.

Запуск:

```powershell
cargo run --example smoke
```

Либо с явной директорией:

```powershell
cargo run --example smoke -- C:\Users\RAMMLS\Desktop\IDE-Git-Ru\my-aura-demo
```

Smoke-сценарий автоматически:

- инициализирует репозиторий;
- создаёт файл;
- добавляет файл в индекс;
- создаёт коммит;
- создаёт ветку `feature`;
- переключается на неё;
- меняет файл;
- показывает `status`, `diff`, `log`;
- возвращается на `main`.

## Примечания

- права файлов в текущей реализации упрощены до `100644`;
- автор и коммитер фиксированы: `Aura User <aura@local>`;
- время коммита записывается в UTC;
- `diff` сейчас line-based и использует алгоритм Майерса;
- для работы CLI пути в `add` интерпретируются относительно `--repo`.
