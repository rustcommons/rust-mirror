# Curated crate mirroring design

**Owner review draft, 2026-11-12. Design only.** rust-mirror 0.3.0 mirrors
Rust toolchains, not Cargo registries. None of the commands, configuration extensions
or library items proposed here exists yet. Approval of this document would select a
design; it would not authorize dependencies, implementation, publication, or writes
to a real Nexus. See [requirements](REQUIREMENTS.md) and [roadmap](ROADMAP.md).

## Purpose and boundaries

Maintain an organization-wide approved catalog spanning many projects, with several
versions of a crate available simultaneously. It is deliberately larger than any
one project's dependency graph. Selection is an administrator declaration; offline
clients cannot report which versions must survive. The output is a read-only,
curated subset of crates.io, suitable for disconnected static HTTPS hosting.

The catalog approves direct roots and authorizes their explained dependency closure;
it is not a license, vulnerability or build-compatibility certification. A union of
individually resolved roots need not be buildable together. Existing project locks
can require versions outside that union; report the missing coverage, never fall
back silently to the Internet. User-supplied `Cargo.lock` import is deferred, not an
MVP prerequisite. Internal resolver lockfiles are a different implementation detail.

## 1. Catalog syntax and selection

### Decision: Cargo dependency values, separate mirror rules

Use a dedicated catalog file, keeping toolchain selections unchanged. The `[crates]`
table accepts Cargo's stable **registry dependency declaration syntax**: strings,
inline tables and expanded tables, version requirements, `package` aliases,
`features`, `default-features`, and `optional`. Validate against an explicitly tested
Cargo schema/version baseline, not an independently invented version grammar.
Cargo feature and requirement semantics apply, including prerelease opt-in and
comma-separated intersections. [Cargo dependency declarations](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
are the syntax reference.

Proposed catalog, with illustrative policy values rather than approved defaults:

```toml
[crates]
serde = "1"
tokio = { version = "1.40", features = ["full"], default-features = false }
rand_08 = { package = "rand", version = "0.8" }
rand_09 = { package = "rand", version = "0.9" }
serde_legacy = { package = "serde", version = "=1.0.197" }

[crate-rules.serde]
latest = 3
pins = ["1.0.197"]

[crate-rules.rand_08]
latest = 2

[crate-output]
base-url = "https://mirror.example/generations/004/cargo/"

[crate-policy]
features = "declared"
resolver = "2"
hosts = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"]
targets = ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"]
```

A rule is keyed by the dependency alias, not the canonical package name. Aliases
supply several independent ranges/features for one package without violating
Cargo syntax or TOML's unique-key rule. `latest` selects the highest N non-yanked
versions satisfying that alias's requirement; proposed default N is 1. It is not
N major versions. Explicit `pins` add exact versions, even outside the range,
using that alias's feature profile. Use a separate alias when an older pin needs
different features. Select first, validate features for every selected version,
and fail with a diagnostic rather than silently reducing N. An exact requirement
such as `=1.0.197` is itself a pin. Wildcard or omitted versions retain Cargo's
meaning, subject to the same bounded selection; array-valued dependencies and
custom OR strings are refused. Overlapping rules deduplicate payloads by canonical
registry/name/version/checksum while preserving every selection reason.

Every entry is a curated root, including an entry pasted with `optional = true`.
That flag describes its source declaration; it does not disable organizational
approval or make the root disappear. Resolver wrappers explicitly activate that
root. Feature names and `default-features` still control its requested profile.
The editor must explain this curation distinction before saving.

### Paste and refusal behavior

Accept a standalone `[dependencies]` block as paste input: preview a header-only
conversion to `[crates]`, preserve dependency values, and show diagnostics before
saving. Do not treat an entire project manifest as a catalog or infer its workspace,
patches, target tables, resolver or feature definitions. Those need explicit policy.

Refuse `git`, `path`, `branch`, `tag` and `rev`, including combinations that also
contain `version`; Cargo's registry fallback for publishing is not permission to
replace the original source. Refuse unresolved `workspace = true` with an
inheritance explanation. Accept the `registry` field's Cargo syntax but require
that it resolve to crates.io in the MVP; named alternate/private registries are
unsupported with a field-specific diagnostic, not silently redirected. Unknown,
unstable or newer-schema fields fail clearly against the documented baseline.
No local dependency traversal, Git invocation or credential-bearing URL is allowed.
Apply registry-identity refusal to transitive index edges as well: normalize only
the official crates.io identities to the broker; reject other registry URLs before
Cargo sees them. No dependency record may introduce an uncontrolled source.

Rejected: `serde = ["1", "=..."]` inside `[crates]` breaks paste compatibility; a catalog
containing only one lockfile underserves the organization; one giant synthetic
workspace can falsely reject independently useful versions through unification or
native-library `links` conflicts.

### Future pin sources

Normalize inputs into `SelectionSource { id, kind, digest, roots }`. A root carries
registry identity, real name, alias, requirement or exact pin, feature profile and
provenance. Future lockfile imports contribute exact pins with original checksums
and import identity; they merge with manual sources, never replace the catalog.
Removing one source does not remove another source's protection or delete retained
bytes. Conflicting checksums for the same identity fail. A lockfile supplies neither
an organization's full approval list nor complete feature/host policy. No lockfile
import command/parser is included in this proposal's MVP.

## 2. Dependency closure and resolver

### Decision: Cargo resolves; planning controls its inputs and I/O

Use an installed, explicitly selected Cargo executable as the initial resolution
engine, not a new Cargo library dependency or a handwritten greedy resolver. Record
its exact version and executable identity, resolver edition, policy and index-input
hashes in the frozen plan. The library owns the subprocess; the GUI adds no resolver
logic. This is a proposed new planning prerequisite requiring owner acceptance.
Fetch/replay of an already frozen plan must not require Cargo or invoke resolution.

Resolve each selected exact root/profile in its own synthetic project with the
root as a registry dependency. Union the resulting resolved versions and dependency
edges. Do not combine all catalog entries into one Cargo workspace. Each witness
records a coherent solution; cross-root combinations remain the consuming project's
responsibility. Cargo can backtrack and unify versions; independently taking the
newest match for every edge is insufficient. See [Cargo resolution](https://doc.rust-lang.org/cargo/reference/resolver.html).

Run only `cargo generate-lockfile` in a private temporary project and isolated
`CARGO_HOME`, outside user configuration discovery. No build, test, install,
`cargo metadata` download, build script or proc-macro execution belongs in planning.
Remove inherited Cargo overrides, proxies, credentials and source configuration;
provide only the intended resolver configuration. Do not execute a Cargo binary
named by an untrusted catalog. Resolve its location through explicit application
configuration and display its identity.

Cargo's only registry endpoint is a loopback sparse **metadata broker** owned by
the backend. It retrieves only requested crates.io index files through
`NetworkOptions`, saves their original bytes, and serves a bounded, frozen view.
Its generated `dl` endpoint refuses every archive request. Empty/synthetic source
files exist only for the local wrapper; no real crate archive is unpacked. It never forwards arbitrary
URLs. This avoids relying on Cargo's private cache format and keeps proxy secrets
inside the existing backend network boundary. Keep the broker loopback-only and
scope its lifetime to the planning session; it is not a mirror server feature.

The inspected Cargo implementation resolves and writes a lockfile separately from
package downloading; this supports the approach, but does not qualify it for this
repository. [Cargo generate-lockfile source](https://github.com/rust-lang/cargo/blob/0a2d4c697121839929f080e539d3ccac11e4375d/src/ops/cargo_update.rs)
and [resolve/download separation](https://github.com/rust-lang/cargo/blob/0a2d4c697121839929f080e539d3ccac11e4375d/src/ops/resolve.rs)
are the inspected references. The first implementation gate must prove **zero
archive requests** with the chosen Cargo release. An unexpected payload request
fails planning; do not weaken the rule to make the subprocess succeed. The ordinary
Cargo command is documented in [generate-lockfile](https://doc.rust-lang.org/cargo/commands/cargo-generate-lockfile.html).

### Features, platforms and dependency kinds

| Dimension | Proposed MVP decision and consequence |
| --- | --- |
| Features | Honor each root's declared defaults and enabled features. Follow Cargo's feature propagation, including optional dependencies, `dep:` and weak forwarding. Do not activate all optional dependencies of all transitive crates. Missing requested features fail that root. |
| Optional dependencies | Include the optional edges activated in the resolution witness, plus everything Cargo needs for its lock resolution. A later project enabling an uncurated feature may need additional crates. The preview must state this coverage boundary. |
| Targets | Record selected host/compiler and compilation target pairs, but retain Cargo's complete all-platform lock closure in the MVP. Do not strip an index dependency or omit a locked archive merely because it is inactive on the mirror machine. This conservative overfetch avoids metadata-only index entries for unavailable payloads. |
| Build dependencies | Include their closure and host-side proc-macro/build requirements. A host is not interchangeable with a target; cross-compilation does not remove host dependencies. |
| Development dependencies | Exclude a registry root's own tests/examples/dev dependencies: it is a dependency of the synthetic project, not a workspace member. Consumer-project dev tools must be explicit catalog roots. |
| Compiler compatibility | Freeze the Cargo/resolver and declared Rust-version context; expose upstream `rust_version` where present. Reject a selected version explicitly exceeding a configured hard compiler ceiling; unknown values remain warnings. Resolution alone proves neither compilation nor native SDK availability. |

Cargo distinguishes lockfile resolution from build-time features; resolver 2 also
separates some host/build and target feature activation. The
[feature reference](https://doc.rust-lang.org/cargo/reference/features.html) defines
feature forwarding. All optional dependencies are rejected as the default because they can greatly
expand downloads beyond approved feature use without proving build compatibility.
A future `all-optional` coverage mode would create additional
bounded root/feature scenarios, not equate `--all-features` with every possible
optional edge or a successful build. Enabled-only archive minimization by target
is also deferred until actual Cargo client probes establish safety.

### Explosion control and the completeness gate

For each root version, use Cargo's newest-compatible **coherent solution**, allowing
its necessary backtracking and multiple incompatible dependency versions. Union the
witnesses; do not download every version matching every transitive requirement.
Keep all versions already retained in the previous generation and their recorded
closures. A new root/profile requires a new witness; unchanged retained data does
not acquire a claim of fresh resolution merely by being copied.

Before approving a plan, replay every new witness against the **filtered proposed
index**, which contains only planned and retained versions. If this cannot recreate
a coherent solution, block the plan with the root/constraint explanation. Do not
emit a plausible but incomplete manifest or automatically expand to all upstream
versions. A union may cause Cargo to choose a different version; the replay gate
must verify that any chosen version and closure is available, not promise identical
selection for arbitrary projects. Client `--locked` cases need explicit pin coverage.

Bound root versions, crates, resolved versions, edges, witness count, metadata
bytes, per-file bytes, total final bytes, transferred bytes, planning time and
subprocess output. Report the first exceeded bound and its inclusion chain.
Counts and paths are exact in the preview. Archive lengths come from opt-in HEAD
requests: distinguish known total, unknown count, retained bytes and additional
transfer estimate. A complete byte total is impossible when lengths are unknown;
show that explicitly and require an approved cap/unknown-size policy before fetch.
Enforce actual streamed bytes regardless of estimates. Deduplication reduces
transfer, not the logical final-generation budget. Never prune to fit a budget.

### Yanked versions and metadata changes

Exclude yanked versions from new floating selections. The MVP refuses a new yanked
pin even if its archive is reachable: Cargo does not normally select it for a new
lock. Supporting a fresh historical yanked pin requires separate resolution design.
An already verified, retained yanked version and its frozen closure can remain for
existing locks; show its state prominently, without changing upstream flags.

Archive bytes/checksums are immutable identities. A new generation can incorporate
a freshly observed yank/unyank by selecting that new upstream line byte-for-byte;
an old generation retains its old observation. Never edit `yanked` locally to make
Cargo select a version. Changed checksum or dependency/feature content for an
existing identity is an integrity conflict, not a routine update. A frozen plan
uses the captured observation, not a later upstream yank state.

## 3. Upstream, trust and budgets

Use `https://index.crates.io/` for individual sparse records and
`https://static.crates.io/crates/{crate}/{crate}-{version}.crate` for payloads.
Do not clone the full Git index, enumerate the entire registry or scrape the web
API for dependency metadata. Extend the existing upstream allowlist explicitly for
these two origins; toolchain requests retain their current origin rules. Do not
accept arbitrary upstream `dl` templates or redirects. Validate names, shard paths,
versions and canonical destination collisions, including Windows case behavior.

Reuse [0.3.0 NetworkOptions](NETWORK.md): direct by default, explicit environment
opt-in/manual proxy, exclusions, programmatic/stdin secrets, additive CA trust and
connect/read timeouts. Publisher settings remain separately configured. No secret
enters the catalog, broker URL, frozen plan, argv, event, report or diagnostic.
Verify each `.crate` SHA-256 against the frozen index `cksum` before committing it.
TLS plus index checksums is not an independent signature or malware assessment.
Do not extract or execute archives in mirroring; any future archive inspection must
have path, decompression and resource limits.

The current [crates.io data-access policy](https://github.com/rust-lang/crates.io/blob/07a026ab6cb3a93d117c8079fceb52be800826ea/svelte/src/routes/data-access/+page.svelte) recommends
sparse access for a small set and direct CDN downloads. It currently specifies no
rate limit for those two services; the API has a maximum of one request per second
and requires an identifying User-Agent, with contact information strongly suggested.
This design uses no API calls. Send an identifying User-Agent with a configurable
organization contact anyway; show its effective value before requests. Start with
conservative bounded concurrency (proposed four requests), configurable pacing,
conditional metadata requests, bounded backoff and `Retry-After` handling. Recheck
policy before implementation/release. CDN limits must not be confused with the API
limit. An operator asking for most/all crates should evaluate the documented bulk
Git-index/database-dump routes separately; bulk ingestion is out of scope. The
[policy source inspected on 2026-11-12](https://github.com/rust-lang/crates.io/blob/07a026ab6cb3a93d117c8079fceb52be800826ea/svelte/src/routes/data-access/+page.svelte)
preserves the source behind the dynamic page.

## 4. Static registry output and Cargo clients

### Decision: immutable sparse generations on Raw or static HTTPS

Proposed sibling layout; this does not extend existing inventory schemas implicitly:

```text
generation-004/
  toolchains/                 # optional existing Rustup tree, unchanged internally
  cargo/
    index/config.json
    index/se/rd/serde         # only mirrored versions' original JSON lines
    index/to/ki/tokio
    crates/serde/serde-1.0.197.crate
  inventory.toml             # future schema discriminates toolchain/crate entries
```

The parent can describe independent toolchain and crate snapshots without requiring
a toolchain refresh for every crate update. Existing schema 1/2/3 toolchain trees
remain readable and retain their own paths; a new crate-capable schema/version needs
explicit validation and old-reader refusal before implementation. Do not wrap or
rewrite a previously published toolchain tree in place.

Illustrative `cargo/index/config.json` for a new immutable public client URL:

```json
{"dl":"https://mirror.example/generations/004/cargo/crates/{crate}/{crate}-{version}.crate"}
```

`api` is omitted for a read-only mirror; Cargo publish/search are not offered.
`dl` is local to this generation, never crates.io. Use Cargo's standard lowercase
shard rules. Filter each upstream index file to **only versions whose complete
verified archives are in the generation**; preserve every selected JSON line's
bytes and ordering, including unknown fields, schema/features2, checksum and yank
state. Do not parse and reserialize those lines or copy index entries without
payloads. Preserve the record bytes and delimiters; reject malformed records and
unsupported semantic schema versions rather than ignoring required semantics.
The generated config and inventory are local metadata, clearly distinguished from
upstream-preserved records. See [Cargo index format and sparse protocol](https://doc.rust-lang.org/cargo/reference/registry-index.html).

Keep exact index observations privately for frozen-plan replay, including versions
not selected; these are planning inputs, not public registry entries. An inventory
records selected line hashes, archive hashes/sizes, provenance, witness graphs,
coverage policy, and previous-generation identity. Generate config for its final
base URL before freezing/publishing: changing the base URL changes generated bytes
and the generation identity. A missing public index path returns 404, never a live
upstream fallback, login page with HTTP 200, or a proxy-filled registry.

### Client configuration: Windows and Unix

For either platform, explicitly create this project-local `.cargo/config.toml`
after reviewing any existing file; do not overwrite an organization's configuration:

```toml
[source.crates-io]
replace-with = "org-mirror"

[source.org-mirror]
registry = "sparse+https://mirror.example/generations/004/cargo/index/"
```

The trailing slash is intentional. This configuration replaces crates.io sources
while retaining their identity for existing dependencies/locks; merely adding
`[registries.org-mirror]` does not redirect ordinary crates.io dependencies.
Source replacement requires identical upstream code; it is unsuitable for modified
crates/private forks. See [Cargo source replacement](https://doc.rust-lang.org/cargo/reference/source-replacement.html).

With the replacement file in the project, example Unix client commands are:

```sh
cargo fetch --locked
cargo build --locked
# Once this client's cache is populated, with network disabled:
cargo build --locked --offline
```

PowerShell uses the same TOML and the same commands:

```powershell
cargo fetch --locked
cargo build --locked
cargo build --locked --offline
```

These are proposed client workflows, not executed validation. `--offline` disables
access to the internal mirror too; a fresh client first fetches over the internal
network. `--locked` requires an existing compatible project lock and all its pinned
versions. Client trust/proxy setup is Cargo's own, separate from the connected
machine's `NetworkOptions`. The MVP proposes anonymous read access behind a trusted
corporate HTTPS endpoint; authenticated Cargo readers require a separate credential-
provider/auth-required design, not publisher credentials embedded in config.

### Publication, activation and rollback

Extend [existing publication rules](NEXUS-RAW.md) to the new inventory only after
qualification: full local verification, no-write remote preflight for every path,
new absent destination, HTTPS, explicit credentials, bounded upload progress,
GET hash verification and inventory last. No overwrite, automatic Nexus setting
changes or remote DELETE. A partial upload stays inactive; use a new prefix after
failure according to current rules. Preflight is not a transaction or proof of
future permission. A generic static host needs an equivalent explicit deployment
adapter/contract; current PUT publication does not support every hosting vendor.

Completion markers alone do not stop a client reading a half-uploaded index.
Publish to an unadvertised generation URL, then have an administrator explicitly
select that URL after verification. Default to generation-pinned Cargo URLs: a
stable front-end routing switch risks mixing cached index/config and archives.
Future atomic activation must pin `dl` to an immutable generation and prove cache
and concurrent-client behavior; a Nexus Raw group switch is not assumed atomic
for Cargo. Retain old generation URLs and rollback by explicit client/routing
selection. Toolchain `RUSTUP_DIST_SERVER` and Cargo replacement remain separate
settings even if both use the same server. This proposal does not alter current
Rustup activation behavior.

### Why not a native Nexus Cargo repository?

Sonatype now documents hosted, proxy and group Cargo repositories with sparse
support; claiming Nexus lacks Cargo support would be wrong. Its hosted upload path
is `cargo publish`, not arbitrary Raw PUT, and it has its own auth/index lifecycle.
See [Sonatype Cargo documentation](https://help.sonatype.com/en/rust-cargo.html).
It is a valid alternative for an organization choosing a registry service, but
needs separate verification of curated offline population, upstream identity,
retention, immutability and exact index-line preservation. A proxy cache can miss
uncached versions once disconnected. Choose Raw/static output here for portable,
air-gap-transferable exact trees and reuse of explicit no-overwrite publication;
do not claim existing Raw tests qualify the native format or this new crate format.

## 5. Incremental updates, resume and retention

Planning refreshes only roots and discovered closure metadata, saving original
responses with hashes, validators and observation times. A multi-file sparse fetch
is not an atomic global crates.io snapshot; the plan names the exact observations
it actually used. New upstream versions appearing after freeze do not change the
plan. An explicit re-plan discovers them and produces a new preview/identity.
Offline planning may use an explicitly chosen complete cache with reported age;
a missing closure record fails, never mixes in an undocumented live fetch.

New output starts from the previous complete generation's validated retained set,
then adds selected versions and their closure. A smaller latest-N rule or removal
of a root does not delete existing mirrored versions. Record all retention reasons,
including which original root/witness still needs a transitive version. On a new
plan, freshly resolved witnesses can add newer transitive versions; old witnesses
remain available with their retained roots. Reuse existing archives only after
rehashing. No shared writable cache or hardlink mutation is implied.

Reuse the [resume contract](RESUMING.md): explicit resume, exclusive OS writer lock,
private `.download` state, exact config/previous/plan identity, bounded validated
ranges, whole-file hashes, new immutable completed output and no live metadata
refresh on resume. Cancellation checkpoints cover broker I/O, resolver subprocess,
copy/hash/download loops; stop and reap only the subprocess we started. Kill/resume
must preserve initialized frozen state. The library returns typed errors and a
structured report rather than relying on stderr.

Extend [whole-generation retention](RETENTION.md), not in-place deletion of records
from an active sparse index. Keep current/next-previous, pins, paused runs and rollback
protection; prune only with an exact paths/bytes preview, explicit apply and stale-
state refusal. Crate root pins protect generations containing their full witness
closure. Existing toolchain retention must not misinterpret a new crate inventory.
Crate-aware catalog matching is a new requirement, not already implemented reuse.
Within-generation crate compaction and remote pruning remain deferred; building a
smaller successor and retiring whole old generations needs separate explicit policy.
No real Nexus deletion is authorized.

## 6. CLI, library and GUI sketch

All names below are proposals, not current commands or compilable API examples.
Use a `crates` command namespace rather than change existing toolchain meanings:

| Proposed command | Effects and structured result |
| --- | --- |
| `crates plan --config catalog.toml --output plan.toml [--previous PATH] [--measure-sizes]` | Metadata/optional HEAD only; frozen roots, witnesses, reasons, warnings, exact paths/counts and known/unknown size totals. Never archive GET. |
| `crates explain --plan plan.toml --crate NAME [--version VERSION]` | Local inclusion paths back to catalog aliases, pins and parent dependency/feature/target edges. |
| `crates fetch --plan plan.toml --output RUN [--resume]` | Frozen downloads plus retained-data verification; returns a complete local crate generation. No publish/activation/prune. |
| `crates sync --config catalog.toml --output RUN [--previous PATH] [--resume]` | One-shot plan/fetch; first emits a reviewable preview. Requires explicit budget/policy acceptance, never implicit publication. Resume reuses frozen state. |
| `publish --preflight ...` / `publish ...` | Existing command extended explicitly to crate-capable inventories; separate no-write and write routes. |
| `generations` / `prune` | Existing family extended only after crate-aware protection and schema qualification. No new remote delete route. |

Library sketch: `crates::Catalog`, `CrateRule`, `SelectionSource`, `CoveragePolicy`,
`CratePlan`, `InclusionReason`, `CrateGenerationReport`; functions
`crates::plan_with_control`, `explain`, `fetch_with_control`, `sync_with_control`.
Accept owned/borrowed typed configuration and explicit `NetworkOptions`, existing
`events::Control`/observer/cancellation, and return serializable reports or existing
public `Error`/`ErrorKind`. Inspection/explanation is local and cheap. Freeze serializable
plans with a schema and digest; never make a GUI keep opaque in-memory state just
to resume. Publish/prune integrations must preserve their existing controlled APIs.
The external Cargo prerequisite belongs to planning options; do not read arbitrary
program names, homes or environments from a saved plan when replaying it.

[Protocol 1](EVENTS.md) framing stays unchanged: hello first, independent heartbeat
with current stage/operation during Cargo/network/hash waits, artifact/bytes/hashing
progress, one final typed result. Proposed command labels `crates-plan`,
`crates-fetch`, `crates-sync`, `crates-explain` and stages `resolve`, `closure-check`,
`measure`, `download`, `verify`, `assemble` are additive. Reuse artifact
`expected_bytes` and transfer summaries; unknown sizes stay null. Result reports
add schema-tagged `roots`, `versions`, `reasons`, `coverage`, `size_estimate` and
`warnings`. Reuse `invalid`, `budget`, `integrity`, `network`, `tls`, `auth`,
`locked`, `resume_mismatch`, `cancelled` and `remote_exists`; include structured
constraint context without asking clients to parse Cargo prose. Bound and redact
subprocess output. Land the actual additive protocol/API specification before GUI
implementation; this draft does not silently amend EVENTS.md or promise API stability.

GUI pages needed: organization catalog editor with paste/validation/diff; per-alias
version/range/pin/feature rules; host/target and coverage policy; preview with counts,
unknown sizes/budgets and expandable “why included” chains; sync progress/stop/resume;
verified generations/retention preview; publication preflight/upload and explicit
client configuration/activation instructions. The worker calls the library and its
serializer; it adds neither network/TLS logic nor dependency solving. Catalog
validation must surface unsupported git/path/registry fields before starting a run.

## 7. Decisions for the owner and qualification gates

| Decision needing approval | Recommendation and reason |
| --- | --- |
| Coverage versus size | MVP declared features plus all-platform Cargo lock closure; other feature profiles need catalog entries. Confirm whether all-optional coverage is required for the first release instead. |
| Cargo prerequisite/client baseline | Accept an explicitly installed/pinned Cargo for planning, initially qualify Cargo 1.90/resolver 2. Decide additional client versions/resolver 3/MSRV contexts before promising coverage. Fetching an existing plan stays library-only. |
| Capacity and pacing | Choose actual organization caps, unknown-length policy, latest-N default, transfer concurrency and contact string. No numeric storage capacity is approved by this design. |
| Registry access | MVP crates.io only and read-only anonymous internal HTTPS clients. Private/alternate registries, authenticated readers and native Nexus integration are separate decisions. |
| Approval/licensing | Decide whether dependency closure is automatically admitted or each newly discovered crate needs approval. Recommend explicit preview approval of the entire closure. Obtain organization redistribution/license review; retaining upstream archives does not grant additional rights. |
| Dependencies | Approve needed packages only after a scoped implementation proposal: likely direct `semver` for Cargo requirements; possibly a Cargo manifest-schema parser and JSON parser. Existing `toml`/serde/hash/network crates can be reused. Do not implement semver, manifest compatibility or index JSON with ad hoc string parsing to avoid approval. |

No implementation dependency is approved or added here. In particular Cargo-as-a-
process avoids importing Cargo's large unstable Rust API, but it does not remove the
need to approve parsing dependencies. A source-format-compatible parser may require
an explicit minimum Cargo/schema policy. Retain existing secret, no-redirect and
no-real-Nexus-write boundaries.

Before an implementation can claim success, qualification must cover: Cargo syntax
fixtures and rejection spans; aliases/ranges/pins/prereleases/yanks; feature default,
optional/weak/renamed dependencies; host/build/target/dev behavior; native `links`
and conflicting constraints; multiple independent roots; archive-free planning;
filtered-index witness replay; metadata/count/size/time budgets; unknown lengths;
index-schema and path/case refusal; checksum corruption; killed-process resume and
locks; new upstream versions versus frozen replay; exact retained index-line bytes;
partial publication/GET failure and no fallback; local HTTPS sparse client fetches
on Windows and Unix from isolated homes. Use representative projects with preexisting
locks and explicit negative controls for missing versions. None of these checks was
run for this design-only change. Existing toolchain/Nexus evidence does not qualify
crate resolution, Cargo clients or the new inventory format.

The phased estimate and deferred work are in [ROADMAP.md](ROADMAP.md). The largest
technical uncertainty is qualification of isolated metadata-only Cargo resolution
and filtered-index replay, not file downloading.

## Reference comparison

Panamax provides a useful transferable-mirror precedent, but not this policy model.
At inspected revision
[`84ca2d7064f5a9931371f9eb74c4003707897a7b`](https://github.com/panamax-rs/panamax/tree/84ca2d7064f5a9931371f9eb74c4003707897a7b),
its [default configuration](https://github.com/panamax-rs/panamax/blob/84ca2d7064f5a9931371f9eb74c4003707897a7b/src/mirror.default.toml)
uses the Git crates.io index and supports rewriting its mirror URL. Its
[crate synchronization source](https://github.com/panamax-rs/panamax/blob/84ca2d7064f5a9931371f9eb74c4003707897a7b/src/crates.rs)
walks version records and optionally filters a vendor/lockfile-derived whitelist;
that is not a Cargo-syntax organizational range catalog with per-version resolution
witnesses. Reuse the concept of portable upstream-preserving output, not an
unbounded all-crates traversal or treating one project's lock as organization policy.

Sources were inspected on 2026-11-12. Cargo Book and service policies are living
documents; the implementation must recheck them. Pinned source references document
what was actually inspected, not a claim that future Cargo behavior is fixed.
