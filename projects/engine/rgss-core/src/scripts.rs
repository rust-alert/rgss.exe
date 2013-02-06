//! XP / VX / Ace：`Scripts.rxdata`（或 ini 指定路径）加载。

use std::path::{Path, PathBuf};

use flate2::read::ZlibDecoder;
use std::io::Read;

use crate::detect::{DetectReport, MakerEngine, detect_game_root};
use crate::marshal::{self, MarshalError, MarshalValue};

/// 一条脚本槽。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptEntry {
    /// 编辑器侧 ID。
    pub id: i64,
    /// 脚本名。
    pub name: String,
    /// 解压后的源码（UTF-8 有损）。
    pub source: String,
}

/// 脚本包。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPack {
    /// 来自检测。
    pub engine: MakerEngine,
    /// 脚本文件相对路径。
    pub scripts_path: PathBuf,
    /// 按顺序的脚本。
    pub entries: Vec<ScriptEntry>,
}

/// 加载错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptsError {
    /// 检测失败说明。
    Detect(String),
    /// 不是 RGSS Ruby 引擎。
    NotRgssRuby {
        /// 引擎标签。
        engine: String,
    },
    /// 缺脚本文件。
    MissingFile {
        /// 路径。
        path: String,
    },
    /// Marshal。
    Marshal(String),
    /// zlib。
    Inflate(String),
    /// 形状不对。
    BadShape,
}

impl std::fmt::Display for ScriptsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Detect(_) => f.write_str("rgss.scripts.detect"),
            Self::NotRgssRuby { .. } => f.write_str("rgss.scripts.not_rgss_ruby"),
            Self::MissingFile { .. } => f.write_str("rgss.scripts.missing_file"),
            Self::Marshal(_) => f.write_str("rgss.scripts.marshal"),
            Self::Inflate(_) => f.write_str("rgss.scripts.inflate"),
            Self::BadShape => f.write_str("rgss.scripts.bad_shape"),
        }
    }
}

impl std::error::Error for ScriptsError {}

impl ScriptsError {
    /// 人类可读说明。
    pub fn explain(&self) -> String {
        match self {
            Self::Detect(s) => s.clone(),
            Self::NotRgssRuby { engine } => {
                format!("引擎 {engine} 没有 Scripts.rxdata 式 RGSS Ruby 包")
            }
            Self::MissingFile { path } => format!("找不到脚本包：{path}"),
            Self::Marshal(s) => format!("Marshal 失败：{s}"),
            Self::Inflate(s) => format!("zlib 失败：{s}"),
            Self::BadShape => "Scripts 根不是 [[id, name, blob], ...]。".into(),
        }
    }
}

/// 从游戏根加载脚本包。
pub fn load_scripts(game_root: &Path) -> Result<ScriptPack, ScriptsError> {
    let report = detect_game_root(game_root)
        .map_err(|e| ScriptsError::Detect(format!("{} {}", e.code(), e.explain())))?;
    load_scripts_with_report(game_root, &report)
}

/// 已有检测结果时加载。
pub fn load_scripts_with_report(
    game_root: &Path,
    report: &DetectReport,
) -> Result<ScriptPack, ScriptsError> {
    match report.engine {
        MakerEngine::Xp | MakerEngine::Vx | MakerEngine::Ace => {}
        other => {
            return Err(ScriptsError::NotRgssRuby {
                engine: other.label().into(),
            });
        }
    }
    let rel = report
        .scripts_path
        .clone()
        .unwrap_or_else(|| default_scripts_rel(report.engine).into());
    let path = join_game_path(game_root, &rel);
    if !path.is_file() {
        return Err(ScriptsError::MissingFile {
            path: path.display().to_string(),
        });
    }
    let bytes = std::fs::read(&path).map_err(|e| ScriptsError::MissingFile {
        path: format!("{} ({e})", path.display()),
    })?;
    let root = marshal::load(&bytes).map_err(|e: MarshalError| ScriptsError::Marshal(e.to_string()))?;
    let entries = decode_script_array(&root)?;
    Ok(ScriptPack {
        engine: report.engine,
        scripts_path: PathBuf::from(rel.replace('\\', "/")),
        entries,
    })
}

fn default_scripts_rel(engine: MakerEngine) -> &'static str {
    match engine {
        MakerEngine::Xp => "Data/Scripts.rxdata",
        MakerEngine::Vx => "Data/Scripts.rvdata",
        MakerEngine::Ace => "Data/Scripts.rvdata2",
        _ => "Data/Scripts.rxdata",
    }
}

fn join_game_path(root: &Path, rel: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for part in rel.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        out.push(part);
    }
    out
}

fn decode_script_array(root: &MarshalValue) -> Result<Vec<ScriptEntry>, ScriptsError> {
    let arr = root.as_array().ok_or(ScriptsError::BadShape)?;
    let mut entries = Vec::with_capacity(arr.len());
    for item in arr {
        let triple = item.as_array().ok_or(ScriptsError::BadShape)?;
        if triple.len() < 3 {
            return Err(ScriptsError::BadShape);
        }
        let id = triple[0].as_fixnum().unwrap_or(0);
        let name = match &triple[1] {
            MarshalValue::String(b) => decode_name(b),
            MarshalValue::Symbol(s) => s.clone(),
            _ => String::new(),
        };
        let blob = triple[2].as_bytes().unwrap_or(&[]);
        let source = if blob.is_empty() {
            String::new()
        } else {
            inflate_script(blob)?
        };
        entries.push(ScriptEntry { id, name, source });
    }
    Ok(entries)
}

fn decode_name(bytes: &[u8]) -> String {
    crate::detect::decode_legacy_bytes(bytes)
}

fn inflate_script(blob: &[u8]) -> Result<String, ScriptsError> {
    let mut decoder = ZlibDecoder::new(blob);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| ScriptsError::Inflate(e.to_string()))?;
    Ok(crate::detect::decode_legacy_bytes(&out))
}

/// 去掉 `=begin` / `=end` 块与行注释前的空白行压缩，保留可执行源码。
pub fn strip_rgss_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_begin = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !in_begin && trimmed.starts_with("=begin") {
            in_begin = true;
            continue;
        }
        if in_begin {
            if trimmed.starts_with("=end") {
                in_begin = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}
