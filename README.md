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
- `push`
- `status`
- `log`
- `branch`
- `remote add`
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
- устанавливает локальный бинарник `aura`;
- ставит PowerShell-интеграцию в пользовательские профили;
- создаёт `aura.cmd` shim в уже доступной пользовательской директории из `PATH`, чтобы команда поднималась даже в IDE-терминалах со старым окружением;
- проверяет, что `aura help` запускается из произвольной папки, а не только из каталога проекта.

Ручной вариант установки:

```powershell
cargo install --path .\vcs-core --bin aura --force
```

Покажи встроенную справку CLI:

```powershell
aura help
```

После запуска `install-aura.bat` команда `aura` должна открываться из любого каталога.
Если IDE держит старый `PATH`, установщик дополнительно поднимает PowerShell-функцию и `aura.cmd` shim для новых терминалов.

## CLI

Основная точка входа для ручной проверки находится в `vcs-core/src/main.rs`.

Общий формат:

```powershell
aura [-C PATH | --repo PATH] <command> [args]
```

Особенности поведения:

- `aura` автоматически ищет `.aura` в текущей папке и во всех родительских каталогах;
- `aura status`, `aura log`, `aura diff`, `aura branch`, `aura checkout` и `aura switch` можно вызывать из любой вложенной папки репозитория;
- `aura add` интерпретирует относительные пути от текущей директории, как это делает `git`;
- `aura add -A` индексирует всё рабочее дерево и одновременно stage-ит удаления;
- `-C PATH` и `--repo PATH` позволяют работать с репозиторием, не переходя в него;
- `aura commit -m "message"` поддерживается как основной способ задания сообщения;
- `aura push` и `aura push origin <branch>` пока работают с локальными Aura-репозиториями через именованные remotes.

Доступные команды:

- `init [PATH]` - инициализирует новый Aura-репозиторий;
- `add <PATH>...` - добавляет файлы и директории в индекс;
- `add -A` - индексирует все файлы репозитория и stage-ит удаления;
- `commit [-m] <MESSAGE>` - создаёт коммит из текущего индекса;
- `status` - показывает staged, unstaged и untracked изменения;
- `log` - показывает историю коммитов;
- `branch [NAME]` - выводит список веток или создаёт новую;
- `remote add <NAME> <PATH>` - настраивает локальный remote-репозиторий;
- `push [REMOTE] [BRANCH]` - пушит текущую или указанную ветку в remote;
  если передан один аргумент и это локальная ветка, Aura трактует команду как `push origin <BRANCH>`;
- `checkout <NAME>` - переключает рабочее дерево на ветку;
- `switch <NAME>` - алиас для `checkout`;
- `diff` - показывает diff рабочей директории относительно индекса.

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
aura -C ..\demo-repo branch feature
aura -C ..\demo-repo switch feature
aura -C ..\demo-repo log
aura -C ..\demo-repo diff
```

Работа из вложенной папки репозитория:

```powershell
cd ..\demo-repo\src

Set-Content .\nested.txt "from nested dir"
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
