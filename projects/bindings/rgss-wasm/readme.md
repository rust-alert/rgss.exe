# `rgss-wasm`

Wasm 绑定。导出 C ABI，产物拷至 `projects/platforms/wasm/rgss-unknown-wasm32`。

```bash
cargo build -p rgss-wasm --target wasm32-unknown-unknown --release
```

游戏根检测仍经 `@game-gpt/rgss` → `rgss-napi`。
