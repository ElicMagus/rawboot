# 公共迁移工具

用于将内部历史和跟踪器发布到公共仓库的通用辅助工具（ADR-0016）。仓库 URL、令牌和私有字符串在
运行时传入，因此此目录不包含内部主机、令牌路径或 slug。

## 先决条件

- `git-filter-repo`（`pipx install git-filter-repo`）
- `GITEA_TOKEN` 中的 Gitea 令牌和 `GITHUB_TOKEN` 中的 GitHub 令牌

## 导出一份清单

```sh
scripts/migrate/gitea_manifest.py \
    --base https://<gitea-host>/api/v1/repos/<owner>/<repo> \
    --output target/migrate/gitea.json

scripts/migrate/github_manifest.py \
    --repo <owner>/<repo> \
    --output target/migrate/github.json
```

清单捕获标签、里程碑、issue、pull request、评论、作者和被阻塞链接。时间戳会被记录，但从不比较。

## 比较清单

```sh
scripts/migrate/diff_manifests.py \
    --expected target/migrate/expected.json \
    --actual target/migrate/github.json
```

`expected.json` 是渲染后的公共清单：Gitea 文本，英文翻译在上，俄语原文在 `<details>` 中，而不是
原始 Gitea 导出。任何差异都会以非零状态退出。

## 扫描机密

```sh
scripts/migrate/scan_secrets.py \
    --repo /tmp/public-clone \
    --manifest target/migrate/gitea.json \
    --forbidden-file ~/private/forbidden.txt
```

内置模式覆盖私钥、GitHub 和云令牌、`Authorization` 头、URL 中的凭据、`/home/<user>/` 和机密
文件名。项目特定字符串（内部主机、slug、令牌路径）位于仓库之外的私有文件中；每行一个正则
表达式。`--allow-file` 抑制已知安全的匹配。扫描会读取每个历史 blob、每条提交信息和每个跟踪器
文本。

## 重写历史

```sh
scripts/migrate/rewrite_history.sh \
    --source /path/to/internal/repo \
    --output /tmp/public-clone \
    --replacements ~/private/replacements.txt
```

它克隆源仓库，剥离 `.gitea/`、`scripts/export-public.sh`、`scripts/issue-view.sh`、`target/` 和
`embed/`，应用替换文件，将 `master` 重命名为 `main` 并打印结果。作者和提交者日期会保留。它从不
推送。

`replacements.example.txt` 展示了 `git-filter-repo --replace-text` 格式；将其复制到仓库之外并
填入真实值。

## 渲染归档文本

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

英文在上，俄语原文在 `<details>` 中，再加上带有原始日期的来源说明，对于 pull request 还带有
指向重写后 merge commit 的链接。相同的私有替换规则应用于两种语言。

## 导入跟踪器

```sh
scripts/migrate/import_tracker.py \
    --repo <owner>/<repo> \
    --manifest target/migrate/expected.json \
    --token-file ~/private/github-token
```

条目严格按编号升序创建。如果 GitHub 的下一个编号与清单不匹配，脚本会中止，因此多余的对象无法
悄悄改变归档。`--mode synthetic` 会启动一个空仓库并打开一次性 pull request（排练）；
`--mode refs` 使用清单中重写后的 `head_sha`/`base_sha`，并且需要历史已经推送。归档的 pull request 在
创建后关闭，其临时分支会被删除。`--prune-labels` 删除清单中不存在的目标标签（例如 GitHub 的
默认集合）。使用 `--dry-run` 打印计划。

空沙盒仓库的排练流程：

```sh
scripts/migrate/import_tracker.py --repo <owner>/sandbox --manifest gitea.json \
    --token-file ~/private/github-token --mode synthetic
scripts/migrate/github_manifest.py --repo <owner>/sandbox --output github.json --token-file ~/private/github-token
scripts/migrate/diff_manifests.py --expected gitea.json --actual github.json
```

## 门禁顺序

1. `rewrite_history.sh`
2. 对克隆和清单运行 `scan_secrets.py`
3. 在克隆内运行 `scripts/check.sh`
4. 跟踪器导入
5. `diff_manifests.py`
