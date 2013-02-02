#![warn(missing_docs)]
//! RPG Maker **Node-API** 绑定。
//!
//! 产品入口：
//!
//! ```text
//! rgss detect --path <游戏根>
//! ```
//!
//! ```text
//! cargo build -p rgss-napi --release --features node
//! ```

mod host;

#[cfg(feature = "node")]
mod node;

pub use host::{HostInfo, RgssHost};

/// npm 元包名。
pub const NPM_PACKAGE_NAME: &str = "@game-gpt/rgss";
