# IDE-Git-Ru

`IDE-Git-Ru` содержит базовые компоненты системы контроля версий Aura. На текущем этапе основной готовый модуль проекта находится в крейте `vcs-core/` и публикуется как библиотека `aura-control`.

## Что Уже Есть

- асинхронный Rust-крейт `aura-control`;
- скрытая директория репозитория `.aura`;
- объектное хранилище `blob`, `tree`, `commit`;
- ссылки `HEAD` и `refs/heads/*`;
- бинарный индекс `.aura/index` через `bincode`;
- публичный API `Repository { path }`;
- CLI для ручной проверки репозитория;
- smoke-пример для быстрого прогона основного сценария.

## Структура

```text
IDE-Git-Ru/
├── README.md
└── vcs-core/
    ├── Cargo.toml
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

## Возможности Aura

Aura хранит объекты по модели, близкой к Git:

- скрытая папка репозитория: `.aura`;
- формат объекта: `type <len>\0data`;
- идентификатор объекта: SHA-1 от несжатого содержимого с заголовком;
- хранение: `.aura/objects/xx/yyyy...`;
- сжатие: `zlib`;
- ветки: `.aura/refs/heads/<name>`;
- `HEAD`: `ref: refs/heads/<name>`;
- индекс: `.aura/index`.

Поддержанные операции:

- `init`
- `add`
- `commit`
- `status`
- `log`
- `branch`
- `checkout`
- `diff`

## Требования

- Rust toolchain с доступными `cargo` и `rustc`;
- Windows PowerShell или любой терминал, в котором доступен `cargo`.

## Быстрый Старт

Перейди в каталог крейта:

```powershell
cd C:\Users\RAMMLS\Desktop\IDE-Git-Ru\vcs-core
```

Проверь, что проект собирается:

```powershell
cargo check
```

Установи CLI-команду `aura` одной командой:

```powershell
.\install-aura.bat
```

Скрипт:

- проверяет наличие `cargo`;
- при необходимости ставит Rust через `winget`;
- добавляет `%USERPROFILE%\.cargo\bin` в пользовательский `PATH`;
- устанавливает локальный бинарник `aura`.

Ручной вариант установки:

```powershell
cargo install --path .\vcs-core --bin aura --force
```

Покажи встроенную справку CLI:

```powershell
aura help
```

## CLI

Основная точка входа для ручной проверки находится в `vcs-core/src/main.rs`.

Общий формат:

```powershell
aura [--repo PATH] <command> [args]
```

Доступные команды:

- `init [PATH]` - инициализирует новый Aura-репозиторий;
- `add <PATH>...` - добавляет файлы и директории в индекс;
- `commit <MESSAGE>` - создаёт коммит из текущего индекса;
- `status` - показывает staged, unstaged и untracked изменения;
- `log` - показывает историю коммитов;
- `branch [NAME]` - выводит список веток или создаёт новую;
- `checkout <NAME>` - переключает рабочее дерево на ветку;
- `diff` - показывает diff рабочей директории относительно индекса.

Примеры:

```powershell
aura init ..\demo-repo

Set-Content ..\demo-repo\hello.txt "hello from aura"
New-Item -ItemType Directory ..\demo-repo\src -Force
Set-Content ..\demo-repo\src\lib.txt "demo source"

aura --repo ..\demo-repo add hello.txt src
aura --repo ..\demo-repo status
aura --repo ..\demo-repo commit initial snapshot
aura --repo ..\demo-repo branch feature
aura --repo ..\demo-repo checkout feature
aura --repo ..\demo-repo log
aura --repo ..\demo-repo diff
```

## Smoke Проверка

Для быстрого end-to-end прогона добавлен пример `examples/smoke.rs`.

Запуск:

```powershell
cargo run --example smoke
```

Или в отдельную пользовательскую папку:

```powershell
cargo run --example smoke -- C:\Users\RAMMLS\Desktop\IDE-Git-Ru\my-aura-demo
```

Smoke-сценарий автоматически выполняет:

- `init`;
- создание файла;
- `add`;
- `commit`;
- создание ветки `feature`;
- `checkout feature`;
- изменение файла;
- `status`, `diff`, `log`;
- возврат на `main`.

## Основные Модули

- `vcs-core/src/lib.rs` - публичный API, `Repository`, `FileSystem`, ошибки;
- `vcs-core/src/object.rs` - объекты `blob`, `tree`, `commit`, сериализация, SHA-1, zlib;
- `vcs-core/src/refs.rs` - `HEAD`, ветки, чтение и запись ссылок;
- `vcs-core/src/index.rs` - сериализация индекса и операции с записями;
- `vcs-core/src/repository.rs` - высокоуровневые операции репозитория;
- `vcs-core/src/main.rs` - CLI для ручной проверки;
- `vcs-core/examples/smoke.rs` - smoke-сценарий.

## Что Дальше

Ближайшие логичные шаги:

- добавить интеграционные тесты в `vcs-core/tests/`;
- расширить CLI до полноценного пользовательского интерфейса;
- встроить `aura-control` в остальные части проекта IDE;
- добавить поддержку более сложных сценариев индекса и checkout.
