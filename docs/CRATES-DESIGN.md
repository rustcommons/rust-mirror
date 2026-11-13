# Curated crate mirroring design

**Owner review draft, revised 2026-11-13 for decisions dated 2026-11-12.** rust-mirror 0.3.0 mirrors
Rust toolchains, not Cargo registries. None of the commands, configuration extensions
or library items proposed here exists yet. The owner authorized native Phase 0 and direct `semver`; `serde_json` was approved
earlier. Broader implementation, publication and real Nexus writes remain outside
this authorization. No external or downloaded Cargo is allowed at runtime, and
Cargo must not be embedded as a library. Cargo is a CI conformance oracle only. See [requirements](REQUIREMENTS.md) and [roadmap](ROADMAP.md).

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

### Decision: native closure plus bounded consistency search

Resolution is built into rust-mirror. No external Cargo executable, managed Cargo
installation, Cargo library, or third-party solver is part of the runtime. Use the
approved `semver` crate for Cargo-style requirements and `serde_json` for index JSON.
The goal is a supply set with a coherent solution for each selected root/profile,
not byte-for-byte agreement with Cargo's preferred lockfile. Cargo remains a
qualification oracle in CI only, never a planning subprocess.

Start with conservative edge closure: for each encountered normal/build requirement
on every platform, choose its newest eligible non-yanked version with the requested
features, then repeat for that version's activated edges. Deduplicate payloads but
retain all reasons. This is a useful starting set, **not a completeness proof**.

For stable versions with interval requirements and no additional constraints, if
all requirements have a nonempty intersection, the least of their individual maxima
lies in that intersection and is its maximum. This argument does not extend to all
Cargo candidate sets. Two explicit counterexamples must be regression fixtures:

- Feature availability: `x` 1.3 has feature `a`, 1.2 has `b`, and 1.1 has both.
  Two paths requiring `x = "1"` with `a` and `b` select 1.3 and 1.2 independently.
  Cargo must unify compatible 1.x uses; 1.1 is the missing joint candidate.
- Prerelease eligibility: index versions are 1.1.0, 1.2.0-alpha, 1.3.0-alpha.
  Requirements `>=1.0.0, <1.2.0-zzz` and `>=1.0.0, <1.3.0-zzz` independently
  select the two different prereleases. Each requirement excludes the other's
  prerelease; their common stable candidate 1.1.0 is absent from that starting set.
  Test each requirement separately with `VersionReq::matches`, never concatenate
  comparators: doing so can incorrectly union prerelease eligibility.

Use a bounded, deterministic native backtracking consistency search for each exact
root/profile, with candidates ordered newest first. It must track requirements and
feature unions per package/compatibility class, Cargo-compatible version unification,
and one version per native `links` name. Choices activate that version's own feature
and dependency graph; retract conditional constraints when backtracking rather than
leaving constraints from discarded versions. Never silently replace an exact root.
Search may add an older candidate and its closure from already captured index
metadata when the initial edge set is insufficient. Every addition has a reason,
counts against the preview/bounds and requires the same whole-plan approval.

Replay the resulting witness using only the proposed filtered index, without live
network or archives. On contradiction, unsupported semantics or exhausted search
budget, refuse the plan with structured constraints; never label independent edge
satisfaction as a successful consistency check. No PubGrub/other solver crate is
approved. If fixtures or corpus evidence show a reliable native search cannot be
maintained within this scope, report the failing case and request a solver decision
rather than adding a dependency or weakening the gate.

This is an existence goal for the declared profiles, not a formal proof of every
Cargo release's resolver behavior. A native witness is necessary; CI conformance
against real Cargo is an additional blocking release gate. Unsupported constructs
fail explicitly until the native model and conformance agree. The inspected
[Cargo resolver behavior](https://doc.rust-lang.org/cargo/reference/resolver.html)
and [Cargo resolution source](https://github.com/rust-lang/cargo/blob/0a2d4c697121839929f080e539d3ccac11e4375d/src/ops/resolve.rs)
are references, not embedded dependencies.

### Native feature propagation

Compute feature closure separately for each candidate version. Start with declared
features and `default` only when enabled and defined. Merge feature requests from
all active incoming edges, including dependency declarations' defaults/features.
Process named feature expansion, explicit `dep:alias` activation, implicit optional
features unless suppressed by `dep:`, strong `alias/feature` forwarding, and weak
`alias?/feature` forwarding only after that optional dependency becomes active.
Revisit weak forwarding when another path activates the dependency.
Keep build-time activation distinct from lock coverage: a local Cargo 1.90 fixture
and reqwest/default conformance reproduced [Cargo issue 10801](https://github.com/rust-lang/cargo/issues/10801).
An enabled weak forwarding expression can put its otherwise inactive optional
dependency and forwarded-feature closure in Cargo.lock. Include those records and
archives as explicitly explained `lock_only` coverage; do not claim the optional
dependency is enabled in a build. This is not all-optional mirroring. A fixture with
that forwarding feature disabled must exclude the edge. The conformance suite must
keep this version-specific behavior visible rather than silently rewriting the
upstream record or dropping the declared-features policy.
 Aliases address
feature edges; the index `package` value supplies canonical package identity.

Merge index `features` and schema-2 `features2` according to Cargo's index format;
refuse unknown required schemas. All-platform normal/build edges stay in lock-style
coverage; dev edges do not propagate. Feature growth may invalidate the currently
selected version and require backtracking to one that offers the union. Never
reuse another version's feature definitions or activate all optional dependencies.

### Features, platforms and dependency kinds

| Dimension | Proposed MVP decision and consequence |
| --- | --- |
| Features | Honor each root's declared defaults and enabled features. Model Cargo's feature propagation, including optional dependencies, `dep:` and weak forwarding. Do not activate all optional dependencies of all transitive crates. Missing requested features fail that root. |
| Optional dependencies | Include the optional edges activated in the resolution witness, plus the native all-platform consistency witness. A later project enabling an uncurated feature may need additional crates. The preview must state this coverage boundary. |
| Targets | Record selected host/compiler and compilation target pairs, but retain the native complete all-platform normal/build closure in the MVP. Do not strip an index dependency or omit a locked archive merely because it is inactive on the mirror machine. This conservative overfetch avoids metadata-only index entries for unavailable payloads. |
| Build dependencies | Include their closure and host-side proc-macro/build requirements. A host is not interchangeable with a target; cross-compilation does not remove host dependencies. |
| Development dependencies | Exclude a registry root's own tests/examples/dev dependencies: model it as a registry dependency, not a workspace member. Consumer-project dev tools must be explicit catalog roots. |
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

For each root/profile, combine conservative newest-per-edge coverage with a
native coherent witness, permitting older candidates when unification, exact pins,
feature availability or `links` requires them. Incompatible version classes may
coexist; compatible uses must unify as Cargo requires. A `links` conflict may force
backtracking in an upstream package choice; checking only dependency requirements
at the leaf is insufficient. An unsatisfiable root is a diagnostic, not permission
to ignore `links`, widen a requirement or silently drop a feature.

Keep every previously retained version and its recorded closure. A new root/profile
requires a new witness; copying an old witness does not establish fresh resolution.
Replay each new root against only the filtered proposed index. Do not mirror all
matching upstream versions as a fallback, and do not promise arbitrary combinations
of independently curated roots. Explain all conservatively included alternatives,
including versions not used by the witness. Existing client locks require explicit
pin coverage. Root-to-edge explanations come from the native graph, not Cargo text.

Bound root versions, crates, resolved versions, edges, witness count, metadata
bytes, per-file bytes, total final bytes, transferred bytes, planning time and
diagnostic output. Report the first exceeded bound and its inclusion chain.
Counts and paths are exact in the preview. Archive lengths come from opt-in HEAD
requests: distinguish known total, unknown count, retained bytes and additional
transfer estimate. A complete byte total is impossible when lengths are unknown;
show that explicitly and require an approved cap/unknown-size policy before fetch.
Enforce actual streamed bytes regardless of estimates. Deduplication reduces
transfer, not the logical final-generation budget. Never prune to fit a budget.

### Yanked versions and metadata changes

Exclude yanked versions from new floating selections. The MVP refuses a new yanked
pin even if its archive is reachable: Cargo does not normally select it for a new
lock; the native planner must use the same refusal. Supporting a fresh historical yanked pin requires separate resolution design.
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
refresh on resume. Cancellation checkpoints cover broker I/O, native closure/search,
copy/hash/download loops; search state and diagnostics must be bounded. Kill/resume
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
No public runtime API accepts a Cargo executable or invokes a solver subprocess.
Cargo executable selection exists only in the CI qualification harness.

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
diagnostic output. Land the actual additive protocol/API specification before GUI
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
| Coverage versus size | Approved: declared features only, with all-platform normal/build closure; other feature profiles need catalog entries. |
| Resolution/client baseline | Approved: built-in resolution, no runtime Cargo or Cargo library. Cargo 1.90/resolver 2 is an initial CI oracle; additional client/resolver/MSRV baselines need qualification. |
| Capacity and pacing | Choose actual organization caps, unknown-length policy, latest-N default, transfer concurrency and contact string. No numeric storage capacity is approved by this design. |
| Registry access | MVP crates.io only and read-only anonymous internal HTTPS clients. Private/alternate registries, authenticated readers and native Nexus integration are separate decisions. |
| Approval/licensing | Approved: one explicit approval of the whole preview per plan, including the explained transitive closure. No per-crate approval step. Obtain organization redistribution/license review; retaining upstream archives does not grant additional rights. |
| Dependencies | Approved: direct `semver`; earlier `serde_json` approval applies. Reuse existing TOML/serde/hash/network crates. No Cargo library, PubGrub/other solver, tar or flate2 is needed/approved for Phase 0; any further dependency requires a separate decision. |

Owner decisions dated 2026-11-12 supersede the original external-Cargo draft.
Native Phase 0 is authorized; no crate publication or broader implementation is
implied. Retain existing secret, no-redirect and
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
locks and explicit negative controls for missing versions. Phase 0 must record actual results separately from this design; the initial
design-only revision ran none of these runtime checks. Existing toolchain/Nexus evidence does not qualify
crate resolution, Cargo clients or the new inventory format.

The phased estimate and deferred work are in [ROADMAP.md](ROADMAP.md). The largest
technical uncertainty is native feature/unification/backtracking correctness and
filtered-index conformance, not file downloading.

## Phase 0 exit evidence

Use a metadata-only native planner for tokio/full, serde, clap/derive, reqwest/default,
rand 0.8 and 0.9, openssl-sys/native-links cases and a proc-macro-heavy stack.
Freeze exact roots and index observations, record zero archive requests during
planning and the complete inclusion graph. In CI, serve the filtered sparse output
to real Cargo in isolated homes: generate-lockfile for every profile, plus fetch/build
for cheap synthetic fixtures. Cargo is a test oracle only. Fixtures must cover the
two counterexamples above, exact pins, yanks, aliases, missing/version-varying
features, defaults, dep:/weak forwarding, features2, build/dev/target edges, links
conflicts, backtracking and bounded refusal. Any conformance failure blocks release.

Compare native mirrored version identities against the union of full-index Cargo
lockfiles for the same exact profiles and metadata observation. Report native count,
Cargo union count, intersection, native-only count/percentage and missing identities;
missing identities need not be wrong if another coherent choice exists, but must be
explained. Also report per-root ratios and metadata bytes. Payload-byte overfetch
is unknown without HEAD/content sizes and must not be invented from version counts.
No Phase 0 completion claim is valid without actual green CI and measured numbers.

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
