# Цель armv7a-none-eabi (soft-float), а не hard-float ABI

Оригинал на C использует `-mfloat-abi=hard`, но загрузчик не делает вычислений
с плавающей точкой, так что hard-float ничего не даёт. Мы измерили оба: `.text`
занимает 22 067 B для `armv7a-none-eabi` (soft) против 22 167 B для
`armv7a-none-eabihf` (hard) — hard-float на 100 B *больше*, и он потребовал бы
включения VFP и hard-float ABI без всякой выгоды. Поэтому мы
стандартизируемся на `armv7a-none-eabi`.
