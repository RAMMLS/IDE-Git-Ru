# Aura Testing

`TESTING.md` содержит ручные сценарии проверки основных возможностей Aura и ожидаемые результаты.

## Подготовка

Открой PowerShell в корне проекта:

```powershell
cd C:\Users\RAMMLS\Desktop\IDE-Git-Ru
```

Если `aura` ещё не установлена глобально:

```powershell
.\install-aura.bat
```

Либо используй бинарник напрямую из `vcs-core`:

```powershell
C:\Users\RAMMLS\Desktop\IDE-Git-Ru\vcs-core\target\debug\aura.exe help
```

## Сценарий 1. Инициализация Репозитория

```powershell
mkdir test-local -Force
aura init .\test-local
Get-ChildItem .\test-local -Force
```

Ожидается:

- вывод `Initialized Aura repository at ...`;
- в `test-local` существует каталог `.aura`.

## Сценарий 2. Первый Файл И Первый Коммит

```powershell
Set-Content .\test-local\main.c ""
aura -C .\test-local status
aura -C .\test-local add -A
aura -C .\test-local commit -m "initial commit"
aura -C .\test-local log
```

Ожидается:

- до `add` файл виден как untracked;
- после `add -A` файл виден в staged;
- `commit` проходит без panic даже для пустого файла;
- после `commit` ветка `main` указывает на новый коммит.

## Сценарий 3. Изменение Файла И Diff

```powershell
Set-Content .\test-local\main.c "int main() { return 0; }"
aura -C .\test-local diff
aura -C .\test-local status
aura -C .\test-local add .\main.c
aura -C .\test-local commit -m "update main.c"
```

Ожидается:

- `diff` показывает добавление строки;
- `status` до `add` показывает изменение в `unstaged`;
- `commit` печатает git-подобный summary со статистикой.

## Сценарий 4. Создание И Переключение Веток

```powershell
aura -C .\test-local branch feature
aura -C .\test-local branch
aura -C .\test-local switch feature
aura -C .\test-local branch
```

Ожидается:

- ветка `feature` создаётся успешно;
- в списке веток текущая ветка отмечается `*`;
- после `switch feature` текущей веткой становится `feature`.

## Сценарий 5. Файл Только Для Ветки

```powershell
Set-Content .\test-local\feature.c "void feature() {}"
aura -C .\test-local add -A
aura -C .\test-local commit -m "add feature.c"
Get-ChildItem .\test-local -File

aura -C .\test-local switch main
Get-ChildItem .\test-local -File
```

Ожидается:

- в ветке `feature` файл `feature.c` присутствует;
- после `switch main` файл `feature.c` исчезает, если в `main` его нет.

## Сценарий 6. Возврат Обратно В Ветку

```powershell
aura -C .\test-local switch feature
Get-ChildItem .\test-local -File
```

Ожидается:

- `feature.c` снова появляется в рабочем дереве.

## Сценарий 7. Защита От Перезаписи Untracked Файла

```powershell
aura -C .\test-local switch main
Set-Content .\test-local\feature.c "local only"
aura -C .\test-local status
aura -C .\test-local switch feature
```

Ожидается:

- `status` показывает `feature.c` только в `untracked`;
- `switch feature` завершается ошибкой, что untracked файл будет перезаписан.

## Сценарий 8. Удаление Файла

```powershell
aura -C .\test-local switch feature
Remove-Item .\test-local\feature.c
aura -C .\test-local status
aura -C .\test-local add -A
aura -C .\test-local commit -m "remove feature.c"
```

Ожидается:

- до `add -A` файл виден как deleted в `unstaged`;
- после `add -A` удаление попадает в `staged`;
- `commit` фиксирует удаление.

## Сценарий 9. Remote И Push

```powershell
mkdir test-remote -Force
aura init .\test-remote
aura -C .\test-local remote add origin .\test-remote
aura -C .\test-local push origin main
```

Ожидается:

- `remote add origin ...` сохраняет remote;
- `push origin main` проходит успешно;
- при отсутствии remote выводится сообщение, что remote не настроен в текущем репозитории.

## Сценарий 10. Pull В Новый Репозиторий

```powershell
mkdir test-clone -Force
aura init .\test-clone
aura -C .\test-clone remote add origin ..\test-remote
aura -C .\test-clone pull origin main
aura -C .\test-clone status
Get-ChildItem .\test-clone -File
```

Ожидается:

- `pull origin main` подтягивает файлы из remote;
- `status` после pull показывает clean state;
- рабочее дерево соответствует содержимому ветки `main`.

## Сценарий 11. Pull Уже Актуальной Ветки

```powershell
aura -C .\test-clone pull origin main
```

Ожидается:

- вывод `Already up to date.`

## Сценарий 12. Команды Из Вложенной Папки

```powershell
mkdir .\test-local\src -Force
Set-Location .\test-local\src
Set-Content .\nested.c "void nested() {}"
aura add nested.c
aura status
Set-Location ..\..
```

Ожидается:

- Aura находит `.aura` в родительской директории;
- относительный путь `nested.c` корректно добавляется в индекс.

## Очистка

```powershell
Remove-Item .\test-local -Recurse -Force
Remove-Item .\test-remote -Recurse -Force
Remove-Item .\test-clone -Recurse -Force
```

## Автоматический Прогон

Для автоматического прогона смотри `test-aura.ps1` в корне проекта.
