# RGSS

RPG Maker 游戏包检测宿主。Spark 是引擎，本仓负责认出游戏根属于哪一版。

XP / VX / VX Ace 的脚本是 **RGSS 特殊 Ruby**。2000 / 2003 只有事件指令，这里只报告版本，不执行。

本仓库不附带 RPG Maker 运行时、RTP 或任何游戏素材。

## 检测

```bash
pnpm install
pnpm launch -- --path "<游戏根>"
```

发布构建：`pnpm launch -- --path "<游戏根>" --release`。

等价命令：

```text
rgss detect --path <游戏根>
```

成功时打印一行，例如 `XP RgssRuby Library=RGSS103J.dll`。

| 引擎             | 结果                                 |
|------------------|--------------------------------------|
| 2000 / 2003      | `Event`                              |
| XP / VX / VX Ace | `RgssRuby`                           |
| MV / MZ          | 失败，错误码 `rgss.detect.js_engine` |

## 开发构建

```bash
cargo test -p rgss-core
cargo check -p rgss-napi
```
