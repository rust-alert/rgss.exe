//! JS / 测试共用的宿主门面。

use std::path::Path;

use rgss_game::{DetectReport, PlayReport, play_game_root, validate_game_root};

/// 给命令行看的包信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInfo {
    /// 产品名。
    pub name: &'static str,
    /// crate 版本。
    pub version: &'static str,
    /// npm 包名。
    pub npm_package: &'static str,
}

impl Default for HostInfo {
    fn default() -> Self {
        Self {
            name: "RGSS",
            version: env!("CARGO_PKG_VERSION"),
            npm_package: crate::NPM_PACKAGE_NAME,
        }
    }
}

/// 检测 / play 宿主。不打开窗口。
#[derive(Debug, Default)]
pub struct RgssHost;

impl RgssHost {
    /// 新建宿主。
    pub fn new() -> Self {
        Self
    }

    /// 包信息。
    pub fn info(&self) -> HostInfo {
        HostInfo::default()
    }

    /// 识别 2000 / 2003 / XP / VX / VX Ace。MV / MZ 失败。
    pub fn detect(&self, path: &str) -> Result<DetectReport, String> {
        validate_game_root(Path::new(path))
    }

    /// 加载 Scripts 并经 oak-ruby → spark-script-ruby → spark-vm 编译执行。
    pub fn play(&self, path: &str) -> Result<PlayReport, String> {
        play_game_root(Path::new(path)).map_err(|e| e.to_string())
    }
}
