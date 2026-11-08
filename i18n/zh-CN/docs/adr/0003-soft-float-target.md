# 目标为 armv7a-none-eabi（软浮点），而非硬浮点 ABI

原始 C 使用 `-mfloat-abi=hard`，但引导加载器不做浮点数学运算，所以
硬浮点毫无收益。我们测量了两者：`armv7a-none-eabi`（软）的 `.text` 为 22 067 B，而
`armv7a-none-eabihf`（硬）为 22 167 B——硬浮点反而*大* 100 B，而且会强制启用
VFP 并为无收益的硬浮点 ABI 负责。因此我们统一采用 `armv7a-none-eabi`。
