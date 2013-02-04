# `@game-gpt/rgss`

产品入口。

```text
rgss --path <游戏根>            # 开窗口跑
rgss detect --path <游戏根>     # 仅检测
rgss play --path <游戏根>       # 无头编译执行
```

`--path` 必须是游戏根（含 `Game.ini` 或 `RPG_RT.ini` 等指纹）。没有可识别指纹时命令失败。

开窗口需要已构建的 `rgss` 二进制（`cargo build -p rgss-game --bin rgss`，或走 `pnpm launch`）。
