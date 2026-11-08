# Трекер задач: Gitea

Задачи для этого репозитория живут в self-hosted Gitea: https://github.com/ElicMagus/rawboot

## Доступ

- Базовый URL: https://github.com/ElicMagus/rawboot
- База API: https://github.com/ElicMagus/rawboot/api/v1
- Владелец/репозиторий: `ElicMagus/rawboot-internal`
- Аутентификация: Personal Access Token хранится ВНЕ репозитория в
  `~/.config/external-secret-file/gitea-token` (chmod 600). Передавайте его как
  `Authorization: token $(cat ~/.config/external-secret-file/gitea-token)`.
  **Никогда не печатайте, не выводите и не коммитьте токен.**
- TLS: действующий wildcard Let's Encrypt (`*.nikmaos.ru`); `curl` и `git`
  работают без `-k` / `GIT_SSL_NO_VERIFY`.

### Сетевые git-операции (push / fetch / ls-remote)

Любая сетевая команда `git` к Gitea по HTTPS запрашивает имя пользователя и
токен в качестве пароля. Без helper'а команды без терминала падают с
`could not read Username for 'https://github.com/ElicMagus/rawboot'`. Используйте helper
`GIT_ASKPASS` для каждой сетевой операции вместо встраивания токена в URL
remote, чтобы токен никогда не попадал в `.git/config` или историю оболочки:

```sh
GIT_ASKPASS=~/.config/external-secret-file/git-askpass.sh git ls-remote origin
GIT_ASKPASS=~/.config/external-secret-file/git-askpass.sh git push -u origin <branch>
```

## Рабочий процесс

Issue -> ветка -> pull request -> мерж в `master`.

- Имя ветки: `feat/<N>-<slug>` для фич, `fix/<N>-<slug>` для исправлений
  (`<N>` — номер issue из Gitea).
- Открыть pull request в Gitea со ссылкой на issue (например, `Closes #<N>`),
  затем влить в `master`.

## Операции

Все вызовы используют заголовок `Authorization: token <token>`. `{B}` ниже —
это `https://github.com/ElicMagus/rawboot/api/v1/repos/ElicMagus/rawboot-internal`.

- **Опубликовать issue** (создать):
  `POST {B}/issues` с JSON `{"title": ..., "body": ..., "labels": [<label ids>]}`
  -> поле `number` в ответе — это номер issue.
- **Получить issue**: `GET {B}/issues/<N>`
- **Получить его для агента**: `scripts/issue-view.sh <N>` печатает заголовок,
  состояние, метки, тело, `## Agent Brief` и ссылки зависимостей/блокировок
  одним вызовом.
- **Получить бриф**: `GET {B}/issues/<N>/comments`. Комментарий `## Agent Brief`,
  написанный triage, авторитетен: он несёт объём работ, критерии приёмки и
  пункты вне объёма, которые тело может опускать. Прочтите его перед
  реализацией.
- **Список issue**: `GET {B}/issues?state=all&limit=50`
- **Комментарий**: `POST {B}/issues/<N>/comments` с `{"body": ...}`
- **Применить метки**: `POST {B}/issues/<N>/labels` с `{"labels": [<label ids>]}`
- **Закрыть / переоткрыть**: `PATCH {B}/issues/<N>` с `{"state":"closed"}` / `{"state":"open"}`

### Зависимости (нативные ссылки blocked-by)

Предпочитайте нативные ссылки зависимостей Gitea текстовой конвенции
`Blocked by:`. `dependencies` issue `<N>` — это issue, которые его
**блокируют**; его `blocks` — это issue, которые блокирует он. И то и другое
читается через `GET` и редактируется через `POST` / `DELETE`, тело — `IssueMeta`:
`{"owner":"ElicMagus","repo":"rawboot-internal","index":<other issue>}`.
**Gitea требует `owner` и `repo` явно.**

```sh
# make #42 depend on #40 (#40 blocks #42)
curl -s -X POST -H "Authorization: token $(cat ~/.config/external-secret-file/gitea-token)" \
  -H "Content-Type: application/json" \
  -d '{"owner":"ElicMagus","repo":"rawboot-internal","index":40}' \
  "{B}/issues/42/dependencies"

# read #42's blockers
curl -s -H "Authorization: token $(cat ~/.config/external-secret-file/gitea-token)" \
  "{B}/issues/42/dependencies"
```

Тикет `<N>` **разблокирован**, когда каждый элемент из `/dependencies` сообщает
`state == "closed"`.

### Id меток

`POST {B}/issues/<N>/labels` и `PUT {B}/issues/<N>/labels` (заменяет весь
набор) принимают числовые id, а не имена. Держите эту таблицу в синхроне с
`GET {B}/labels`:

| Label           | id  |
| --------------- | --- |
| bugfix          | 61  |
| feature         | 62  |
| needs-info      | 57  |
| needs-triage    | 56  |
| ready-for-agent | 58  |
| ready-for-human | 59  |
| wontfix         | 60  |

## Примечания

- Тела issue и комментарии не должны содержать символы за пределами BMP
  (эмодзи): схема Gitea/MySQL хранит текст как `utf8mb3` и отвергает такой
  ввод с HTTP 500. Держите текст трекера в пределах простого BMP
  (ASCII/кириллица).
- Этот файл можно редактировать в любое время; повторно запускайте
  `/setup-matt-pocock-skills` только чтобы сменить вид трекера или начать
  заново.
