#![warn(missing_docs)]
//! RGSS **Wasm** 绑定（`wasm32-unknown-unknown`）。
//!
//! 浏览器侧只暴露版本探针。游戏根检测走 `@game-gpt/rgss` 的 `rgss detect`。
//!
//! ```text
//! cargo build -p rgss-wasm --target wasm32-unknown-unknown --release
//! ```

/// npm 平台包名。
pub const NPM_PLATFORM_PACKAGE: &str = "rgss-unknown-wasm32";

/// 导出版本编码，供加载器确认产物。符号保持稳定，不依赖分配器。
#[unsafe(no_mangle)]
pub extern "C" fn rgss_version_code() -> u32 {
    parse_version_code(env!("CARGO_PKG_VERSION"))
}

/// 二维长度探针。纯运算，无别名指针。
#[unsafe(no_mangle)]
pub extern "C" fn rgss_vec2_length(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

fn parse_version_code(v: &str) -> u32 {
    let mut parts = v.split('.');
    let major: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    major.saturating_mul(1_000_000) + minor.saturating_mul(1_000) + patch
}
