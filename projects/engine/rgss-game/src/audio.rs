//! RGSS `Audio` 模块：记录播放请求（尚无真实发声）。
//!
//! 脚本常在标题/场景切换时调用 `bgm_play` / `se_play` 等。先提供完整方法面，
//! 避免未注册宿主槽导致运行失败。后续可接 Spark 音频或系统播放器。

use std::sync::{Arc, Mutex};

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

/// 跨帧可查询的 Audio 状态。
#[derive(Debug, Default)]
pub struct AudioState {
    bgm: Mutex<Option<AudioCue>>,
    bgs: Mutex<Option<AudioCue>>,
    me: Mutex<Option<AudioCue>>,
    se: Mutex<Option<AudioCue>>,
}

impl AudioState {
    /// 新建空状态。
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 最近一次 BGM 请求。
    pub fn last_bgm(&self) -> Option<AudioCue> {
        self.bgm.lock().ok()?.clone()
    }

    /// 最近一次 SE 请求。
    pub fn last_se(&self) -> Option<AudioCue> {
        self.se.lock().ok()?.clone()
    }

    fn set(slot: &Mutex<Option<AudioCue>>, cue: Option<AudioCue>) {
        if let Ok(mut g) = slot.lock() {
            *g = cue;
        }
    }
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
            AudioState::set(&audio.bgm, Some(play_cue(ctx.heap, &args)));
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgm_stop", move |_ctx, _args| {
            AudioState::set(&audio.bgm, None);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgm_fade", move |_ctx, _args| {
            AudioState::set(&audio.bgm, None);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgs_play", move |ctx, args| {
            AudioState::set(&audio.bgs, Some(play_cue(ctx.heap, &args)));
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgs_stop", move |_ctx, _args| {
            AudioState::set(&audio.bgs, None);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_bgs_fade", move |_ctx, _args| {
            AudioState::set(&audio.bgs, None);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_me_play", move |ctx, args| {
            AudioState::set(&audio.me, Some(play_cue(ctx.heap, &args)));
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_me_stop", move |_ctx, _args| {
            AudioState::set(&audio.me, None);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_me_fade", move |_ctx, _args| {
            AudioState::set(&audio.me, None);
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_se_play", move |ctx, args| {
            AudioState::set(&audio.se, Some(play_cue(ctx.heap, &args)));
            Ok(Value::Null)
        });
    }
    {
        let audio = audio.clone();
        vm.register_native("Audio_se_stop", move |_ctx, _args| {
            AudioState::set(&audio.se, None);
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
        let audio = AudioState::new();
        let mut heap = Heap::new();
        let path = heap.alloc_string("Theme2");
        AudioState::set(
            &audio.bgm,
            Some(play_cue(
                &heap,
                &[path, Value::Number(80.0), Value::Number(110.0)],
            )),
        );
        let cue = audio.last_bgm().expect("bgm");
        assert_eq!(cue.path, "Theme2");
        assert_eq!(cue.volume, 80.0);
        assert_eq!(cue.pitch, 110.0);
        AudioState::set(&audio.bgm, None);
        assert!(audio.last_bgm().is_none());

        let se_path = heap.alloc_string("Cursor1");
        AudioState::set(&audio.se, Some(play_cue(&heap, &[se_path])));
        assert_eq!(audio.last_se().expect("se").path, "Cursor1");
    }
}
