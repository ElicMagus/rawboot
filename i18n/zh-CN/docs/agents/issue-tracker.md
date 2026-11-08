# Issue 跟踪器：Gitea

本仓库的 issue 位于自托管 Gitea：https://github.com/ElicMagus/rawboot

## 访问

- 基础 URL：https://github.com/ElicMagus/rawboot
- API 基础：https://github.com/ElicMagus/rawboot/api/v1
- Owner/repo：`ElicMagus/rawboot-internal`
- 认证：Personal Access Token 存放在仓库之外，位于
  `~/.config/external-secret-file/gitea-token`（chmod 600）。以
  `Authorization: token $(cat ~/.config/external-secret-file/gitea-token)` 传递。
  **切勿打印、echo 或提交该 token。**
- TLS：有效的 Let's Encrypt 通配符证书（`*.nikmaos.ru`）；`curl` 和 `git`
  无需 `-k` / `GIT_SSL_NO_VERIFY` 即可工作。

### 网络 git 操作（push / fetch / ls-remote）

任何通过 HTTPS 针对 Gitea 的网络 `git` 命令都会提示输入用户名和
作为密码的 token。没有 helper 时，无终端命令会失败并报
`could not read Username for 'https://github.com/ElicMagus/rawboot'`。对每个网络操作使用 `GIT_ASKPASS`
helper，而不是把 token 嵌入 remote
URL，这样 token 永远不会落入 `.git/config` 或 shell 历史：

```sh
GIT_ASKPASS=~/.config/external-secret-file/git-askpass.sh git ls-remote origin
GIT_ASKPASS=~/.config/external-secret-file/git-askpass.sh git push -u origin <branch>
```

## 工作流

Issue -> 分支 -> pull request -> 合并进 `master`。

- 分支名：feature 用 `feat/<N>-<slug>`，fix 用 `fix/<N>-<slug>`
  （`<N>` = Gitea 中的 issue 编号）。
- 打开一个引用该 issue 的 Gitea pull request（例如 `Closes #<N>`），然后
  合并进 `master`。

## 操作

所有调用使用 header `Authorization: token <token>`。下面的 `{B}` 是
`https://github.com/ElicMagus/rawboot/api/v1/repos/ElicMagus/rawboot-internal`。

- **发布 issue**（创建）：
  `POST {B}/issues`，JSON `{"title": ..., "body": ..., "labels": [<label ids>]}`
  -> 响应中的 `number` 即 issue 编号。
- **获取 issue**：`GET {B}/issues/<N>`
- **为 agent 获取它**：`scripts/issue-view.sh <N>` 在一次调用中打印标题、状态、标签、正文、
  `## Agent Brief` 以及依赖/阻塞链接。
- **获取 brief**：`GET {B}/issues/<N>/comments`。由 triage 写入的 `## Agent Brief` 评论是
  权威的：它携带正文可能省略的范围、验收标准和范围外事项。
  实现前先阅读它。
- **列出 issue**：`GET {B}/issues?state=all&limit=50`
- **评论**：`POST {B}/issues/<N>/comments`，`{"body": ...}`
- **应用标签**：`POST {B}/issues/<N>/labels`，`{"labels": [<label ids>]}`
- **关闭 / 重新打开**：`PATCH {B}/issues/<N>`，`{"state":"closed"}` / `{"state":"open"}`

### 依赖（原生 blocked-by 链接）

优先使用 Gitea 的原生依赖链接，而不是文本化的 `Blocked by:` 约定。
issue `<N>` 的 `dependencies` 是**阻塞**它的那些 issue；它的 `blocks`
是它阻塞的那些 issue。两者都用 `GET` 读取，用 `POST` /
`DELETE` 编辑，正文是一个 `IssueMeta`：`{"owner":"ElicMagus","repo":"rawboot-internal","index":<other issue>}`。
**Gitea 要求显式提供 `owner` 和 `repo`。**

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

当 `/dependencies` 中的每一项都报告
`state == "closed"` 时，ticket `<N>` 即**解除阻塞**。

### 标签 id

`POST {B}/issues/<N>/labels` 和 `PUT {B}/issues/<N>/labels`（替换整个
集合）接受数字 id，而不是名称。保持此表与
`GET {B}/labels` 同步：

| Label           | id  |
| --------------- | --- |
| bugfix          | 61  |
| feature         | 62  |
| needs-info      | 57  |
| needs-triage    | 56  |
| ready-for-agent | 58  |
| ready-for-human | 59  |
| wontfix         | 60  |

## 备注

- Issue 正文和评论不得包含非 BMP 字符（emoji）：Gitea/MySQL schema
  以 `utf8mb3` 存储文本，会以 HTTP 500 拒绝此类输入。将跟踪器文本
  限制为纯 BMP（ASCII/Cyrillic）。
- 此文件随时可编辑；仅在更改跟踪器类型或重新开始时才
  重跑 `/setup-matt-pocock-skills`。
