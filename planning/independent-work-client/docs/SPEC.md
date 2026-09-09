# Tachiko Work: an independent work client

State: **Steward-supplied provisional product/client design**, 2026-09-10 JST.
This document does not amend an Accepted semantic, storage, authorization, SDK,
licensing or release contract. Its upstream authority is indexed in SOURCES.md.
The founder explicitly requested a new independent frontend, designed without
inheriting the current UI. That supersedes #354's earlier recommendation to defer
an independent client; it does not authorize retiring the existing product.

## 1. Product thesis

**One piece of work; several useful ways to understand and change it.**
A table answers “what varies, what matters, what should change?” A document
answers “why, what did we decide, what does the evidence mean?” A project keeps
these connected and preserves the work. An assistant helps inspect or propose
changes to that same work. It is not a fourth application with its own data.

The primary job is to turn a team's working information into a decision and a
safe, understandable change, then continue that work later. The design must make
this possible without an account, AI subscription, Git knowledge, schema wizard
or explanation of Rust/revision/grant internals. Those mechanisms earn visible
space only when the user needs control or an explanation.

The product is neither a spreadsheet pasted beside a block editor and chatbox,
nor a universal database UI. Shared meaning must not force all documents into
rows. A document can be valuable as authored narrative even without a table.
A table can be valuable without a document. Linking them should remove duplicate
work, not require users to adopt an ontology.

### Experience rules

1. Open useful work before asking the user to configure a platform.
2. Change views without duplicating values, identities or authorization.
3. Human editing stays direct; ordinary human keystrokes do not need an AI-style
   approval ceremony. Delegated changes do require exact review/approval.
4. Explain impact where the user is editing, not in a detached technical console.
5. Preserve drafts and sources; distinguish changed, saved, and externally sent.
6. AI is optional. Every essential open/edit/review/save task works without it.
7. Unsupported or lossy behavior is visible; a smooth animation is not evidence
   that a change succeeded.

## 2. Information architecture and concepts

| Concept | User meaning | Ownership and constraint |
|---|---|---|
| Workspace/Home | My recent work and available locations | Initially a host-local catalog, not a new cross-project semantic transaction layer |
| Project | One coherent piece of work with a save/export boundary | One resident occurrence of a core-owned document; canonical `.ro`/`.roproj` at storage boundary |
| Table | Compare and edit structured facts | A projection of a collection, stable entities and fields; row position is never identity |
| Brief/document | Write context and explain decisions using linked facts | Authored text plus references; no independent copied business values or editor-owned canonical JSON |
| Change | A direct edit receipt or an uncommitted proposed transition | Different lifecycle states are visibly distinct; a receipt is not persistent history/undo |
| Assistant task | Help with this selected work | Scoped semantic queries/proposals; no intrinsic authority, background scheduler or hidden host effects |

### Navigation

Home -> Recent projects / Open existing / Start from a supported template.
Project -> named work views (initially Table and Brief), with a contextual
Inspect / Changes / Assist panel. The project name and save state remain visible.
The same selection can be shown as a row, a briefing subject or a review target.

Views are not separate copies or separate apps. Opening Brief must not import
from Table; it queries the same entity/fields. A contextual panel opens only when
needed. There is no compulsory always-open chat sidebar, oversized dashboard of
empty cards, Office ribbon clone, raw schema tree, or Git-centric start screen.

Home's catalog is not the user's document. Losing a recent-items entry must not
make a valid exported `.ro` unintelligible. First delivery opens one project at a
time; multi-project residency and cross-project references are not invented here.

### Screen contracts

**Home:** a clear Open action and one working sample, recent saved copies, and an
honest storage-location label. No sign-in gate. Inapplicable future templates
are absent, not broken demo buttons.

**Table:** focus belongs to the work. A compact toolbar provides view/search
controls as supported; the grid occupies the main area. Direct editing supports
keyboard flow and editable drafts. Calculated cells are recognizable and can be
inspected. Typed errors identify the affected value and a recovery action.

**Brief:** a readable title and authored decision notes, followed by live facts
for the selected work item. Selecting a fact reveals its source. The initial
brief is a template-backed narrative projection, not a full Word replacement.
Changing a linked value updates the corresponding fact; stale evidence must not
masquerade as current. Reading a document must not silently modify its source.

**Changes:** before/after, affected work, checks and outcome. A direct human edit
may have a current-session receipt. A delegated proposal is explicitly not yet
applied. Do not market a transient receipt list as durable history or undo.

**Assist:** selection/scope is visible; explain and suggest are separate actions.
A suggestion becomes a reviewable semantic proposal, not authoritative prose.
“Approve and apply” may be one human gesture backed by the proper trusted
approval/execution sequence. Missing capability disables the action with a useful
reason; it never falls back to editing a local JS document.

**Failure/recovery:** a draft remains editable after rejection; a save failure
leaves accepted work unsaved; an uncertain publication is labeled uncertain.
Close/Open while work is unsaved offers Keep editing / Save a copy / explicit
Discard. Choosing Keep editing preserves focus, selection and drafts. Never
silently discard because an incoming OS open event arrived.

## 3. How document, spreadsheet and agents stay coherent

The stable work target, current runtime occurrence and revision connect the
surfaces. A title is not an ID; a sheet address is not an ID; a URL or file path
is not an authorization domain. Projections may adapt labels and arrangement.

A document reference is conceptually a live semantic reference with a source and
freshness, not a copied number hidden in rich text. A deliberately frozen excerpt
would be labeled as a snapshot, but a general persisted embed syntax is not
accepted by this seed. No frontend-specific block schema becomes durable law.

For M1, a plain Text `notes` field and a small fixed brief template exercise real
authored narrative plus linked facts. There is no new core primitive, rich-text
persistence format, Markdown reference grammar, or generic document builder.
Later free-form documents must pressure-test paragraphs, lists, references,
missing targets, copy/paste, export and revision meaning before stabilizing an
editor model. A document adapter may use an editor library for drafts/rendering;
the library's private JSON is not automatically the canonical document model.

The agent path is Query -> scoped Propose -> review exact change/base -> Human
Approval -> trusted Execute -> receipt. AI/provider text is untrusted input.
Human corrections and agent proposals use the same core laws and source targets,
but do not pretend to have identical permission or approval requirements.
Semantic approval never implies Save, Git push, network upload or deployment.

## 4. Frontend, core, and host boundary

| Layer | Owns | Must not own |
|---|---|---|
| Client/UI | Rendering, accessibility, selection, viewport, draft buffers, pending work, revision-keyed projections | Canonical document mirror, formula evaluator, validation policy, semantic diff/merge engine, trusted grants |
| Core/runtime (upstream) | Typed queries/commands, identity, calculation, authoritative gates, immutable proposals, exact approval checks and atomic publication | Browser storage, filesystem, credentials UI, deployment |
| Host/composition | Runtime lifetime, trusted identity context, selected file access, persistence transactions, dialogs, effects | Alternative semantic rules or permission minted by renderer-supplied claims |

A local TypeScript port may normalize invocation/result shape for this one client.
It is experimental implementation wiring, not a new public SDK, protocol, or
semantic command catalogue. Missing core capability is a tracked gap, not a
reason to implement its semantics in React.

### Runtime ownership and lifecycle

Web uses one resident Rust/WASM runtime in a Worker per open occurrence. Normal
edits send typed intent and bounded queries, not a full serialized document on
every keystroke. Explicit open/save/export/recovery may use full snapshots.

Projection ownership is keyed by opaque occurrence + revision + semantic target
(and effective disclosure context when relevant). A closed occurrence cannot be
revived by a late response. Revisions are opaque equality/precondition values,
not integers the frontend increments. On a failed refresh, stale evidence is
marked or withheld until authoritative refresh. A lost Execute reply is not
permission to retry a mutation blindly. A proven status/receipt query may resolve
uncertainty; a timeout alone may not.

Presentation caches, scroll positions and recent-item catalogs can be disposable.
Authored notes, typed values, formulas and references cannot live only in them.

### Persistence and portability

M1 offers explicit create-only **Save a copy** and portable export. It preserves
the input. It must not claim autosave or in-place replacement has been solved.
“Saved on this device” means a successful durable host commit, not a semantic
receipt, buffer export, toast timeout or same-runtime round trip. Browser-local
storage is not a backup promise; portable export remains available.

Persist/export canonical storage output from the upstream codec. The experimental
kit's `ProjectExport.bytes` is an app-private transfer envelope; do not rename it
`.ro`, invent a client parser for its private layout, or establish it as a new
permanent file format. A bounded upstream consumer-kit capability must expose
canonical export/import where needed. Canonical `.roproj/v1` has 18 files for this
fixture, including empty shards; format-2 admission/migration belongs to its own
Accepted authority and current capability, not this v1 slice.

Longer-term save-in-place/autosave is a desirable UX direction, not a license to
bypass the storage/recovery decision process. No existing browser origin, native
app identifier, storage database, or file association is silently reused.

### Trust and privacy

Default editing is local: no telemetry, remote fonts, account requirement or
provider calls. A future configured provider receives only an explicitly selected
scope after consent, through a host adapter. Credentials stay out of documents,
renderer state and bundles. Documents/provider output cannot appoint a principal,
expand a grant, turn a Human credential into an AI credential, or trigger a host
effect. Text rendering is non-executable; external links require safe schemes and
explicit user action. Dev fault/observation hooks must not ship in release builds.

## 5. First demonstrable vertical slice: next-release decision

User goal: **change the next-release priorities, explain the decision, and reopen
the saved work without losing meaning.** This is a small team/game-tool planning
example, not a replacement of the game-development wedge by a generic PM suite.

1. Open the included ordinary `.roproj/v1` release-plan sample (three items), or
   open the same supported project through the native-file carrier when available.
2. In Table, edit the first item's Impact from 5 to 3 with the keyboard. Its
   core-calculated Priority changes from 10 to 8 (Impact + Friction).
3. Open Brief for that same item. It shows the current Impact/Priority and an
   editable authored note. Apply the CJK note “先完成試玩回饋，再決定下一版範圍。”
4. Return to Table or inspect the current-session change. Show the affected
   values and truthful state, without an internal-revision lecture.
5. Save a distinct copy. Close the entire browser process, reopen its persistent
   profile, choose that saved copy, and recover the same identities, note and
   recomputed facts through a fresh runtime. The source remains untouched.
6. Demonstrate a rejected number, cancelled draft, IME composition, failed save,
   and lost publication reply without data loss or false success.

This **M1** slice already joins spreadsheet, authored narrative and project
lifecycle. **M2** adds a deterministic, clearly labeled proposal-source fixture
through the real delegated authorization boundary, then the contextual agent UI.
It must not claim an LLM is connected or call an “AI” button a real agent demo.
The combined demo is complete only after that second milestone. A network LLM
provider is not needed to prove proposal/approval safety and is a later adapter.

The sample is not schema-authoring functionality. Arbitrary table creation,
multi-sheet spreadsheet breadth and sophisticated interoperability continue to
have their own core/product owners. A polished M1 must not be presented as launch
completion under #256/#261.

## 6. Visual and interaction direction

Aim for a calm working surface, not a marketing dashboard. Use system fonts with
CJK coverage, a neutral canvas, one restrained accent, clear borders and generous
space around narrative without wasting table space. Never communicate errors or
save status by color alone. No external font or decorative asset dependency.

Provisional design targets: 14px working text, about 34px comfortable rows with a
density option later; a collapsible navigation rail around 220px; a contextual
panel around 340–380px; readable narrative limited to roughly 65–75 characters per
line. These are iteration starting points, not acceptance pixel locks. At narrower
widths the panel becomes a focused view rather than squeezing both editors. Grid
horizontal scrolling is local, not an accidental whole-page overflow.

Keyboard: arrows navigate cells; Enter/F2 edit; Escape cancels a draft and returns
focus; Tab commits admissible input and moves predictably. IME Enter during
composition never publishes. Notes use an explicit Apply action or its documented
shortcut; switching views preserves an uncommitted draft. Dialogs have accessible
names, correct focus containment and restoration. Apply WAI-ARIA grid practices,
but prefer native controls whenever suitable. Manual VoiceOver/NVDA and real
Japanese/Chinese IME checks remain separate from synthetic event tests.

Initial performance work measures input feedback, calculation pending states,
Worker startup, memory and first useful rendering on a recorded device/browser.
A sub-100ms feedback target is a design budget, not a claimed runtime SLA.
Do not promise million-row performance, add virtualization that breaks focus, or
build a custom canvas grid before measured pressure justifies it. A thousand-row
interaction probe can guide the later grid renderer choice; the three-row canary
is a correctness fixture, not performance evidence.

## 7. Independent repository and technology strategy

Proposed name: **nurockplayer/tachiko-work-client**. One client repository serves
Web and Desktop. Do not create separate sheet/document/AI repositories or split
this into a package platform before there are real independent consumers.

Provisional stack: React + TypeScript + Vite, pnpm, native DOM/CSS, and Playwright
acceptance. A local-first editor does not currently need SSR or server components;
a Vite SPA accepts the cost of owning a small routing/loading layer. Do not add a
Next.js server, Redux-style canonical document store, general RPC framework, rich
editor framework or heavyweight grid simply because one is familiar. Select and
pin exact supported dependency versions in bootstrap with an actual lockfile;
this seed does not invent a tested dependency resolution.

Initial Desktop: Tauri 2 as a thin OS/file host around the same UI and the same
WASM runtime. Running the semantic engine natively is an optional later topology
optimization requiring evidence, not a prerequisite for the first desktop demo.
Keep Tauri permissions and command scopes narrow; framework capabilities do not
replace checks inside host commands. First OS proof is macOS; other platforms
are not implied by a cross-platform framework dependency.

Suggested initial layout (private details are replaceable):

    src/app/                  navigation and composition
    src/features/work/        table, brief, contextual work flow
    src/features/changes/     receipt and proposal presentation
    src/adapters/core/        the one experimental consumer-kit adapter
    src/hosts/browser/        local save/open composition
    src/ui/                   small shared accessible primitives
    src-tauri/                later native I/O host, not a second semantic engine
    tests/acceptance/          Steward-owned behavioral contracts
    docs/                     client-local UX and ownership; upstream pointers
    vendor/tachiko/            one intact verified kit, or equivalent pinned artifact

The exact kit includes JS, types, Worker modules and WASM from one source commit.
Record source SHA, artifact digest, build provenance, capabilities and license
notices. Build/run the consumer from a clean checkout with no upstream sibling
paths. The upstream builder may still compile the current app-local adapter;
that is producer implementation evidence, not a mandate to reuse the old UI.
No copied `mountDesigner`, CSS, screenshots or app state/store may define the new
client. Keep the current exporter intact during initial consumer qualification.

An explicit follow-up upgrades between two genuinely different qualified core
artifacts and rolls back without losing the v1 saved sample. Reinstalling the
same hash is not upgrade evidence. A client kit remains experimental even after
an independent first-party consumer works; #231's independent-adopter and stable
SDK evidence is not satisfied by our own client.

## 8. Implementation sequence and ownership

**C0 — bootstrap and real-kit acceptance qualification.** Astra High coordinates;
Terra can build/package a bounded core artifact and Luna can run fixture/harness
checks. This is authorized tests/tooling-only preflight, not product Ready. Create
or identify the new repo without overwriting an existing one, retain seed
provenance on a dedicated branch, pin actual versions/artifact checksums, run the
real canary, and return exact evidence for readiness. No permanent new release
policy or architecture is introduced through bootstrap. Default to private
staging when creating an otherwise unspecified remote; public release and
contribution terms continue to follow #15/#202, not a new licensing decision.

**C1 — consumable canonical I/O kit (upstream).** Terra High, Guarded. Deliver the
smallest independently useful artifact/canonical I/O capability required by the
new host, preserving the experimental boundary. Do not rewrite the old UI or
promise a public SDK. Own codec/compatibility evidence in the upstream PR.

**C2 — linked human work + durable Web lifecycle (new repo).** Astra High for the
first integration, Medium for subsequent bounded visual refinement. Guarded
because of save/lifetime/data-loss risks. Consume C1; use M1 browser acceptance,
write implementation-owned unit tests, and obtain independent final-head review.
One coherent delivery PR, not a PR per panel or per technical layer.

**C3 — trusted delegated consumer bridge (upstream).** Terra High, Guarded. Reuse
existing Propose/Approve/Execute/receipt authority and expose only the bounded
consumer seam needed by C4. No browser-forged grants, approval booleans, general
agent platform or provider integration. The M2 contract suite fixes the laws;
transport wiring and actual baseline still require qualification.

**C4 — contextual agent review/apply (new repo).** Astra Medium after C3 is pinned.
Guarded. Deliver the whole task -> scoped proposal -> review -> explicit apply ->
receipt flow, including stale/denied outcomes. Luna Medium may own isolated
presentation/accessibility/unit-test work, not lifecycle or authorization design.

**C5 — same work on macOS (new repo).** Terra High for the host, Astra Medium for
interaction integration. Guarded. Same UI/WASM meaning, separate app identity,
explicit file selection/open, create-only copy, source preservation and real
process/OS-open evidence. Do not compete with existing #345/#351 ownership.

Concurrency is bounded by actual write ownership: C2 and C3 may proceed in parallel
after the C1 artifact is stable. Do not run two writers against one branch, one
consumer port, or one unsettled persistence contract. Astra owns integration and
user-flow coherence; a fresh independent deep reviewer (e.g. Sol High) covers
Guarded final heads. Agent labels do not assert any local profile file exists.

### Readiness and endpoint

Current handoff is **preflight-authorized**. The authored browser/agent tests are
unverified against real product composition. Green assertion self-tests are not
production acceptance. C0 returns qualified canary/fixture/test wiring evidence;
the Steward then decides each implementation Issue's Ready state under existing
governance. Do not open competing implementation PRs or merge a knowingly failing
seed. Applicable unit tests belong to the implementer. Material acceptance
challenges return to the Steward, not a unilateral looser assertion.

### Migration and release, not prerequisites for the first demo

Keep existing Designer and its current release lane live. #330/#331 grouped-sum,
#345/#351 old-app desktop, #315 table authoring, #348 old-client fault qualification,
and #350/#353 old-UI cleanup remain with their current owners. No feature deletion,
redirect, default file-association takeover, origin migration or silent browser
storage reuse is authorized.

Before replacement: enumerate current supported workflows and capability gaps;
prove canonical source preservation, saved-document upgrades/rollback, browser
origin/native identity migration or an explicit export/import bridge; run genuine
target-user tasks and normal distribution checks; obtain the existing #256/#261
release decision. First-party client success does not constitute independent
third-party adoption or legal/contribution-policy clearance.
