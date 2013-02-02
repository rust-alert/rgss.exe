# `@game-gpt/rgss`

产品入口。`rgss detect --path <游戏根>` 识别 RPG Maker 2000 / 2003 / XP / VX / VX Ace。

`--path` 必须是游戏根（含 `Game.ini` 或 `RPG_RT.ini` 等指纹）。没有可识别指纹时命令失败。不要使用 `cargo run`。窗口不由此命令打开。
