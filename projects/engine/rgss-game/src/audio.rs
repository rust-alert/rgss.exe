//! RGSS `Audio`：解析 `Audio/BGM|BGS|ME|SE` 下的文件并用 `spark-audio` 播放。
//!
//! MIDI 无法解码时仍记下 cue，但不发声。`fade(time)` 在后台按毫秒渐降音量后停止。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use spark_audio::{AudioBus, AudioDecoder, Playback};
use spark_gc::{GcObject, Value};
use spark_vm::Vm;

/// 一路音频上一次播放请求。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioCue {
    /// 相对游戏根的路径（或脚本传入的原样字符串）。
    pub path: String,
    pub volume: f32,
    pub pitch: f32,
}

/// 一路音频：最近一次请求，以及仍在响的句柄。
#[derive(Default)]
struct Slot {
    cue: Option<AudioCue>,
    play: Option<Playback>,
}

/// 跨帧可查询的 Audio 状态。
pub struct AudioState {
    root: PathBuf,
    bus: Mutex<Option<AudioBus>>,
    bgm: Mutex<Slot>,
    bgs: Mutex<Slot>,
    me: Mutex<Slot>,
    se: Mutex<Slot>,
}

impl AudioState {
    /// 新建。设备在第一次真正播放时才打开。
    pub fn new(game_root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            root: game_root,
            bus: Mutex::new(None),
            bgm: Mutex::new(Slot::default()),
            bgs: Mutex::new(Slot::default()),
            me: Mutex::new(Slot::default()),
            se: Mutex::new(Slot::default()),
        })
    }

    /// 最近一次 BGM 请求。
    pub fn last_bgm(&self) -> Option<AudioCue> {
        self.bgm.lock().ok()?.cue.clone()
    }

    /// 最近一次 SE 请求。
    pub fn last_se(&self) -> Option<AudioCue> {
        self.se.lock().ok()?.cue.clone()
    }

    fn play_kind(&self, kind: AudioKind, cue: AudioCue) {
        let (folder, looping) = match kind {
            AudioKind::Bgm => ("BGM", true),
            AudioKind::Bgs => ("BGS", true),
            AudioKind::Me => ("ME", false),
            AudioKind::Se => ("SE", false),
        };
        let playback = resolve_audio_file(&self.root, folder, &cue.path).and_then(|path| {
            let pcm = AudioDecoder::decode_path(&path, None).ok()?;
            let mut guard = self.bus.lock().ok()?;
            let bus = guard.get_or_insert_with(AudioBus::try_open);
            let volume = (cue.volume / 100.0).clamp(0.0, 1.0);
            let speed = if cue.pitch <= 0.0 {
                1.0
            } else {
                (cue.pitch / 100.0).clamp(0.05, 4.0)
            };
            bus.start_pcm(&pcm, volume, speed, looping).ok().flatten()
        });
        if let Ok(mut g) = self.slot(kind).lock() {
            g.play = playback;
            g.cue = Some(cue);
        }
    }

    fn stop_kind(&self, kind: AudioKind) {
        if let Ok(mut g) = self.slot(kind).lock() {
            g.play = None;
            g.cue = None;
        }
    }

    /// `Audio.xxx_fade(time)`：`time` 为毫秒。取出播放句柄后在后台渐降音量。
    fn fade_kind(&self, kind: AudioKind, duration_ms: f32) {
        let (play, start_vol) = {
            let Ok(mut g) = self.slot(kind).lock() else {
                return;
            };
            let start = g
                .cue
                .as_ref()
                .map(|c| (c.volume / 100.0).clamp(0.0, 1.0))
                .unwrap_or(1.0);
            g.cue = None;
            (g.play.take(), start)
        };
        let Some(play) = play else {
            return;
        };
        fade_playback(play, start_vol, duration_ms);
    }

    fn slot(&self, kind: AudioKind) -> &Mutex<Slot> {
        match kind {
            AudioKind::Bgm => &self.bgm,
            AudioKind::Bgs => &self.bgs,
            AudioKind::Me => &self.me,
            AudioKind::Se => &self.se,
        }
    }
}

/// 后台渐降音量后停止。`duration_ms < 1` 时立即停止。
fn fade_playback(play: Playback, start_vol: f32, duration_ms: f32) {
    if duration_ms < 1.0 {
        play.stop();
        return;
    }
    thread::spawn(move || {
        let steps = ((duration_ms / 50.0).ceil() as u32).clamp(4, 40);
        let step = Duration::from_millis((duration_ms / steps as f32).max(1.0) as u64);
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            play.set_volume(start_vol * (1.0 - t));
            thread::sleep(step);
        }
        play.stop();
    });
}

/// 渐降步数（供测试）。
pub fn fade_step_count(duration_ms: f32) -> u32 {
    if duration_ms < 1.0 {
        return 0;
    }
    ((duration_ms / 50.0).ceil() as u32).clamp(4, 40)
}

/// BGM / BGS 循环，ME / SE 一次。
#[derive(Clone, Copy)]
enum AudioKind {
    Bgm,
    Bgs,
    Me,
    Se,
}

/// 在游戏根下解析 RGSS 音频名。优先已存在的文件，否则试 `Audio/<folder>/<name>.<ext>`。
///
/// 同时存在时优先可解码格式（ogg/wav/mp3/flac），最后才是 mid。
pub fn resolve_audio_file(root: &Path, folder: &str, name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let direct = root.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let named = root.join("Audio").join(folder).join(name);
    if named.is_file() {
        return Some(named);
    }
    for ext in ["ogg", "wav", "mp3", "flac", "mid"] {
        let candidate = named.with_extension(ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn path_from_value(heap: &spark_gc::Heap, v: &Value) -> String {
    match v {
        Value::Handle(h) => match heap.get(*h) {
            Ok(GcObject::String(s)) => s.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

fn num_arg(args: &[Value], idx: usize, default: f32) -> f32 {
    args.get(idx)
        .and_then(|v| v.as_number())
        .unwrap_or(default as f64) as f32
}

fn play_cue(heap: &spark_gc::Heap, args: &[Value]) -> AudioCue {
    // Audio.xxx_play(filename, volume=100, pitch=100) — 无 self。
    let path = args
        .first()
        .map(|v| path_from_value(heap, v))
        .unwrap_or_default();
    AudioCue {
        path,
        volume: num_arg(args, 1, 100.0),
        pitch: num_arg(args, 2, 100.0),
    }
}

/// 注册 `Audio_*` 宿主函数。
pub fn register_audio_natives(vm: &mut Vm, audio: Arc<AudioState>) {
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgm_play", move |ctx, args| {
            audio.play_kind(AudioKind::Bgm, play_cue(ctx.heap, &args));
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgm_stop", move |_ctx, _args| {
            audio.stop_kind(AudioKind::Bgm);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgm_fade", move |_ctx, args| {
            let ms = num_arg(&args, 0, 0.0);
            audio.fade_kind(AudioKind::Bgm, ms);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgs_play", move |ctx, args| {
            audio.play_kind(AudioKind::Bgs, play_cue(ctx.heap, &args));
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgs_stop", move |_ctx, _args| {
            audio.stop_kind(AudioKind::Bgs);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgs_fade", move |_ctx, args| {
            let ms = num_arg(&args, 0, 0.0);
            audio.fade_kind(AudioKind::Bgs, ms);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_me_play", move |ctx, args| {
            audio.play_kind(AudioKind::Me, play_cue(ctx.heap, &args));
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_me_stop", move |_ctx, _args| {
            audio.stop_kind(AudioKind::Me);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_me_fade", move |_ctx, args| {
            let ms = num_arg(&args, 0, 0.0);
            audio.fade_kind(AudioKind::Me, ms);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_se_play", move |ctx, args| {
            audio.play_kind(AudioKind::Se, play_cue(ctx.heap, &args));
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_se_stop", move |_ctx, _args| {
            audio.stop_kind(AudioKind::Se);
            Ok(Value::Null)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spark_gc::Heap;

    #[test]
    fn play_args_default_volume_pitch() {
        let mut heap = Heap::new();
        let path = heap.alloc_string("Audio/BGM/Theme");
        let cue = play_cue(&heap, &[path]);
        assert_eq!(cue.path, "Audio/BGM/Theme");
        assert_eq!(cue.volume, 100.0);
        assert_eq!(cue.pitch, 100.0);
    }

    #[test]
    fn bgm_play_records_cue() {
        let audio = AudioState::new(PathBuf::from("."));
        let mut heap = Heap::new();
        let path = heap.alloc_string("Theme2");
        audio.play_kind(
            AudioKind::Bgm,
            play_cue(&heap, &[path, Value::Number(80.0), Value::Number(110.0)]),
        );
        let cue = audio.last_bgm().expect("bgm");
        assert_eq!(cue.path, "Theme2");
        assert_eq!(cue.volume, 80.0);
        assert_eq!(cue.pitch, 110.0);
        audio.stop_kind(AudioKind::Bgm);
        assert!(audio.last_bgm().is_none());

        let se_path = heap.alloc_string("Cursor1");
        audio.play_kind(AudioKind::Se, play_cue(&heap, &[se_path]));
        assert_eq!(audio.last_se().expect("se").path, "Cursor1");
    }

    #[test]
    fn fade_step_count_clamps() {
        assert_eq!(fade_step_count(0.0), 0);
        assert_eq!(fade_step_count(100.0), 4);
        assert_eq!(fade_step_count(1000.0), 20);
        assert_eq!(fade_step_count(10_000.0), 40);
    }

    #[test]
    fn resolve_prefers_ogg_over_mid() {
        let dir = std::env::temp_dir().join(format!("rgss-audio-{}", std::process::id()));
        let bgm = dir.join("Audio").join("BGM");
        std::fs::create_dir_all(&bgm).unwrap();
        std::fs::write(bgm.join("Theme.mid"), b"MThd").unwrap();
        std::fs::write(bgm.join("Theme.ogg"), b"OggS").unwrap();
        let found = resolve_audio_file(&dir, "BGM", "Theme").expect("file");
        assert_eq!(found.extension().and_then(|e| e.to_str()), Some("ogg"));
        assert!(resolve_audio_file(&dir, "BGM", "").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
