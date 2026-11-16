# kernel-api（模块契约）

这是 godoword 的"接口说明书"。所有适配器与教学模块都只依赖它，彼此不直接往来。
这样任何一个模块出问题，都可以照着这份契约整体重写，而不牵连其他模块。

开发者向的精确 trait 定义见 [docs/contract.md](../docs/contract.md)。