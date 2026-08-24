# Issues — Rust port plan ("final directory structure" proposal)

Grievances with the proposed Rust/`lexicon` CLI plan, in descending severity.
"Infeasible" means: as drawn, the plan cannot deliver what it promises — not
merely "harder."

## 1. The prebuilt-binary directories cannot be populated from this project
`begin-here/setup/` and `begin-here/cli/` promise six native binaries each
(three OSes × two architectures), and every impl folder promises a committed
executable. Rust cross-compilation is realistic for the four Linux/Windows
targets, but macOS binaries require Apple's SDK and linker — legally and
practically unavailable off-Mac. This workspace is a Linux container, so it
can produce exactly one of the six targets. The structure has no story for
who builds the other five, on what, or how they stay in sync when core
changes. Hardest wall in the plan.

## 2. One binary slot per impl folder contradicts the six-platform promise
`get_raw_data_impl/get_raw_data_impl[.exe]` is a single file. If a Windows
user runs `lexicon source X --add`, the folder holds a Windows binary and
every Linux collaborator's `lexicon data --get X` breaks. `begin-here/` got
per-platform subfolders for exactly this reason; the impl folders did not.
Aligned 100% to the given structure, a multi-platform team is structurally
impossible: the tree can hold only one platform's build of each impl at a
time.

## 3. The session engine's semantics are POSIX; the structure promises Windows
The engine's guarantees — `--bg` detach via new session groups, truthful
exit codes from SIGTERM/SIGINT, the flock-based cross-process throttle token
bucket — are Unix mechanisms. Windows has no SIGTERM, no setsid, no flock
(LockFileEx, Ctrl handlers, and job objects have different semantics; the
"untrapped TERM" subtlety does not exist there at all). "Identical session
rules on all six platforms" requires redesigning the engine contract around
the weakest platform, not translating it.

## 4. The processed-data layout erases an agreed decision
The tree shows `data/processed/<source>.sqlite` and nothing else — no
`asset/<kind>/…`, no `data/work/`. The agreed Option B is: ALL bytes as real
files, no blobs; sqlite holds only metadata and locators. Aligned 100% to
this structure, either every asset becomes a blob (reversing the decision)
or impls write outside the sanctioned tree (violating it). Likewise the
process-phase abandon extra ("clear `data/work/`") loses its target folder.
Not a Rust problem — the given tree contradicts the schema plan.

## 5. "Rust verifies GetRawData" is weaker than stated
`--add` succeeding means the crate compiled. A `main.rs` that ignores core
entirely also compiles. The trait check only bites if the generated skeleton
makes `core::run::<impl>()` the only path to a working binary — achievable,
but that is discipline encoded in the skeleton, not something Cargo
enforces. Since core ships as source in the same tree, an impl can always
fork or `#[path]`-trick its way in; the privacy gain over Python
(`pub(crate)` — real and compiler-checked) is still compile-time convention,
not a physical boundary.

## 6. Version skew has no home in the structure
Each impl carries its own `Cargo.lock` and embeds core as of its build
moment; the installed `lexicon` CLI was built even earlier, elsewhere.
Nothing in the tree records which core version a committed binary embeds, so
`session_status.json` / on-disk format changes can silently divide binaries
into incompatible generations. Fixable (embed a format version, refuse
mismatches) — but the structure as given has no slot for it.

## 7. Environment- and workflow-practicalities
- The scheme assumes `cargo` can reach crates.io wherever `--add` runs; in
  this workspace the same firewall that makes the npm registry unusable will
  likely block or throttle crates.io, so even the one buildable target may
  not build here.
- The dev loop changes character: today an impl edit is save-and-run; under
  the plan every impl change is a compile, which slows the
  inspect-and-iterate rhythm this project runs on.

## What survives untouched
The two-phase model; sessions/status/raw-record conventions; the
façade/private-engine layering (strengthened by `pub(crate)`); the
`--draft`/`--add` no-central-registry flow; `--` argument pass-through
(cleaner than the current CLI). The infeasibilities cluster in two places —
shipping compiled artifacts inside the data tree (1, 2, 6) and promising
POSIX semantics on non-POSIX platforms (3) — plus one internal contradiction
with the schema decisions (4) that any language would have to resolve.
