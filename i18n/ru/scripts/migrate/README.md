# Инструментарий публичной миграции

Универсальные помощники для публикации внутренней истории и трекера в публичный
репозиторий (ADR-0016). URL репозиториев, токены и приватные строки передаются во
время выполнения, поэтому этот каталог не содержит внутреннего хоста, пути к
токену или слага.

## Предварительные требования

- `git-filter-repo` (`pipx install git-filter-repo`)
- токен Gitea в `GITEA_TOKEN` и токен GitHub в `GITHUB_TOKEN`

## Экспорт манифеста

```sh
scripts/migrate/gitea_manifest.py \
    --base https://<gitea-host>/api/v1/repos/<owner>/<repo> \
    --output target/migrate/gitea.json

scripts/migrate/github_manifest.py \
    --repo <owner>/<repo> \
    --output target/migrate/github.json
```

Манифест фиксирует метки, вехи, issue, pull request, комментарии, авторов и связи
blocked-by. Временные метки записываются, но никогда не сравниваются.

## Сравнение манифестов

```sh
scripts/migrate/diff_manifests.py \
    --expected target/migrate/expected.json \
    --actual target/migrate/github.json
```

`expected.json` — это отрендеренный публичный манифест: текст Gitea с английским
переводом сверху и русским оригиналом в `<details>`, а не сырой экспорт Gitea.
Любое расхождение завершает работу с ненулевым кодом.

## Сканирование секретов

```sh
scripts/migrate/scan_secrets.py \
    --repo /tmp/public-clone \
    --manifest target/migrate/gitea.json \
    --forbidden-file ~/private/forbidden.txt
```

Встроенные шаблоны покрывают приватные ключи, токены GitHub и облачные токены,
заголовки `Authorization`, учётные данные в URL, `/home/<user>/` и имена файлов с
секретами. Строки, специфичные для проекта (внутренний хост, слаг, путь к токену),
лежат в приватном файле вне репозитория; по одному regex на строку. `--allow-file`
подавляет известные безопасные совпадения. Сканирование читает каждый исторический
blob, каждое сообщение коммита и каждый текст трекера.

## Переписать историю

```sh
scripts/migrate/rewrite_history.sh \
    --source /path/to/internal/repo \
    --output /tmp/public-clone \
    --replacements ~/private/replacements.txt
```

Он клонирует источник, удаляет `.gitea/`, `scripts/export-public.sh`,
`scripts/issue-view.sh`, `target/` и `embed/`, применяет файл замен, переименовывает
`master` в `main` и печатает результат. Даты авторов и коммиттеров сохраняются. Он
никогда не делает push.

`replacements.example.txt` показывает формат `git-filter-repo --replace-text`;
скопируйте его за пределы репозитория и заполните реальными значениями.

## Отрендерить тексты архива

```sh
scripts/migrate/render_tracker.py \
    --manifest target/migrate/gitea.json \
    --translations target/migrate/translations.json \
    --output target/migrate/expected.json \
    --replacements ~/private/replacements.txt \
    --commit-map /tmp/public-clone/.git/filter-repo/commit-map \
    --repo /tmp/public-clone \
    --public-repo <owner>/<repo> \
    --import-date 2026-11-08
```

Английский идёт сверху, русский оригинал — в `<details>`, плюс заметка о
происхождении с исходными датами и, для pull request, ссылка на переписанный
merge-коммит. Одни и те же приватные правила замен применяются к обоим языкам.

## Импорт трекера

```sh
scripts/migrate/import_tracker.py \
    --repo <owner>/<repo> \
    --manifest target/migrate/expected.json \
    --token-file ~/private/github-token
```

Элементы создаются строго в порядке возрастания номеров. Скрипт прерывается, если
следующий номер GitHub не совпадает с манифестом, так что посторонний объект не
может незаметно сдвинуть архив. `--mode synthetic` поднимает пустой репозиторий и
открывает одноразовые pull request (репетиция); `--mode refs` использует переписанные
`head_sha`/`base_sha` из манифеста и требует уже запушенную историю. Архивные pull
request закрываются после создания, а их временные ветки удаляются. `--prune-labels`
удаляет целевые метки, которых нет в манифесте (например, набор по умолчанию GitHub).
Используйте `--dry-run`, чтобы напечатать план.

Поток репетиции для пустого sandbox-репозитория:

```sh
scripts/migrate/import_tracker.py --repo <owner>/sandbox --manifest gitea.json \
    --token-file ~/private/github-token --mode synthetic
scripts/migrate/github_manifest.py --repo <owner>/sandbox --output github.json --token-file ~/private/github-token
scripts/migrate/diff_manifests.py --expected gitea.json --actual github.json
```

## Порядок гейтов

1. `rewrite_history.sh`
2. `scan_secrets.py` по клону и манифестам
3. `scripts/check.sh` внутри клона
4. импорт трекера
5. `diff_manifests.py`
