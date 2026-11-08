# 发布公开仓库

内部 Gitea 仓库是权威来源。公开 GitHub 仓库
（`https://github.com/ElicMagus/rawboot`）是历史被重写的镜像
（ADR-0016）。旧的单 squash 提交导出已退役。

## 发布内容

- `master` 直至 `v0.1.0` 标签（207 次提交）以及该附注标签本身。
- 除 `.gitea/`、`scripts/export-public.sh`、
  `scripts/issue-view.sh`、`target/` 和 `embed/` 之外的每个受跟踪文件。
- 代码、环境与决策，包括 `AGENTS.md`、
  `CODING_STANDARDS.md`、`docs/agents/` 和 `research/`。
- 该标签之后的提交（本 runbook、迁移工具、`i18n/`
  树及后续修复）作为普通公开提交叠加在其上。
- 未合并的内部分支不发布；先合并，再运行手动
  发布。

## 重写

历史使用 `git-filter-repo` 重写：

- 从每个修订中剥离上面列出的内部路径；
- 保留作者和提交者日期以及真实姓名和电子邮件地址；
- 用中性文本替换机密和密码、外部 token 文件路径、形如
  `/home/<user>` 的主目录路径以及对内部 Gitea 的链接；
- 保留俄语的提交主题和正文。

标签消息、打标签者和日期保留；只有标签目标改变。

## 验证关卡

四项均为阻断性；在任何推送之前于干净克隆上运行它们。

1. **清单差异。** `scripts/migrate/` 从 Gitea 和
   GitHub 导出清单，然后对比编号、类型、状态、正文、标签、里程碑、评论
   数量和正文，以及 blocked-by 边。要求零差异。
2. **机密扫描。** 对每个历史 blob
   （`git rev-list --objects --all`）和每个跟踪器文本 grep token 与密码
   模式、token 文件路径、`/home/<user>` 以及内部 Gitea 链接。
3. **构建。** `scripts/check.sh` 在重写后的克隆中通过。
4. **抽查。** 手动对照 Gitea 比较十个 issue。

## 一次性迁移

1. 冻结 Gitea 编号：在步骤 5 结束前不要创建任何 issue 或 pull request，否则
   GitHub 编号将永久偏移。
2. 取 bundle：一个 `master` 加标签的内部 `git bundle`，以及
   当前公开 `main` 的 bundle。
3. 在临时目录中试运行：重写，运行所有关卡，统计提交数，
   确认标签目标。
4. 在一个一次性仓库中演练跟踪器：按升序创建全部 56 个对象
   并对比清单。
5. 用 `--force` 推送重写后的 `main`，并推送 `v0.1.0` 标签。
6. 创建标签和里程碑，然后按升序创建对象 #1–56。
   Pull request 作为已关闭记录归档，带一个临时 base
   分支和一个链接真实 merge 提交的正文。导入 72 条评论、
   blocked-by 边和双语 EN/RU 文本（英文在上，俄文
   原文在 `<details>` 中）。
7. 删除临时 base 分支并重新运行关卡。

## 发布后续修复

1. 将修复合并到内部 `master`，若为发布版本则打标签。
2. 用相同的过滤器重写一个全新克隆并运行关卡。
3. 推送重写后的 `main` 和任何新标签。新提交出现在 `v0.1.0` 之后。

## 跟踪器

GitHub issue 和 pull request 是只读归档；不要在那里创建新的。
Gitea 仍是唯一的活动跟踪器。

## 注意事项

- 强制推送不可逆：步骤 2 的 bundle 是唯一退路。
- GitHub 将 issue、评论和 pull request 的时间戳设为迁移日期；
  每个正文记录原始日期。
- 经所有者决定，内部邮件域名仍会在提交元数据中可见。
