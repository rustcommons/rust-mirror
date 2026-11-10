# Resume frozen-plan downloads

A selected snapshot can now retain verified files and an interrupted transfer.
This applies to `fetch`/schema 2; [generation assembly](MULTI-VERSION.md)
can reuse completed selections but does not itself resume. The legacy `fetch-toolchain` route and HTTP
publisher are unchanged; resumable Nexus publication is not implemented.

## Commands

```sh
rust-mirror fetch --plan ./project-plan --output ./project-snapshot
# After interruption, repeat with the same plan and output:
rust-mirror fetch --plan ./project-plan --output ./project-snapshot --resume
```

Normal `fetch` still requires a new output. `--resume` is a valueless flag and
requires an already initialized output for the **exact** frozen plan, including
its selection, metadata/artifact identities and byte budget. A changed plan must
use a new output; changing config alone does not change a saved plan.

Progress goes to stderr as file-level `Download`, `Reuse`, `Resume` and `Restart`
events. The byte count on a resume event is its stored offset, not a percentage
or a measured network-byte total. Stdout remains the completed mirror path.
Errors return status 1. Retry is explicit; there is no automatic retry loop,
retry-count option or background synchronization in this iteration.

## What is retained

`OUTPUT/.download` is private work state, bound to a canonical frozen-plan hash.
It contains verified payloads and `.part` files named by expected SHA-256, plus
an assembly tree. Do not serve or publish this directory. Only `OUTPUT/mirror`
is a completed snapshot. Its schema/layout and upstream metadata remain unchanged.

On each retry:

1. Acquire the output's operating-system file lock; refuse another active writer.
2. Recheck the plan identity and expected workspace paths/types. Refuse unrelated
   entries, symbolic links/devices and malformed state rather than delete user files.
3. Rehash stored complete payloads. Reuse only matching bytes; damaged owned
   cache files are downloaded again. Identical content can satisfy multiple paths.
4. For a partial file, request the remaining bytes. Accept only a coherent `206`
   byte range with the requested offset, known total, expected length and budget.
   A server returning `200` or `416` causes that file to restart from zero, while
   other verified files remain reusable. Malformed ranges fail without appending.
5. Verify the **whole-file** SHA-256, then assemble and hash-check copied bytes.
   An incorrect final hash discards that partial; another retry downloads it fresh.
6. Rename the fully verified assembly to `mirror` and remove payload work files.
   Small identity/lock files remain for a subsequent completed-output check.

A completed output is never overwritten. `--resume` rehashes its entire inventory
and compares it with the frozen plan; matching output succeeds without network.
Changed completed files cause refusal and are left unchanged. This can be expensive
for a large mirror because existence alone does not prove integrity.

## Failure and filesystem boundaries

- A killed process releases its OS lock. The lock file remains; its existence is
  not an active-lock indication. Never delete it while a fetch may be running.
- Network/disk errors can leave progress for retry. During initial workspace setup,
  an interruption can leave missing/malformed identity state; use a new output if
  the tool cannot validate it. It will not adopt an arbitrary old directory.
- Trusted private parent directories and a local filesystem with working file
  locking/rename semantics are required. Multi-machine/NFS locking, hostile local
  mutation and power-loss durability are not qualified. Do not edit active state.
- Current limits still apply: 512 MiB per payload, 1,024 payload entries and the
  frozen snapshot byte budget. Reused bytes count toward the completed snapshot
  budget. Assembly can temporarily need roughly twice the selected payload space.
- SHA-256 detects mixed or changed content; there is no ETag/If-Range identity
  promise. Original artifact URLs remain frozen, TLS remains verified and redirects
  remain disabled. A server unable to supply valid ranges falls back only for
  `200`/`416`; protocol errors are reported explicitly.
- This is per-output resume, not a global/shared artifact cache, retention/pruning,
  channel update, independent signature verification or resumable upload feature.

## Validation

[The dated evidence](evals/2026-11-10-resumable-downloads.md) distinguishes local
protocol/failure tests, twelve official-origin range samples, actual killed-process
resume and prior offline installation evidence. Run the permanent bounded driver
only with a verified matching baseline and fresh output/evidence paths:

```sh
python3 scripts/prove_resume.py --cli /path/rust-mirror --plan /path/project-plan --baseline /path/prior-snapshot/mirror --output /path/new-output --evidence /path/new-evidence
```

It interrupts its own official-origin downloader, seeds the other archives from
verified baseline bytes, resumes the interrupted transfer and compares every
completed file/inventory hash. It does not repeat installation tests, install
system tools, write to Nexus or change the user's Rustup/Cargo homes.
