use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rgss_core::{DetectError, MakerEngine, ScriptKind, detect_game_root};

fn scratch() -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("rgss-detect-{millis}-{n}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    if let Some(parent) = Path::new(name).parent() {
        if parent != Path::new("") {
            fs::create_dir_all(dir.join(parent)).unwrap();
        }
    }
    fs::write(dir.join(name), bytes).unwrap();
}

fn ldb(ids: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for id in ids {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.push(0);
    }
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes
}

#[test]
fn detects_xp_from_ini_and_project() {
    let dir = scratch();
    write(
        &dir,
        "Game.ini",
        b"[Game]\r\nLibrary=RGSS103J.dll\r\nScripts=Data\\Scripts.rxdata\r\nTitle=Sample\r\nRTP1=RPGXP\r\n",
    );
    write(&dir, "Game.rxproj", b"RPGXP 1.03");
    let report = detect_game_root(&dir).unwrap();
    assert_eq!(report.engine, MakerEngine::Xp);
    assert_eq!(report.script, ScriptKind::RgssRuby);
    assert_eq!(report.library.as_deref(), Some("RGSS103J.dll"));
    assert_eq!(report.scripts_path.as_deref(), Some("Data\\Scripts.rxdata"));
    assert!(report.summary_line().starts_with("XP RgssRuby"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn decodes_shift_jis_title() {
    let dir = scratch();
    let mut ini = b"[Game]\r\nLibrary=RGSS104E.dll\r\nTitle=".to_vec();
    ini.extend_from_slice(&[0x83, 0x65, 0x83, 0x58, 0x83, 0x67]);
    ini.extend_from_slice(b"\r\n");
    write(&dir, "Game.ini", &ini);
    let report = detect_game_root(&dir).unwrap();
    assert_eq!(report.engine, MakerEngine::Xp);
    assert_eq!(report.title.as_deref(), Some("テスト"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn decodes_gbk_title() {
    let dir = scratch();
    let mut ini = b"[Game]\r\nLibrary=RGSS103J.dll\r\nTitle=".to_vec();
    // 「罪途」GBK
    ini.extend_from_slice(&[0xD7, 0xEF, 0xCD, 0xBE]);
    ini.extend_from_slice(b"\r\n");
    write(&dir, "Game.ini", &ini);
    let report = detect_game_root(&dir).unwrap();
    assert_eq!(report.engine, MakerEngine::Xp);
    assert_eq!(report.title.as_deref(), Some("罪途"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn detects_vx_and_ace() {
    let vx = scratch();
    write(
        &vx,
        "Game.ini",
        b"Library=RGSS202E.dll\r\nScripts=Data\\Scripts.rvdata\r\n",
    );
    write(&vx, "Data/System.rvdata", b"");
    let report = detect_game_root(&vx).unwrap();
    assert_eq!(report.engine, MakerEngine::Vx);
    let _ = fs::remove_dir_all(vx);

    let ace = scratch();
    write(&ace, "Game.ini", b"Library=RGSS301.dll\r\n");
    write(&ace, "Game.rvproj2", b"RPGVXAce 1.02");
    let report = detect_game_root(&ace).unwrap();
    assert_eq!(report.engine, MakerEngine::Ace);
    assert_eq!(report.script, ScriptKind::RgssRuby);
    let _ = fs::remove_dir_all(ace);
}

#[test]
fn detects_2000_and_2003() {
    let rm2000 = scratch();
    write(&rm2000, "RPG_RT.ini", b"[RPG_RT]\r\nGameTitle=Old\r\n");
    write(&rm2000, "RPG_RT.exe", b"");
    write(&rm2000, "RPG_RT.ldb", &ldb(&[0x0B]));
    let report = detect_game_root(&rm2000).unwrap();
    assert_eq!(report.engine, MakerEngine::Rm2000);
    assert_eq!(report.script, ScriptKind::Event);
    assert_eq!(report.title.as_deref(), Some("Old"));
    assert!(report.summary_line().starts_with("2000 Event"));
    let _ = fs::remove_dir_all(rm2000);

    let rm2003 = scratch();
    write(&rm2003, "RPG_RT.ini", b"GameTitle=New\r\n");
    write(&rm2003, "RPG_RT.ldb", &ldb(&[0x0B, 0x1D]));
    let report = detect_game_root(&rm2003).unwrap();
    assert_eq!(report.engine, MakerEngine::Rm2003);
    assert_eq!(report.script, ScriptKind::Event);
    let _ = fs::remove_dir_all(rm2003);
}

#[test]
fn version_chunk_1a_is_still_2000() {
    let dir = scratch();
    write(&dir, "RPG_RT.exe", b"");
    write(&dir, "RPG_RT.ldb", &ldb(&[0x1A]));
    let report = detect_game_root(&dir).unwrap();
    assert_eq!(report.engine, MakerEngine::Rm2000);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rejects_mv_and_conflicts() {
    let mv = scratch();
    write(&mv, "Game.rpgproject", b"RPGMV");
    write(&mv, "js/rpg_core.js", b"");
    let err = detect_game_root(&mv).unwrap_err();
    assert_eq!(err, DetectError::JsEngine);
    assert_eq!(err.to_string(), "rgss.detect.js_engine");
    let _ = fs::remove_dir_all(mv);

    let clash = scratch();
    write(&clash, "Game.ini", b"Library=RGSS301.dll\r\n");
    write(&clash, "RPG_RT.exe", b"");
    write(&clash, "RPG_RT.ldb", &ldb(&[0x0B]));
    let err = detect_game_root(&clash).unwrap_err();
    assert_eq!(err.code(), "rgss.detect.ambiguous");
    assert!(err.explain().contains("2000"));
    assert!(err.explain().contains("VXAce"));
    let _ = fs::remove_dir_all(clash);

    let empty = scratch();
    assert_eq!(
        detect_game_root(&empty).unwrap_err().code(),
        "rgss.detect.unrecognized"
    );
    let _ = fs::remove_dir_all(empty);

    let missing = scratch().join("nope");
    assert_eq!(
        detect_game_root(&missing).unwrap_err().code(),
        "rgss.detect.not_directory"
    );
}

#[test]
#[ignore]
fn detect_external_sample() {
    let path = std::env::var("RGSS_SAMPLE").expect("RGSS_SAMPLE");
    let report = detect_game_root(Path::new(&path)).unwrap();
    assert_eq!(report.engine, MakerEngine::Xp);
    assert_eq!(report.script, ScriptKind::RgssRuby);
    assert_eq!(report.library.as_deref(), Some("RGSS103J.dll"));
}
