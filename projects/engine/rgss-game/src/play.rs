//! 经 `spark-script-ruby` → `spark-vm` 编译并执行 RGSS 脚本包。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use rgss_core::{
    MarshalValue, ScriptPack, ScriptsError, detect_game_root, load_marshal,
    load_scripts_with_report, strip_rgss_comments,
};
use spark_gc::{GcObject, Value};
use spark_script_ruby::{self, RubyScriptError};
use spark_vm::{Module, StdHost, Vm, VmError};

/// 单脚本编译结果。
#[derive(Debug, Clone)]
pub struct ScriptCompileStatus {
    /// 下标。
    pub index: usize,
    /// 名。
    pub name: String,
    /// 源码字节。
    pub source_bytes: usize,
    /// 成功与否。
    pub ok: bool,
    /// 错误码或说明。
    pub detail: String,
}

/// 一次 play 报告。
#[derive(Debug, Clone)]
pub struct PlayReport {
    /// 引擎标签。
    pub engine: String,
    /// 脚本总数。
    pub total: usize,
    /// 编译成功数。
    pub compiled: usize,
    /// 空脚本数。
    pub empty: usize,
    /// 各脚本状态。
    pub statuses: Vec<ScriptCompileStatus>,
    /// 是否执行了合并模块的入口。
    pub ran_entry: bool,
    /// 入口执行说明。
    pub run_detail: String,
}

impl PlayReport {
    /// 一行摘要。
    pub fn summary_line(&self) -> String {
        format!(
            "{} scripts {}/{} compiled empty={} ran={} {}",
            self.engine,
            self.compiled,
            self.total,
            self.empty,
            self.ran_entry,
            self.run_detail
        )
    }
}

/// Play 错误。
#[derive(Debug)]
pub enum PlayError {
    /// 检测 / 加载。
    Scripts(ScriptsError),
}

impl std::fmt::Display for PlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scripts(e) => write!(f, "{} {}", e, e.explain()),
        }
    }
}

impl std::error::Error for PlayError {}

/// 窗口帧闸：每次 `Graphics.update` 等宿主放行一帧。
#[derive(Debug)]
pub struct FrameSync {
    ready: Mutex<bool>,
    cv: Condvar,
    exit: AtomicBool,
}

impl FrameSync {
    /// 新建闸门。
    pub fn new() -> Self {
        Self {
            ready: Mutex::new(false),
            cv: Condvar::new(),
            exit: AtomicBool::new(false),
        }
    }

    /// 宿主：允许 VM 通过一次 `Graphics.update`。
    pub fn pump_frame(&self) {
        if let Ok(mut ready) = self.ready.lock() {
            *ready = true;
            self.cv.notify_one();
        }
    }

    /// VM：阻塞直到宿主 `pump_frame`；若已请求退出则返回 `false`。
    pub fn wait_frame(&self) -> bool {
        let Ok(mut ready) = self.ready.lock() else {
            return false;
        };
        while !*ready {
            if self.exit.load(Ordering::SeqCst) {
                return false;
            }
            ready = match self.cv.wait(ready) {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
        }
        *ready = false;
        !self.exit.load(Ordering::SeqCst)
    }

    /// 请求结束标题循环并唤醒等待中的 VM。
    pub fn request_exit(&self) {
        self.exit.store(true, Ordering::SeqCst);
        self.pump_frame();
    }

    /// 是否已请求退出。
    pub fn should_exit(&self) -> bool {
        self.exit.load(Ordering::SeqCst)
    }
}

impl Default for FrameSync {
    fn default() -> Self {
        Self::new()
    }
}

/// 已编译、待执行的脚本会话（可供窗口宿主驱动）。
pub struct PlaySession {
    /// 检测标题（窗口用）。
    pub title: String,
    /// 游戏根路径（资源加载）。
    pub game_root: std::path::PathBuf,
    /// 链接后的方法表。
    pub linked: Module,
    /// 按 Scripts 顺序的 `(名, 模块)`。
    pub scripts: Vec<(String, Module)>,
    /// 编译成功数。
    pub compiled: usize,
    /// 空脚本数。
    pub empty: usize,
    /// 总数。
    pub total: usize,
    /// 引擎标签。
    pub engine: String,
    /// 各脚本状态。
    pub statuses: Vec<ScriptCompileStatus>,
}

/// 加载并编译，不执行。
pub fn prepare_game_root(path: &Path) -> Result<PlaySession, PlayError> {
    let detect = detect_game_root(path).map_err(|e| {
        PlayError::Scripts(ScriptsError::Detect(format!("{} {}", e.code(), e.explain())))
    })?;
    let pack = load_scripts_with_report(path, &detect).map_err(PlayError::Scripts)?;
    let title = detect
        .title
        .clone()
        .unwrap_or_else(|| "RGSS".into());
    Ok(prepare_script_pack(&pack, title, path.to_path_buf()))
}

/// 对已加载包编译链接。
pub fn prepare_script_pack(
    pack: &ScriptPack,
    title: String,
    game_root: std::path::PathBuf,
) -> PlaySession {
    let natives = rgss_native_names();
    let mut statuses = Vec::new();
    let mut modules: Vec<(String, Module)> = Vec::new();
    let mut empty = 0usize;

    for (index, entry) in pack.entries.iter().enumerate() {
        let stripped = strip_rgss_comments(&entry.source);
        if stripped.trim().is_empty() {
            empty += 1;
            statuses.push(ScriptCompileStatus {
                index,
                name: entry.name.clone(),
                source_bytes: entry.source.len(),
                ok: true,
                detail: "empty".into(),
            });
            continue;
        }
        match spark_script_ruby::compile(&stripped, &natives) {
            Ok(module) => {
                statuses.push(ScriptCompileStatus {
                    index,
                    name: entry.name.clone(),
                    source_bytes: entry.source.len(),
                    ok: true,
                    detail: "ok".into(),
                });
                modules.push((entry.name.clone(), module));
            }
            Err(err) => {
                statuses.push(ScriptCompileStatus {
                    index,
                    name: entry.name.clone(),
                    source_bytes: entry.source.len(),
                    ok: false,
                    detail: format_ruby_err(&err),
                });
            }
        }
    }

    let compiled = statuses.iter().filter(|s| s.ok && s.detail == "ok").count();
    let linked = if modules.is_empty() {
        Module {
            functions: vec![spark_vm::FuncProto::new("__main", 0)],
            entry: 0,
            native_names: Vec::new(),
        }
    } else {
        Module::link_methods(
            &modules
                .iter()
                .map(|(_, m)| m.clone())
                .collect::<Vec<_>>(),
        )
    };
    eprintln!(
        "rgss play: linked {} methods from {} scripts",
        linked.functions.len().saturating_sub(1),
        modules.len()
    );

    PlaySession {
        title,
        game_root,
        linked,
        scripts: modules,
        compiled,
        empty,
        total: pack.entries.len(),
        engine: pack.engine.label().into(),
        statuses,
    }
}

/// 加载、编译并无头执行（测试用）。
pub fn play_game_root(path: &Path) -> Result<PlayReport, PlayError> {
    let session = prepare_game_root(path)?;
    Ok(run_session_headless(session))
}

/// 对已加载的脚本包执行编译链路。
pub fn play_script_pack(pack: &ScriptPack) -> PlayReport {
    let session = prepare_script_pack(pack, "RGSS".into(), std::path::PathBuf::new());
    run_session_headless(session)
}

fn run_session_headless(session: PlaySession) -> PlayReport {
    let mut report = PlayReport {
        engine: session.engine.clone(),
        total: session.total,
        compiled: session.compiled,
        empty: session.empty,
        statuses: session.statuses.clone(),
        ran_entry: false,
        run_detail: String::new(),
    };
    if session.scripts.is_empty() {
        report.run_detail = "没有脚本通过 oak-ruby → spark-script-ruby 编译".into();
        return report;
    }
    let max_frames: u32 = std::env::var("RGSS_MAX_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(180);
    let frames = Arc::new(AtomicU32::new(0));
    match run_linked(
        session.linked,
        &session.scripts,
        session.game_root.clone(),
        frames,
        max_frames,
        None,
        None,
        crate::input::InputPad::new(),
    ) {
        Ok((v, frame_count, methods)) => {
            report.ran_entry = true;
            report.run_detail = format!(
                "linked ok value={v:?} frames={frame_count} methods={methods} scripts_ok={}/{}",
                session.compiled, session.total
            );
        }
        Err(e) => {
            report.run_detail = format!("linked vm {e} ({e:?})");
        }
    }
    report
}

/// 在后台线程跑脚本，与窗口 `FrameSync` 同步（`Graphics.update` 对齐宿主帧）。
pub fn run_session_threaded(
    session: PlaySession,
    frames: Arc<AtomicU32>,
    sync: Arc<FrameSync>,
    max_frames: u32,
    display: Option<Arc<crate::display::DisplayState>>,
    input: Arc<crate::input::InputPad>,
) -> std::thread::JoinHandle<Result<(Value, u32, usize), VmError>> {
    std::thread::Builder::new()
        .name("rgss-vm".into())
        .stack_size(
            std::env::var("RUST_MIN_STACK")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(16 * 1024 * 1024),
        )
        .spawn(move || {
            run_linked(
                session.linked,
                &session.scripts,
                session.game_root,
                frames,
                max_frames,
                Some(sync),
                display,
                input,
            )
        })
        .expect("spawn rgss-vm")
}

fn format_ruby_err(err: &RubyScriptError) -> String {
    err.to_string()
}

fn rgss_native_names() -> Vec<&'static str> {
    vec![
        "Graphics_freeze",
        "Graphics_transition",
        "Graphics_frame_rate",
        "Graphics_update",
        "Font_default_name_set",
        "Input_update",
        "Input_press?",
        "Input_trigger?",
        "Input_dir4",
        "Audio_me_stop",
        "Audio_bgs_stop",
        "print",
        "puts",
        "p",
        "load_data",
        "save_data",
        "pow",
        "rand",
        "FileTest_exist?",
        "RPG_Cache_title",
        "Bitmap_initialize",
        "Bitmap_fill_rect",
        "Bitmap_blt",
        "Bitmap_dispose",
        "Sprite_initialize",
        "Sprite_dispose",
        "Sprite_update",
        "Color_initialize",
        "Rect_initialize",
        "Viewport_initialize",
        "Viewport_dispose",
    ]
}

fn run_linked(
    linked: Module,
    scripts: &[(String, Module)],
    game_root: std::path::PathBuf,
    frames: Arc<AtomicU32>,
    max_frames: u32,
    sync: Option<Arc<FrameSync>>,
    display: Option<Arc<crate::display::DisplayState>>,
    input: Arc<crate::input::InputPad>,
) -> Result<(Value, u32, usize), VmError> {
    let method_count = linked.functions.len().saturating_sub(1);
    let native_names: Vec<String> = linked.native_names.clone();
    let mut vm = Vm::new(linked);
    for name in &native_names {
        let n = name.clone();
        vm.register_native(n, |_ctx, _args| Ok(Value::Null));
    }
    let display = display.unwrap_or_else(|| crate::display::DisplayState::new(game_root.clone()));
    let arm_budget = Arc::new(AtomicBool::new(false));
    register_rgss_natives(
        &mut vm,
        frames.clone(),
        max_frames,
        arm_budget.clone(),
        sync.clone(),
        display.clone(),
        input.clone(),
    );
    crate::display::register_display_natives(&mut vm, display.clone());
    let mut host = StdHost;
    let mut last = Value::Null;
    let mut title_frames = 0u32;
    for (name, script) in scripts {
        if sync.as_ref().is_some_and(|s| s.should_exit()) {
            break;
        }
        eprintln!("rgss play: run __main of [{name}]");
        frames.store(0, Ordering::SeqCst);
        let is_main = name == "Main";
        arm_budget.store(is_main, Ordering::SeqCst);
        let step_cap = if is_main {
            if sync.is_some() {
                u64::MAX / 4
            } else {
                20_000_000
            }
        } else {
            500_000
        };
        vm.step_limit = step_cap;
        vm.call_hits.clear();
        match vm.run_script_main(script, &mut host) {
            Ok(v) => last = v,
            Err(VmError::CallOverflow) => {
                let f = frames.load(Ordering::SeqCst);
                if f > 0 && is_main {
                    eprintln!("rgss play: title loop exit in [{name}] frames={f}");
                    title_frames = f;
                    last = Value::Number(f as f64);
                    break;
                }
                let mut hits: Vec<_> = vm.call_hits.iter().collect();
                hits.sort_by(|a, b| b.1.cmp(a.1));
                let top: Vec<String> = hits
                    .into_iter()
                    .take(12)
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                eprintln!(
                    "rgss play: skip hang in [{name}] frames={f} hits={}",
                    top.join(" ")
                );
                continue;
            }
            Err(e) => {
                eprintln!("rgss play: skip error in [{name}]: {e}");
                continue;
            }
        }
    }
    Ok((
        last,
        if title_frames > 0 {
            title_frames
        } else {
            frames.load(Ordering::SeqCst)
        },
        method_count,
    ))
}

fn register_rgss_natives(
    vm: &mut Vm,
    frames: Arc<AtomicU32>,
    max_frames: u32,
    arm_budget: Arc<AtomicBool>,
    sync: Option<Arc<FrameSync>>,
    display: Arc<crate::display::DisplayState>,
    input: Arc<crate::input::InputPad>,
) {
    vm.register_native("Graphics_freeze", |_ctx, _args| Ok(Value::Null));
    vm.register_native("Graphics_transition", |_ctx, _args| Ok(Value::Null));
    {
        let frames = frames.clone();
        let arm_budget = arm_budget.clone();
        let sync = sync.clone();
        let display = display.clone();
        vm.register_native("Graphics_update", move |ctx, _args| {
            if !arm_budget.load(Ordering::SeqCst) {
                return Ok(Value::Null);
            }
            display.snapshot_from_heap(ctx.heap);
            if let Some(sync) = &sync {
                if !sync.wait_frame() {
                    ctx.globals.insert("$scene".into(), Value::Null);
                    return Err(VmError::CallOverflow);
                }
            }
            let n = frames.fetch_add(1, Ordering::SeqCst).saturating_add(1);
            if sync.is_none() && n >= max_frames {
                ctx.globals.insert("$scene".into(), Value::Null);
                return Err(VmError::CallOverflow);
            }
            Ok(Value::Null)
        });
    }
    vm.register_native("Graphics_frame_rate", |_ctx, _args| Ok(Value::Number(60.0)));
    vm.register_native("Font_default_name_set", |_ctx, _args| Ok(Value::Null));
    vm.register_native("Input_update", |_ctx, _args| Ok(Value::Null));
    crate::input::register_input_natives(vm, input);
    vm.register_native("Audio_me_stop", |_ctx, _args| Ok(Value::Null));
    vm.register_native("Audio_bgs_stop", |_ctx, _args| Ok(Value::Null));
    {
        let root = display.game_root().to_path_buf();
        vm.register_native("FileTest_exist?", move |ctx, args| {
            let path = args
                .first()
                .and_then(|v| match v {
                    Value::Handle(h) => match ctx.heap.get(*h) {
                        Ok(GcObject::String(s)) => Some(s.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or_default();
            let full = if Path::new(&path).is_absolute() {
                PathBuf::from(&path)
            } else {
                root.join(&path)
            };
            Ok(Value::Bool(full.is_file()))
        });
    }
    {
        let root = display.game_root().to_path_buf();
        vm.register_native("load_data", move |ctx, args| {
            let path = args
                .first()
                .and_then(|v| match v {
                    Value::Handle(h) => match ctx.heap.get(*h) {
                        Ok(GcObject::String(s)) => Some(s.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or_default();
            let full = if Path::new(&path).is_absolute() {
                PathBuf::from(&path)
            } else {
                root.join(&path)
            };
            if let Ok(bytes) = std::fs::read(&full) {
                if let Ok(val) = load_marshal(&bytes) {
                    return Ok(marshal_to_value(ctx, &val));
                }
            }
            // 缺文件：标题进度等常用 Fixnum，回退 0。
            Ok(Value::Number(0.0))
        });
    }
    {
        let root = display.game_root().to_path_buf();
        vm.register_native("save_data", move |ctx, args| {
            let obj = args.first().cloned().unwrap_or(Value::Null);
            let path = args
                .get(1)
                .and_then(|v| match v {
                    Value::Handle(h) => match ctx.heap.get(*h) {
                        Ok(GcObject::String(s)) => Some(s.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap_or_default();
            let full = if Path::new(&path).is_absolute() {
                PathBuf::from(&path)
            } else {
                root.join(&path)
            };
            if let Some(parent) = full.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // 仅写 Fixnum 最小 Marshal，够 RateProgress。
            if let Some(n) = obj.as_number() {
                let mut out = vec![4u8, 8, b'i'];
                let i = n as i64;
                if (0..=122).contains(&i) {
                    out.push((i + 5) as u8);
                } else if (-123..0).contains(&i) {
                    out.push((i - 5) as u8);
                } else {
                    // 简化：四字节小端
                    out.push(4);
                    out.extend_from_slice(&(i as i32).to_le_bytes());
                }
                let _ = std::fs::write(&full, out);
            }
            Ok(Value::Null)
        });
    }
    vm.register_native("rand", |_ctx, _args| Ok(Value::Number(0.0)));
    vm.register_native("pow", |_ctx, args| {
        let a = args.first().and_then(|v| v.as_number()).unwrap_or(0.0);
        let b = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0);
        Ok(Value::Number(a.powf(b)))
    });
    vm.register_native("Hash_new", |ctx, _args| {
        Ok(Value::Handle(ctx.heap.alloc(GcObject::Table(HashMap::new()))))
    });
    vm.register_native("Array_new", |ctx, args| {
        let n = args
            .first()
            .and_then(|v| v.as_number())
            .unwrap_or(0.0)
            .max(0.0) as usize;
        let n = n.min(100_000);
        let fill = args.get(1).cloned().unwrap_or(Value::Null);
        let elems = vec![fill; n];
        Ok(Value::Handle(ctx.heap.alloc(GcObject::Array(elems))))
    });
}

fn marshal_to_value(ctx: &mut spark_vm::NativeCtx<'_>, val: &MarshalValue) -> Value {
    match val {
        MarshalValue::Nil => Value::Null,
        MarshalValue::Bool(b) => Value::Bool(*b),
        MarshalValue::Fixnum(n) => Value::Number(*n as f64),
        MarshalValue::String(b) => {
            let s = String::from_utf8_lossy(b).into_owned();
            ctx.heap.alloc_string(s)
        }
        MarshalValue::Symbol(s) => ctx.heap.alloc_string(s.clone()),
        MarshalValue::Array(items) => {
            let elems: Vec<Value> = items.iter().map(|it| marshal_to_value(ctx, it)).collect();
            Value::Handle(ctx.heap.alloc(GcObject::Array(elems)))
        }
        MarshalValue::Hash(map) => {
            let mut table = HashMap::new();
            for (k, v) in map {
                table.insert(k.clone(), marshal_to_value(ctx, v));
            }
            Value::Handle(ctx.heap.alloc(GcObject::Table(table)))
        }
    }
}
