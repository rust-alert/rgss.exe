# `rgss-game`

游戏根校验、脚本编译执行，以及窗口试跑。

产品二进制名是 **`rgss`**：

```text
cargo run -p rgss-game --bin rgss -- --path <游戏根>
```

库 API 仍由 `rgss-napi` / `@game-gpt/rgss` 调用（`detect` / 无头 `play`）。
