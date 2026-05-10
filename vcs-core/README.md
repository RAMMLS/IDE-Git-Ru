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

## Установка CLI

Чтобы вызывать систему контроля версий как обычную команду `aura`, установи бинарник в пользовательский cargo bin:

Предпочтительный вариант из корня репозитория:

```powershell
.\install-aura.bat
```

Скрипт автоматически:

- проверяет наличие `cargo`;
- устанавливает Rust через `winget`, если он отсутствует;
- добавляет `%USERPROFILE%\.cargo\bin` в пользовательский `PATH`;
- устанавливает локальный бинарник `aura`;
- добавляет PowerShell-интеграцию в пользовательские профили;
- создаёт `aura.cmd` shim в уже доступной пользовательской директории из `PATH`, чтобы команда работала и в IDE-терминалах со старым окружением;
- проверяет глобальный запуск `aura help` из произвольного каталога.

Ручной вариант:

```powershell
cd C:\Users\RAMMLS\Desktop\IDE-Git-Ru\vcs-core
cargo install --path . --bin aura --force
```

После этого команда `aura` будет доступна из терминала, если `C:\Users\RAMMLS\.cargo\bin` находится в `PATH`.

Проверка:

```powershell
aura help
```

Если `install-aura.bat` завершился успешно, `aura` должен запускаться из любой папки.
Даже если IDE не перечитала новый `PATH`, новый PowerShell-терминал должен видеть `aura` через профиль и shim.

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
aura help
```

Общий формат:

```powershell
aura [-C PATH | --repo PATH] <command> [args]
```

CLI теперь ближе к привычному сценарию `git`:

- Aura автоматически ищет `.aura` в текущей директории и выше по дереву;
- относительные пути в `aura add` считаются от текущей папки или от пути, указанного через `-C`;
- `aura add -A` индексирует всё рабочее дерево и stage-ит удаления;
- `-C PATH` позволяет работать с репозиторием, не переходя в него;
- `aura commit -m "message"` поддерживается как основной способ задания сообщения;
- `aura commit` теперь печатает git-подобный summary со short SHA и `file changed` / `insertion(+)` / `deletion(-)`;
- `aura push` и `aura push origin <branch>` работают через локальные именованные remotes;
- `aura pull`, `aura pull origin` и `aura pull origin <branch>` подтягивают изменения из remote в текущую ветку через fast-forward;
- `aura switch <branch>` доступна как более понятный алиас для `aura checkout <branch>`.

Поддержанные команды:

- `init [PATH]`
- `add <PATH>...`
- `add -A`
- `commit [-m] <MESSAGE>`
- `status`
- `log`
- `branch [NAME]`
- `remote add <NAME> <PATH>`
- `push [REMOTE] [BRANCH]`
  при одном аргументе, совпадающем с локальной веткой, команда трактуется как `push origin <BRANCH>`
- `pull [REMOTE] [BRANCH]`
  по умолчанию тянет из `origin` в текущую ветку; сейчас поддержан fast-forward сценарий без auto-merge
- `checkout <NAME>`
- `switch <NAME>`
- `diff`

Примеры:

```powershell
aura init ..\demo-repo
aura init ..\demo-remote
aura -C ..\demo-repo remote add origin ..\demo-remote

Set-Content ..\demo-repo\hello.txt "hello from aura"
New-Item -ItemType Directory ..\demo-repo\src -Force
Set-Content ..\demo-repo\src\lib.txt "demo source"

aura -C ..\demo-repo add -A
aura -C ..\demo-repo status
aura -C ..\demo-repo commit -m initial snapshot
aura -C ..\demo-repo push
aura -C ..\demo-repo pull
aura -C ..\demo-repo branch feature
aura -C ..\demo-repo switch feature
aura -C ..\demo-repo log
```

Пример запуска из вложенной папки без `--repo`:

```powershell
cd ..\demo-repo\src

Set-Content .\nested.txt "demo source from nested dir"
aura add nested.txt
aura status
aura commit -m "add nested file"
```

Локальный remote и push:

```powershell
aura remote add origin ..\demo-remote
aura push
aura push main
aura push origin main
aura pull
aura pull origin
aura pull origin main
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
- `commit` и fast-forward `pull` выводят line-based статистику, близкую к `git diff --stat`;
- для работы CLI пути в `add` интерпретируются относительно текущей папки или `-C PATH`;
- текущий `push` синхронизирует только локальные Aura-репозитории по пути, без сети и без HTTP-сервера;
- `pull` пока не делает merge-коммитов и завершится ошибкой, если локальная и remote ветки разошлись;
- smoke-пример по-прежнему запускается через `cargo run --example smoke`.
