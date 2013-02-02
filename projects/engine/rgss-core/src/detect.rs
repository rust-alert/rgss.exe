//! 从游戏根目录的公开文件名与文本指纹判断引擎。
//!
//! `RPG_RT.ldb` 只读顶层块号：`0x1B`–`0x20` 仅 2003 写入。
//! `0x1A` 是 2000 1.61 也会写的版本块，不能用来区分。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 识别到的 RPG Maker 引擎。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MakerEngine {
    /// RPG Maker 2000。脚本是事件指令。
    Rm2000,
    /// RPG Maker 2003。脚本是事件指令。
    Rm2003,
    /// RPG Maker XP（RGSS1）。
    Xp,
    /// RPG Maker VX（RGSS2）。
    Vx,
    /// RPG Maker VX Ace（RGSS3）。
    Ace,
}

impl MakerEngine {
    /// 稳定短名，用于命令行。
    pub fn label(self) -> &'static str {
        match self {
            Self::Rm2000 => "2000",
            Self::Rm2003 => "2003",
            Self::Xp => "XP",
            Self::Vx => "VX",
            Self::Ace => "VXAce",
        }
    }

    /// 该引擎使用的脚本种类。
    pub fn script_kind(self) -> ScriptKind {
        match self {
            Self::Rm2000 | Self::Rm2003 => ScriptKind::Event,
            Self::Xp | Self::Vx | Self::Ace => ScriptKind::RgssRuby,
        }
    }
}

/// 脚本模型。只有 RGSS 三代使用特殊 Ruby。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptKind {
    /// 2000 / 2003 事件指令。本切片不执行。
    Event,
    /// XP / VX / VX Ace 的 RGSS Ruby 子集。
    RgssRuby,
}

impl ScriptKind {
    /// 稳定短名。
    pub fn label(self) -> &'static str {
        match self {
            Self::Event => "Event",
            Self::RgssRuby => "RgssRuby",
        }
    }
}

/// 一条投票证据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// 来源，如 `Game.ini` 或 `RPG_RT.ldb`。
    pub source: String,
    /// 观察到的指纹。
    pub detail: String,
}

/// 成功识别的游戏根。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectReport {
    /// 引擎。
    pub engine: MakerEngine,
    /// 脚本种类。
    pub script: ScriptKind,
    /// RGSS `Library` 文件名。2000 / 2003 为空。
    pub library: Option<String>,
    /// `Scripts=` 原文字。
    pub scripts_path: Option<String>,
    /// 标题（`Title` 或 `GameTitle`）。
    pub title: Option<String>,
    /// 非空 `RTP1`–`RTP3`。
    pub rtp: Vec<String>,
    /// 命中的证据。
    pub evidence: Vec<Evidence>,
    /// 传入的游戏根。
    pub path: PathBuf,
}

impl DetectReport {
    /// 一行人类可读摘要。
    pub fn summary_line(&self) -> String {
        let mut line = format!("{} {}", self.engine.label(), self.script.label());
        if let Some(library) = &self.library {
            line.push_str(" Library=");
            line.push_str(library);
        } else if self.script == ScriptKind::Event {
            line.push_str(" RPG_RT");
        }
        if let Some(scripts) = &self.scripts_path {
            line.push_str(" Scripts=");
            line.push_str(scripts);
        }
        if let Some(title) = &self.title {
            line.push_str(" Title=");
            line.push_str(title);
        }
        line
    }
}

/// 检测失败。`Display` 是稳定错误码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectError {
    /// 路径不是目录。
    NotDirectory {
        /// 用户传入的路径。
        path: String,
    },
    /// MV / MZ。
    JsEngine,
    /// 多套引擎指纹同时成立。
    Ambiguous {
        /// 冲突的引擎，已排序。
        engines: Vec<MakerEngine>,
    },
    /// 目录里没有可识别的游戏根。
    Unrecognized,
}

impl DetectError {
    /// 稳定错误码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotDirectory { .. } => "rgss.detect.not_directory",
            Self::JsEngine => "rgss.detect.js_engine",
            Self::Ambiguous { .. } => "rgss.detect.ambiguous",
            Self::Unrecognized => "rgss.detect.unrecognized",
        }
    }

    /// 给命令行的说明。
    pub fn explain(&self) -> String {
        match self {
            Self::NotDirectory { path } => format!("路径不是目录：{path}"),
            Self::JsEngine => "这是 RPG Maker MV 或 MZ（JavaScript），本宿主不支持。".into(),
            Self::Ambiguous { engines } => {
                let names: Vec<_> = engines.iter().map(|e| e.label()).collect();
                format!("版本证据冲突：{}", names.join("、"))
            }
            Self::Unrecognized => {
                "未识别为 RPG Maker 2000、2003、XP、VX 或 VX Ace 游戏根。".into()
            }
        }
    }
}

impl std::fmt::Display for DetectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for DetectError {}

struct IniFields {
    library: Option<String>,
    scripts: Option<String>,
    title: Option<String>,
    game_title: Option<String>,
    rtp: Vec<String>,
}

struct Votes {
    by_engine: BTreeMap<MakerEngine, Vec<Evidence>>,
    js: bool,
    ini: IniFields,
}

/// 识别 `path` 指向的游戏根。
pub fn detect_game_root(path: &Path) -> Result<DetectReport, DetectError> {
    if !path.is_dir() {
        return Err(DetectError::NotDirectory {
            path: path.display().to_string(),
        });
    }

    let mut votes = Votes {
        by_engine: BTreeMap::new(),
        js: js_markers(path),
        ini: IniFields {
            library: None,
            scripts: None,
            title: None,
            game_title: None,
            rtp: Vec::new(),
        },
    };
    collect_rgss(path, &mut votes);
    collect_runtime(path, &mut votes);

    let engines: Vec<MakerEngine> = votes.by_engine.keys().copied().collect();
    match (engines.len(), votes.js) {
        (0, true) => Err(DetectError::JsEngine),
        (0, false) => Err(DetectError::Unrecognized),
        (1, true) => Err(DetectError::Ambiguous { engines }),
        (1, false) => {
            let engine = engines[0];
            let evidence = votes.by_engine.remove(&engine).unwrap_or_default();
            let title = match engine.script_kind() {
                ScriptKind::RgssRuby => votes.ini.title,
                ScriptKind::Event => votes.ini.game_title.or(votes.ini.title),
            };
            Ok(DetectReport {
                engine,
                script: engine.script_kind(),
                library: votes.ini.library,
                scripts_path: votes.ini.scripts,
                title,
                rtp: votes.ini.rtp,
                evidence,
                path: path.to_path_buf(),
            })
        }
        (_, true) => Err(DetectError::Ambiguous { engines }),
        (_, false) => Err(DetectError::Ambiguous { engines }),
    }
}

fn vote(votes: &mut Votes, engine: MakerEngine, source: &str, detail: impl Into<String>) {
    votes
        .by_engine
        .entry(engine)
        .or_default()
        .push(Evidence {
            source: source.to_string(),
            detail: detail.into(),
        });
}

fn collect_rgss(root: &Path, votes: &mut Votes) {
    if let Some(text) = read_named(root, "Game.ini") {
        let ini = parse_ini(&text);
        if let Some(library) = ini.library.clone() {
            let file = file_name_only(&library);
            if let Some(engine) = engine_from_rgss_dll(&file) {
                vote(votes, engine, "Game.ini", format!("Library={file}"));
                votes.ini.library = Some(file);
            }
        }
        if let Some(scripts) = ini.scripts.clone() {
            if let Some(engine) = engine_from_data_name(&scripts) {
                vote(votes, engine, "Game.ini", format!("Scripts={scripts}"));
            }
            votes.ini.scripts = Some(scripts);
        }
        votes.ini.title = ini.title;
        votes.ini.rtp = ini.rtp;
    }

    for name in list_files(root) {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".rvproj2") {
            vote(votes, MakerEngine::Ace, &name, "rvproj2");
        } else if lower.ends_with(".rxproj") {
            vote(votes, MakerEngine::Xp, &name, "rxproj");
        } else if lower.ends_with(".rvproj") {
            vote(votes, MakerEngine::Vx, &name, "rvproj");
        }
        if let Some(engine) = engine_from_project_text(&lower, root, &name) {
            vote(votes, engine, &name, "project-text");
        }
        if lower == "game.rgss3a" {
            vote(votes, MakerEngine::Ace, &name, "rgss3a");
        } else if lower == "game.rgss2a" {
            vote(votes, MakerEngine::Vx, &name, "rgss2a");
        } else if lower == "game.rgssad" {
            vote(votes, MakerEngine::Xp, &name, "rgssad");
        }
    }

    let data = root.join("Data");
    if data.is_dir() {
        for name in list_files(&data) {
            if let Some(engine) = engine_from_data_name(&name) {
                vote(votes, engine, "Data", name);
            }
        }
    }
}

fn collect_runtime(root: &Path, votes: &mut Votes) {
    let mut family = false;
    for marker in ["RPG_RT.exe", "RPG_RT.ini", "RPG_RT.ldb", "RPG_RT.lmt"] {
        if find_file(root, marker).is_some() {
            family = true;
        }
    }
    if list_files(root)
        .iter()
        .any(|name| name.to_ascii_lowercase().ends_with(".lmu"))
    {
        family = true;
    }
    if !family {
        return;
    }

    if let Some(text) = read_named(root, "RPG_RT.ini") {
        let ini = parse_ini(&text);
        if votes.ini.game_title.is_none() {
            votes.ini.game_title = ini.game_title.or(ini.title);
        }
    }

    let text_engine = runtime_project_text(root);
    let ldb_2003 = find_file(root, "RPG_RT.ldb")
        .and_then(|path| fs::read(path).ok())
        .is_some_and(|bytes| ldb_has_2003_chunks(&bytes));

    match (text_engine, ldb_2003) {
        (Some(MakerEngine::Rm2000), true) => {
            vote(votes, MakerEngine::Rm2000, "project", "2000");
            vote(votes, MakerEngine::Rm2003, "RPG_RT.ldb", "chunk-2003");
        }
        (Some(engine), _) => vote(votes, engine, "project", engine.label()),
        (None, true) => vote(votes, MakerEngine::Rm2003, "RPG_RT.ldb", "chunk-2003"),
        (None, false) => vote(votes, MakerEngine::Rm2000, "RPG_RT", "family"),
    }
}

fn runtime_project_text(root: &Path) -> Option<MakerEngine> {
    let mut hit_2000 = false;
    let mut hit_2003 = false;
    for name in list_files(root) {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".r3proj") {
            hit_2003 = true;
            continue;
        }
        if !(lower.ends_with(".rproj") || lower.ends_with(".r3proj")) {
            continue;
        }
        let Some(path) = find_file(root, &name) else {
            continue;
        };
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let text = decode_text(&bytes).to_ascii_lowercase();
        if text.contains("rpg2003") || text.contains("rpg maker 2003") || text.contains("rm2003")
        {
            hit_2003 = true;
        }
        if text.contains("rpg2000") || text.contains("rpg maker 2000") || text.contains("rm2000")
        {
            hit_2000 = true;
        }
    }
    match (hit_2000, hit_2003) {
        (true, false) => Some(MakerEngine::Rm2000),
        (false, true) => Some(MakerEngine::Rm2003),
        _ => None,
    }
}

fn engine_from_project_text(lower_name: &str, root: &Path, name: &str) -> Option<MakerEngine> {
    if !(lower_name.ends_with(".rxproj")
        || lower_name.ends_with(".rvproj")
        || lower_name.ends_with(".rvproj2"))
    {
        return None;
    }
    let path = find_file(root, name)?;
    let bytes = fs::read(path).ok()?;
    let text = decode_text(&bytes);
    let upper = text.to_ascii_uppercase();
    if upper.contains("RPGVXACE") {
        Some(MakerEngine::Ace)
    } else if upper.contains("RPGXP") {
        Some(MakerEngine::Xp)
    } else if upper.contains("RPGVX") {
        Some(MakerEngine::Vx)
    } else {
        None
    }
}

fn engine_from_rgss_dll(file: &str) -> Option<MakerEngine> {
    let upper = file.to_ascii_uppercase();
    let rest = upper.strip_prefix("RGSS")?;
    let generation = rest.chars().next()?;
    match generation {
        '1' => Some(MakerEngine::Xp),
        '2' => Some(MakerEngine::Vx),
        '3' => Some(MakerEngine::Ace),
        _ => None,
    }
}

fn engine_from_data_name(name: &str) -> Option<MakerEngine> {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".rvdata2") {
        Some(MakerEngine::Ace)
    } else if lower.ends_with(".rvdata") {
        Some(MakerEngine::Vx)
    } else if lower.ends_with(".rxdata") {
        Some(MakerEngine::Xp)
    } else {
        None
    }
}

fn js_markers(root: &Path) -> bool {
    if find_file(root, "Game.rpgproject").is_some() {
        return true;
    }
    const MARKERS: &[&str] = &[
        "js/rpg_core.js",
        "js/rmmz_core.js",
        "js/rmmz_objects.js",
        "www/js/rpg_core.js",
        "www/js/rmmz_core.js",
    ];
    MARKERS.iter().any(|rel| root.join(rel).is_file())
}

/// `0x1B`–`0x20` 为 2003 数据库顶层块。
fn ldb_has_2003_chunks(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let id = u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        i += 4;
        if id == 0 {
            break;
        }
        let Some((size, used)) = read_ber(&bytes[i..]) else {
            break;
        };
        i += used;
        if (0x1B..=0x20).contains(&id) {
            return true;
        }
        let size = size as usize;
        if i.checked_add(size).is_none_or(|end| end > bytes.len()) {
            break;
        }
        i += size;
    }
    false
}

fn read_ber(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut value: u32 = 0;
    for (n, &byte) in bytes.iter().enumerate() {
        if n >= 5 {
            return None;
        }
        value = value.checked_shl(7)?.checked_add(u32::from(byte & 0x7F))?;
        if byte & 0x80 == 0 {
            return Some((value, n + 1));
        }
    }
    None
}

fn parse_ini(text: &str) -> IniFields {
    let mut fields = IniFields {
        library: None,
        scripts: None,
        title: None,
        game_title: None,
        rtp: Vec::new(),
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') || line.starts_with('[')
        {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = strip_quotes(value.trim());
        if value.is_empty() && !matches!(key.as_str(), "title" | "gametitle") {
            if matches!(key.as_str(), "rtp1" | "rtp2" | "rtp3") {
                continue;
            }
        }
        match key.as_str() {
            "library" => fields.library = Some(value),
            "scripts" => fields.scripts = Some(value),
            "title" => fields.title = Some(value),
            "gametitle" => fields.game_title = Some(value),
            "rtp1" | "rtp2" | "rtp3" if !value.is_empty() => fields.rtp.push(value),
            _ => {}
        }
    }
    fields
}

fn strip_quotes(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn file_name_only(value: &str) -> String {
    value
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(value)
        .to_string()
}

fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    encoding_rs::SHIFT_JIS.decode(bytes).0.into_owned()
}

fn read_named(root: &Path, name: &str) -> Option<String> {
    let path = find_file(root, name)?;
    let bytes = fs::read(path).ok()?;
    Some(decode_text(&bytes))
}

fn find_file(root: &Path, name: &str) -> Option<PathBuf> {
    let direct = root.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    for entry in fs::read_dir(root).ok()?.flatten() {
        if entry.file_name().to_string_lossy().eq_ignore_ascii_case(name) && entry.path().is_file()
        {
            return Some(entry.path());
        }
    }
    None
}

fn list_files(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        if entry.path().is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names
}
