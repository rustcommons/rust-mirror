# Events protocol 1

This is the owner-approved CLI/GUI contract for rust-mirror 0.2.0, approved
2026-11-11. **Published in 0.2.0; 0.1.x does not implement it.**
Consumers may launch the rust-mirror binary or embed the public library in a
worker child process of their own executable, using the shared event serializer.
Both routes use this protocol without parsing human diagnostics or reproducing
mirroring logic.

## Invocation and discovery

```sh
rust-mirror --events jsonl --version
rust-mirror --events jsonl --cancel-stdin sync --config mirror.toml --output run-001
rust-mirror --events jsonl publish --preflight --source generation-001/mirror --base-url https://nexus.example/repository/rust-new
```

`--events jsonl` is global and opt-in. Each stdout line is one UTF-8 JSON object,
terminated by LF; stdout has no human text in this mode. Diagnostics remain on
stderr. Omit the flag for the existing human CLI. Supported commands include
init/list/import/plan/fetch/sync/assemble/rustup/publish/generations/prune and the
legacy fetch-toolchain command. Help/version are local, cheap discovery operations.

A normal stream starts with `hello`, ends with exactly one `result`, and contains
no events after the result. The first event identifies protocol and backend version.
The version command's successful report repeats `protocol` and `tool_version`.
Do not infer a library ABI or a release from the protocol version.

## Common envelope

Every event contains `type`, `seq` (strictly increasing, starting at 0), and
`elapsed_ms` (monotonic milliseconds since this command's event session began).
Unknown additive fields and unknown nonterminal event types must be ignored.
Reject an unsupported protocol major; protocol 1 is the integer `1`. Existing
fields/types/units cannot change meaning within protocol 1. Breaking changes
require a new protocol version. Tool release versions and protocol versions differ.
Each event is flushed promptly. Consumers must bound their input buffers; event
reports can include inventories and exact pruning plans, not merely short messages.

## Event fields

| type | Additional fields | Meaning |
| --- | --- | --- |
| `hello` | `protocol`: integer, `tool_version`: string, `command`: string | First event; no network needed |
| `stage` | `name`: string, `index`: integer, `total`: integer or null | Named workflow phase; index starts at 1; total null when unknown |
| `artifact` | `path`: string, `action`: string, `offset`: integer, `expected_bytes`: integer or null | Begin an operation; action is download/resume/reuse/restart/verify/copy/upload/head/delete; unknown future actions are displayable strings |
| `bytes` | `path`: string, `count`: integer | Cumulative bytes for the current operation on this path, including its resume offset; reset on a new artifact event, not a global transfer total |
| `hashing` | `path`: string, `count`: integer, `total`: integer or null | Cumulative bytes hashed for the current hash pass; starts at 0 and ends at total on success |
| `heartbeat` | `stage`: string, `operation`: string, `path`: string or null, `progress_seq`: integer, `cancel_requested`: boolean | Latest declared work, not proof of completed I/O or freshness |
| `result` | `ok`: boolean, `report`: object or null, `error`: object or null | Terminal outcome; exactly one of successful report or error is present |

Sizes/counts/offsets are nonnegative byte integers. Unknown sizes are null, never
zero estimates. Paths are filesystem paths or relative mirror artifact names, not
credential-bearing URLs. Hashing/copying/reuse and upstream transfers have distinct
operations; byte events must not be summed across them as network traffic.

The heartbeat is emitted by an independent thread about once per second, including
during blocked network reads and long hashing/copying. `progress_seq` is the last
non-heartbeat progress event sequence, so continued heartbeats with unchanged
progress can be distinguished from loss of the backend itself. Before any stage,
heartbeat stage/operation is `starting`; an unknown current path is null.
A heartbeat describes the most recently declared work; it cannot establish that a
blocked syscall made progress. OS suspension, process death, blocked stdout or an
unresponsive event sink can prevent delivery. The GUI owns its monotonic watchdog,
stall warning, grace period and eventual process kill; no network timeout substitutes
for that watchdog. It must keep draining both output pipes to avoid pipe deadlock.

## Reports and typed errors

`result.report` is command-specific JSON data. Plans, catalogs, publication/preflight
reports and retention plans/apply reports contain the same fields as their library
counterparts. Path-returning preparation commands use `{"mirror":"PATH"}`;
init/import use `{"path":"PATH"}`. Sync additionally includes its structured
`report` alongside `mirror`. Help returns `{"text":"..."}`. Version returns
`{"tool_version":"0.2.0","protocol":1}` for the 0.2.0 binary.

An unsuccessful result has `report: null` and:

```json
{"kind":"cancelled","message":"operation cancelled","report":null}
```

`error.kind` is a stable machine identifier: `output_exists`, `resume_mismatch`,
`locked`, `network`, `integrity`, `budget`, `remote_exists`, `auth`, `tls`, `cancelled`,
`invalid`, `io`, or `internal`. Unknown future kinds are generic failures.
`message` is human context, never a field to parse. `error.report` optionally carries
partial-operation evidence (not success), notably the successful prefix of pruning.
Credentials, authorization headers and private key bytes never belong in events.

Exit codes are 0 for success, 1 for failure, 130 for cooperative cancellation.
Force termination, OS failures and broken output pipes may leave no terminal result;
the GUI must not synthesize success from an earlier event or exit alone.

## Stop, cancellation and recovery

With `--cancel-stdin`, keep stdin open while working. Send the control record
`{"type":"cancel"}` followed by LF, or close stdin to request cancellation.
Whitespace between its literal tokens is allowed; other records are ignored.
Without this flag stdin is not a control channel and EOF does not cancel.
Ctrl-C and supported process-stop signals request the same cancellation token.
Uncatchable termination cannot emit a result. A cancellation request is idempotent.

Cancellation is checked between files and within transfer/copy/hash loops. A blocked
synchronous request/read may not return before its transport timeout; heartbeats
continue while it waits. After a user-defined grace period the GUI may kill the
process. The final cancelled result means the operation stopped, not that writes
were rolled back. Recovery is operation-specific:

- Fetch/sync keep initialized private frozen-plan state for explicit `--resume`.
  If stopped before initialization committed, inspect the output and use a new
  directory or the documented explicit restart route; never fabricate saved state.
- Assemble/rustup preserve completed inputs and previous generations. Unfinished
  staging is not served. Retry with a new output directory; these operations do not
  claim resumable in-place assembly.
- Publish/preflight preserve local inputs. Preflight makes no remote writes.
  Cancelled/interrupted publication can leave partial remote content. Inspect it
  and use a fresh prefix; no remote resume, overwrite, rollback or deletion is implied.
- Pruning can already have unlinked approved files. Its error report identifies the
  successful prefix when available; a killed process can lose that report. Incomplete
  snapshots fail validation. Inspect/restore them explicitly before another preview.

## Publish preflight and trust

`publish --preflight` and the library preflight API validate local closure,
URL/CA/credential policy, TLS/auth responses and absence of every remote path,
including the late inventory marker. They perform no PUT or DELETE and do not
change server settings. A successful HEAD check is not proof that the server will
permit a later PUT. Actual publish repeats preflight immediately before writing;
preflight is not a reservation or an overwrite token. Publication emits upload and
GET verification progress, verifies bytes, and sends the inventory marker last.
Keep no-redeploy server policy and exclusive publishers. Credentials remain in
`NEXUS_USERNAME`/`NEXUS_PASSWORD` for the CLI; the library takes explicit options.

## Rust embedding API

The 0.2.0 public names are `events::{PROTOCOL_VERSION, Event, EventKind,
Observer, CancelToken, Control, JsonLines}`, root `ErrorKind`, `Error` and
`error_kind`, plus `cli::run_with_events(args, writer, cancel)` for a shared command
adapter. `JsonLines<W: Write>` is the canonical serializer so embeddings need not
reproduce encoding. Controlled operations use additive `_with_control` variants:
`sync::{run_with_control, run_forced_with_control}`, `selection::{fetch_with_control,
fetch_resuming_with_control}`, `collection::assemble_with_control`,
`rustup::prepare_with_control`, `publish::{preflight_with_control,publish_with_control}`
and retention preview/apply variants. Existing convenience calls remain usable.

Observers receive structured progress and heartbeats; callbacks must be quick,
thread-safe and must not reenter the same control. The library does not install
process signal handlers. A CLI session owns hello/result framing, signal/control
input handling and final flushing; library-only callers use the same event types,
control and serializer. A source implementation does not establish registry publication; publication is
verified separately in the release record.

<a id="planned-030-additive-network-operations"></a>

## 0.3.0 additive network operations

The [corporate-network contract](NETWORK.md) specifies public API names,
configuration, secret handling, certificate diagnostics, estimates and caching.
These additions are implemented in the unreleased 0.3.0 candidate and retain
protocol 1; 0.2.0 does not implement them.

`check-network`, `releases` and opt-in size measurement use the existing hello,
stage/artifact/bytes/heartbeat and final typed-result framing. Check failures use
`error.report` for the ordered diagnostic report; success returns it in `report`.
Proxy/environment/certificate details in reports are bounded and sanitized.
No credential values, authorization headers or private keys enter events. A TLS
error can include only certificates actually presented; it never changes trust.

An additive `transfer_summary` event carries `completed_bytes`,
`transferred_bytes`, nullable `expected_bytes`, nullable `bytes_per_second`, nullable
`eta_seconds` and `estimated: true`. Completed bytes represent logical transfer
coverage including resumed prefixes; transferred bytes count newly received bytes
in this invocation, excluding reuse/copy/hash. Unknown whole-operation totals and
rate/ETA remain null. Consumers must not treat an estimate as verified completion.
Existing `artifact.expected_bytes` continues to describe that artifact operation.
Unknown additive events/fields remain ignorable; no existing meaning changes.
