//! Protocol-1 progress, independent heartbeats, cancellation and canonical JSON lines.
use crate::{Error, ErrorKind, Result};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
/// Stable CLI event protocol major, independent of package versions and retention schemas.
pub const PROTOCOL_VERSION: u32 = 1;
/// Thread-safe, idempotent cancellation shared by callers and active operations.
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken {
    /// Create an uncancelled token.
    pub fn new() -> Self {
        Self::default()
    }
    /// Request cancellation at the next safe checkpoint; never implies rollback.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
/// One sequenced event; serialize with [`JsonLines`] for the canonical wire encoding.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Monotonic per-session sequence starting at zero.
    pub seq: u64,
    /// Monotonic session age in milliseconds.
    pub elapsed_ms: u64,
    /// Event-specific fields, flattened with the `type` discriminator.
    #[serde(flatten)]
    pub kind: EventKind,
}
/// Stable sanitized reason for a bounded crate transport retry.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetryKind {
    /// HTTP 429; upstream asked to reduce request rate.
    RateLimited,
    /// HTTP 503; service temporarily unavailable.
    Unavailable,
    /// Transient transport/read failure, excluding TLS/auth/integrity refusal.
    Transport,
}
/// Protocol-1 events. Nullable byte totals mean unknown, never an estimate of zero.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    /// First CLI event, including cheap local discovery.
    Hello {
        /// Protocol major.
        protocol: u32,
        /// Backend package version.
        tool_version: String,
        /// Invoked command.
        command: String,
    },
    /// Begin a workflow phase.
    Stage {
        /// Human phase label.
        name: String,
        /// One-based phase index.
        index: u64,
        /// Phase count if known.
        total: Option<u64>,
    },
    /// Begin an artifact operation; byte counters reset here.
    Artifact {
        /// Artifact path, never an authenticated URL.
        path: String,
        /// download/resume/reuse/restart/verify/copy/upload/head/delete.
        action: String,
        /// Initial byte offset.
        offset: u64,
        /// Complete length if known.
        expected_bytes: Option<u64>,
    },
    /// Cumulative bytes for the current artifact operation, including resume offset.
    Bytes {
        /// Artifact path.
        path: String,
        /// Cumulative byte count.
        count: u64,
    },
    /// Cumulative progress for one hash pass.
    Hashing {
        /// Local or relative artifact path.
        path: String,
        /// Bytes hashed so far.
        count: u64,
        /// Complete length if known.
        total: Option<u64>,
    },
    /// Independent liveness; unchanged progress sequence indicates stalled work.
    Heartbeat {
        /// Latest phase.
        stage: String,
        /// Latest operation.
        operation: String,
        /// Current path if known.
        path: Option<String>,
        /// Last progress event, excluding heartbeats.
        progress_seq: u64,
        /// Pending cooperative stop.
        cancel_requested: bool,
    },
    /// Estimated aggregate payload coverage; not a verified completion signal.
    TransferSummary {
        /// Logical payload coverage including resumed/reused prefixes.
        completed_bytes: u64,
        /// Newly received bytes in this invocation, excluding local reuse/copy/hash.
        transferred_bytes: u64,
        /// Advertised total, absent if any artifact is unknown.
        expected_bytes: Option<u64>,
        /// Observed network rate, absent before any bytes/time are available.
        bytes_per_second: Option<f64>,
        /// Estimated seconds remaining; absent without known total/rate.
        eta_seconds: Option<f64>,
        /// Always true; byte/hash verification remains separate.
        estimated: bool,
    },
    /// Successful completion of one concurrent path/action operation.
    ArtifactComplete {
        /// Generation-relative artifact path.
        path: String,
        /// Operation that completed, for example download/verify/head/reuse.
        action: String,
        /// Observed byte count, absent for unknown HEAD estimates.
        bytes: Option<u64>,
        /// True only after whole-content hash verification.
        verified: bool,
    },
    /// Whole crate-fetch counters; only verified/reused files count as completed.
    CrateSummary {
        /// Number of fully verified archives.
        files_completed: u64,
        /// Total planned archive count, absent before selection is complete.
        files_total: Option<u64>,
        /// Actual newly received bytes across attempts.
        transferred_bytes: u64,
        /// Reused verified bytes, excluding network transfers.
        reused_bytes: u64,
    },
    /// Bounded transport delay; independent heartbeat continues throughout.
    Retry {
        /// Generation-relative path, not an authenticated URL.
        path: String,
        /// Next one-based request attempt.
        attempt: u64,
        /// Bounded delay before another request.
        wait_seconds: u64,
        /// Stable category for GUI wording.
        kind: RetryKind,
        /// Sanitized human context, never raw headers.
        reason: String,
    },
    /// Last CLI event; a killed process or broken output may lack this event.
    Result {
        /// Whether the operation completed successfully.
        ok: bool,
        /// Command-specific successful data.
        report: Option<toml::Value>,
        /// Typed failure, possibly with partial results.
        error: Option<Error>,
    },
}
/// Thread-safe sink. Implementations must not reenter the same control or block indefinitely.
pub trait Observer: Send + Sync {
    /// Observe one event; output failures stop normal operation.
    fn observe(&self, event: &Event) -> io::Result<()>;
}
struct Quiet;
impl Observer for Quiet {
    fn observe(&self, _: &Event) -> io::Result<()> {
        Ok(())
    }
}
/// Canonical UTF-8 JSON-lines serializer; one flushed object per event.
/// It uses the existing TOML/Serde data model for command reports, without extra dependencies.
pub struct JsonLines<W: Write + Send>(Mutex<W>);
impl<W: Write + Send> JsonLines<W> {
    /// Own a writer; concurrent calls serialize complete lines without interleaving.
    pub fn new(writer: W) -> Self {
        Self(Mutex::new(writer))
    }
}
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
pub(crate) fn json_value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(v) => quote(v),
        toml::Value::Integer(v) => v.to_string(),
        toml::Value::Float(v) if v.is_finite() => v.to_string(),
        toml::Value::Float(_) => "null".into(),
        toml::Value::Boolean(v) => v.to_string(),
        toml::Value::Datetime(v) => quote(&v.to_string()),
        toml::Value::Array(v) => format!(
            "[{}]",
            v.iter().map(json_value).collect::<Vec<_>>().join(",")
        ),
        toml::Value::Table(v) => format!("{{{}}}", {
            let mut fields = v
                .iter()
                .map(|(k, x)| {
                    let json = if k == "artifacts"
                        && v.get("estimated").and_then(toml::Value::as_bool) == Some(true)
                    {
                        if let Some(items) = x.as_array() {
                            format!(
                                "[{}]",
                                items
                                    .iter()
                                    .map(|item| {
                                        let encoded = json_value(item);
                                        if item.get("expected_bytes").is_none() {
                                            format!(
                                                "{},\"expected_bytes\":null}}",
                                                encoded.trim_end_matches('}')
                                            )
                                        } else {
                                            encoded
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join(",")
                            )
                        } else {
                            json_value(x)
                        }
                    } else {
                        json_value(x)
                    };
                    format!("{}:{}", quote(k), json)
                })
                .collect::<Vec<_>>();
            if v.contains_key("cksum")
                && v.contains_key("name")
                && v.contains_key("version")
                && !v.contains_key("expected_bytes")
            {
                fields.push("\"expected_bytes\":null".into());
            }
            if v.contains_key("plan_id")
                && v.contains_key("observed_at")
                && !v.contains_key("previous_inventory_sha256")
            {
                fields.push("\"previous_inventory_sha256\":null".into());
            }
            for (shape, nullable) in [
                (
                    v.contains_key("chains") && v.contains_key("truncated"),
                    &["total_chains"][..],
                ),
                (
                    v.contains_key("known_archive_bytes") && v.contains_key("unknown_archives"),
                    &["expected_archive_bytes"][..],
                ),
                (
                    v.contains_key("root")
                        && v.contains_key("requirement")
                        && v.contains_key("conservative"),
                    &["parent", "target"][..],
                ),
            ] {
                if shape {
                    for key in nullable {
                        if !v.contains_key(*key) {
                            fields.push(format!("{}:null", quote(key)));
                        }
                    }
                }
            }
            if v.contains_key("certificates")
                && v.contains_key("status")
                && !v.contains_key("error_kind")
            {
                fields.push("\"error_kind\":null".into());
            }
            fields.join(",")
        }),
    }
}
impl<W: Write + Send> Observer for JsonLines<W> {
    fn observe(&self, event: &Event) -> io::Result<()> {
        let mut value = toml::Value::try_from(event).map_err(io::Error::other)?;
        // TOML omits None fields; protocol 1 represents them explicitly as JSON null.
        let table = value
            .as_table_mut()
            .ok_or_else(|| io::Error::other("event must be object"))?;
        let nullable: &[&str] = match event.kind {
            EventKind::TransferSummary { .. } => {
                &["expected_bytes", "bytes_per_second", "eta_seconds"]
            }
            EventKind::ArtifactComplete { .. } => &["bytes"],
            EventKind::CrateSummary { .. } => &["files_total"],
            EventKind::Stage { .. } => &["total"],
            EventKind::Artifact { .. } => &["expected_bytes"],
            EventKind::Hashing { .. } => &["total"],
            EventKind::Heartbeat { .. } => &["path"],
            EventKind::Result { .. } => &["report", "error"],
            _ => &[],
        };
        let mut fields = table
            .iter()
            .map(|(k, v)| format!("{}:{}", quote(k), json_value(v)))
            .collect::<Vec<_>>();
        for key in nullable {
            if !table.contains_key(*key) {
                fields.push(format!("{}:null", quote(key)));
            }
        }
        if let EventKind::Result {
            error: Some(error), ..
        } = &event.kind
        {
            if error.report.is_none() {
                fields.retain(|f| !f.starts_with("\"error\":"));
                let mut object = toml::Value::try_from(error)
                    .map_err(io::Error::other)?
                    .as_table()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| format!("{}:{}", quote(k), json_value(v)))
                    .collect::<Vec<_>>();
                object.push("\"report\":null".into());
                object.sort();
                fields.push(format!("\"error\":{{{}}}", object.join(",")));
            }
        }
        fields.sort();
        let mut writer = self.0.lock().unwrap_or_else(|p| p.into_inner());
        writeln!(writer, "{{{}}}", fields.join(","))?;
        writer.flush()
    }
}
#[derive(Clone)]
struct State {
    stage: String,
    operation: String,
    path: Option<String>,
    progress_seq: u64,
    index: u64,
    active: Option<std::collections::BTreeMap<String, String>>,
}
struct Shared {
    observer: Arc<dyn Observer>,
    cancel: CancelToken,
    state: Mutex<State>,
    sequence: Mutex<u64>,
    start: Instant,
}
/// Per-operation context shared by all safe checkpoints and heartbeat delivery.
#[derive(Clone)]
pub struct Control(Arc<Shared>);
impl Default for Control {
    fn default() -> Self {
        Self::new(Arc::new(Quiet), CancelToken::new())
    }
}
thread_local! {static ACTIVE:RefCell<Option<Control>>=const{RefCell::new(None)};}
impl Control {
    /// Combine an observer and a caller-owned cancellation token.
    pub fn new(observer: Arc<dyn Observer>, cancel: CancelToken) -> Self {
        Self(Arc::new(Shared {
            observer,
            cancel,
            state: Mutex::new(State {
                stage: "starting".into(),
                operation: "starting".into(),
                path: None,
                progress_seq: 0,
                index: 0,
                active: None,
            }),
            sequence: Mutex::new(0),
            start: Instant::now(),
        }))
    }
    /// Shared token; cancellation never rolls back committed writes.
    pub fn cancel_token(&self) -> CancelToken {
        self.0.cancel.clone()
    }
    /// Deliver one serialized event; terminal framing is owned by the CLI session.
    pub fn emit(&self, kind: EventKind) -> Result<()> {
        let mut seq = self.0.sequence.lock().unwrap_or_else(|p| p.into_inner());
        let event = Event {
            seq: *seq,
            elapsed_ms: self.0.start.elapsed().as_millis().min(u64::MAX as u128) as u64,
            kind,
        };
        if !matches!(
            event.kind,
            EventKind::Heartbeat { .. } | EventKind::Hello { .. } | EventKind::Result { .. }
        ) {
            self.0
                .state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .progress_seq = *seq;
        }
        self.0.observer.observe(&event)?;
        *seq += 1;
        Ok(())
    }
    /// Run synchronous work with cooperative checkpoints and an independent 1-second heartbeat.
    /// Nested use restores the previous calling-thread context. The observer must be fast.
    /// Errors are normalized to the public [`Error`] while retaining partial reports.
    pub fn run<T>(&self, operation: &str, work: impl FnOnce() -> Result<T>) -> Result<T> {
        struct Restore(Option<Control>);
        impl Drop for Restore {
            fn drop(&mut self) {
                ACTIVE.with(|a| *a.borrow_mut() = self.0.take());
            }
        }
        let _restore = Restore(ACTIVE.with(|a| a.replace(Some(self.clone()))));
        self.stage(operation)?;
        let stopped = Arc::new((Mutex::new(false), Condvar::new()));
        let stop = stopped.clone();
        let control = self.clone();
        let thread = std::thread::spawn(move || {
            let (lock, cv) = &*stop;
            let mut done = lock.lock().unwrap_or_else(|p| p.into_inner());
            while !*done {
                let (next, _) = cv
                    .wait_timeout(done, Duration::from_secs(1))
                    .unwrap_or_else(|p| p.into_inner());
                done = next;
                if *done {
                    break;
                }
                let s = control
                    .0
                    .state
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone();
                if control
                    .emit(EventKind::Heartbeat {
                        stage: s.stage,
                        operation: s.operation,
                        path: s.path,
                        progress_seq: s.progress_seq,
                        cancel_requested: control.0.cancel.is_cancelled(),
                    })
                    .is_err()
                {
                    control.0.cancel.cancel();
                    break;
                }
            }
        });
        struct Stop {
            stop: Arc<(Mutex<bool>, Condvar)>,
            thread: Option<std::thread::JoinHandle<()>>,
        }
        impl Drop for Stop {
            fn drop(&mut self) {
                let (lock, cv) = &*self.stop;
                *lock.lock().unwrap_or_else(|p| p.into_inner()) = true;
                cv.notify_all();
                if let Some(thread) = self.thread.take() {
                    let _ = thread.join();
                }
            }
        }
        let _stop = Stop {
            stop: stopped,
            thread: Some(thread),
        };
        self.check()?;
        work().map_err(crate::error::normalized)
    }
    /// Refuse further work after a stop request.
    pub fn check(&self) -> Result<()> {
        if self.0.cancel.is_cancelled() {
            Err(Error::new(ErrorKind::Cancelled, "operation cancelled").into())
        } else {
            Ok(())
        }
    }
    fn stage(&self, name: &str) -> Result<()> {
        self.check()?;
        let index = {
            let mut s = self.0.state.lock().unwrap_or_else(|p| p.into_inner());
            s.stage = name.into();
            s.operation = name.into();
            s.path = None;
            if let Some(active) = &mut s.active {
                active.clear();
            }
            s.index += 1;
            s.index
        };
        self.emit(EventKind::Stage {
            name: name.into(),
            index,
            total: None,
        })
    }
}
pub(crate) struct Concurrent(Option<Control>);
impl Drop for Concurrent {
    fn drop(&mut self) {
        if let Some(control) = &self.0 {
            let mut state = control.0.state.lock().unwrap_or_else(|p| p.into_inner());
            state.active = None;
            state.path = None;
            state.operation = "idle".into();
        }
    }
}
pub(crate) fn concurrent() -> Concurrent {
    let control = current();
    if let Some(control) = &control {
        control
            .0
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .active = Some(std::collections::BTreeMap::new());
    }
    Concurrent(control)
}
pub(crate) fn worker<T>(control: Option<Control>, work: impl FnOnce() -> Result<T>) -> Result<T> {
    struct Restore(Option<Control>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|a| *a.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(ACTIVE.with(|a| a.replace(control)));
    check()?;
    work()
}
pub(crate) fn complete(path: &str, action: &str, bytes: Option<u64>, verified: bool) -> Result<()> {
    check()?;
    if let Some(control) = current() {
        {
            let mut state = control.0.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(active) = &mut state.active {
                active.remove(path);
                let next = active
                    .first_key_value()
                    .map(|(path, action)| (path.clone(), action.clone()));
                state.path = next.as_ref().map(|(path, _)| path.clone());
                state.operation = next
                    .map(|(_, action)| action)
                    .unwrap_or_else(|| "idle".into());
            }
        }
        control.emit(EventKind::ArtifactComplete {
            path: path.into(),
            action: action.into(),
            bytes,
            verified,
        })?;
    }
    Ok(())
}
pub(crate) fn current() -> Option<Control> {
    ACTIVE.with(|a| a.borrow().clone())
}
pub(crate) fn check() -> Result<()> {
    if let Some(c) = current() {
        c.check()?;
    }
    Ok(())
}
pub(crate) fn stage(name: &str) -> Result<()> {
    if let Some(c) = current() {
        c.stage(name)?;
    }
    Ok(())
}
thread_local! {static SIZE_HINTS:RefCell<std::collections::BTreeMap<String,u64>>=const{RefCell::new(std::collections::BTreeMap::new())};}
pub(crate) struct SizeHints(std::collections::BTreeMap<String, u64>);
impl SizeHints {
    pub(crate) fn new(hints: std::collections::BTreeMap<String, u64>) -> Self {
        Self(SIZE_HINTS.with(|s| s.replace(hints)))
    }
}
impl Drop for SizeHints {
    fn drop(&mut self) {
        SIZE_HINTS.with(|s| *s.borrow_mut() = std::mem::take(&mut self.0));
    }
}
pub(crate) fn artifact(path: &str, action: &str, offset: u64, total: Option<u64>) -> Result<()> {
    check()?;
    let total = total.or_else(|| {
        if ["download", "resume", "restart"].contains(&action) {
            SIZE_HINTS.with(|s| s.borrow().get(path).copied())
        } else {
            None
        }
    });
    if let Some(c) = current() {
        {
            let mut s = c.0.state.lock().unwrap_or_else(|p| p.into_inner());
            s.path = Some(path.into());
            s.operation = action.into();
            if let Some(active) = &mut s.active {
                active.insert(path.into(), action.into());
            }
        }
        c.emit(EventKind::Artifact {
            path: path.into(),
            action: action.into(),
            offset,
            expected_bytes: total,
        })?;
    }
    Ok(())
}
pub(crate) fn bytes(path: &str, count: u64) -> Result<()> {
    check()?;
    if let Some(c) = current() {
        c.emit(EventKind::Bytes {
            path: path.into(),
            count,
        })?;
    }
    Ok(())
}
pub(crate) fn hashing(path: &str, count: u64, total: Option<u64>) -> Result<()> {
    check()?;
    if let Some(c) = current() {
        {
            let mut s = c.0.state.lock().unwrap_or_else(|p| p.into_inner());
            s.path = Some(path.into());
            s.operation = "hash".into();
            if let Some(active) = &mut s.active {
                active.insert(path.into(), "hash".into());
            }
        }
        c.emit(EventKind::Hashing {
            path: path.into(),
            count,
            total,
        })?;
    }
    Ok(())
}
/// Writer adapter that checks cancellation and emits cumulative transfer/copy progress.
pub(crate) struct ProgressWriter<'a> {
    pub(crate) inner: &'a mut dyn Write,
    pub(crate) path: String,
    pub(crate) count: u64,
}
impl Write for ProgressWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        check().map_err(io::Error::other)?;
        let n = self.inner.write(&buf[..buf.len().min(64 * 1024)])?;
        self.count += n as u64;
        bytes(&self.path, self.count).map_err(io::Error::other)?;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
