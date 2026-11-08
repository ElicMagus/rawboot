# 领域文档

工程 skills 在探索代码库时应如何使用本仓库的领域文档。

## 探索之前，先阅读这些

- 仓库根目录的 **`GLOSSARY.md`**，或者
- 如果存在，仓库根目录的 **`GLOSSARY-MAP.md`**：它为每个 context 指向一个 `GLOSSARY.md`。
- **`docs/adr/`**：阅读涉及你即将工作领域的 ADR。

如果这些文件中的任何一个不存在，**静默继续**。不要指出它们缺失；不要提议预先创建它们。`/domain-modeling` skill 会在术语或决策真正确定时按需创建它们。

## Upstream parity 来源

对于 parity 工作，获取固定版本的 upstream C 树，而不是猜测 URL：
`scripts/fetch-upstream.sh`（默认使用 parity 研究笔记所用 SHA）将其写入
`target/upstream/awboot-<sha>/`。引用 upstream 时使用文件和行号；引用我们的代码时使用符号。

从 `research/parity-checklist.md` 开始：它将每个范围内的 loader 模块映射到其 upstream
源码、本地代码和逐函数差异检查清单。

## 文件结构

单上下文仓库：

```
/
├── GLOSSARY.md
├── docs/adr/
│   ├── 0001-rust-port-and-license.md
│   └── ...
└── src/
```

## 使用术语表的词汇

当你的输出命名一个领域概念时（在 issue 标题、重构提案、假设、测试名称中），使用 `GLOSSARY.md` 中定义的术语。不要漂移到术语表明确避免的同义词。

如果你需要的概念尚未出现在术语表中，这是一个信号：要么你在发明项目不使用的语言（重新考虑），要么存在真正的缺口（为 `/domain-modeling` 记录下来）。

## 标记 ADR 冲突

如果你的输出与现有 ADR 相矛盾，请显式提出，而不是悄悄覆盖：

> _与 ADR-0004（CPU handoff 状态）相矛盾，但值得重新讨论，因为……_

## 按符号引用代码，而不是按行

Upstream（`szemzoa/awboot`）按固定版本的文件和行号引用，因为那棵树从不移动。**我们自己的**代码按符号（`sunxi_gpio_set_pull`、`ccu::USART_BGR_REG`）引用，绝不按行号：我们的文件会变，`mod.rs:120` 这样的引用会腐烂成错误的事实。
