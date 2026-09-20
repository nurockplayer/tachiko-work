# Competitive Landscape

Status: Research / market evidence. This document does not create Accepted product or architecture authority.

Evidence snapshot: **2026-09-18**

Decision issue: [#421](https://github.com/nurockplayer/tachiko-work/issues/421)

## Positioning

Tachiko Work is not primarily competing as an Office clone. It targets a semantic, versionable, computational work substrate that can be used by humans, conventional clients, and AI agents without making one vendor's application, file format, model, or hosted AI service the source of truth.

The 2026 market makes two older differentiators insufficient on their own:

- **AI-native** is no longer unusual; major productivity suites increasingly accept natural-language creation and editing.
- **A semantic or agent layer** is no longer unusual; incumbents are exposing richer semantic context and agent tool surfaces over their existing work ecosystems.

The remaining strategic thesis must therefore be stronger than "Office is legacy" or "Tachiko has AI." It rests on open canonical semantic state, deterministic computation and validation, meaningful version/review semantics, portability, user ownership, and provider/deployment neutrality.

## 2026 agent-first shift

### Microsoft 365 / Work IQ

Microsoft describes [Work IQ](https://www.microsoft.com/en-us/microsoft-365/blog/2026/06/02/announcing-the-new-work-iq-apis/) as the intelligence layer behind Microsoft 365. It builds semantic understanding from organizational content and exposes agent-oriented Chat, Context, Tools, and Workspaces surfaces.

The [Work IQ API overview](https://learn.microsoft.com/en-us/microsoft-365/copilot/extensibility/work-iq-api-overview) supports A2A, REST, and MCP access patterns.

Strategic implication:

- Tachiko Work cannot assume that legacy Office internals alone create a durable opening.
- An incumbent can place a semantic/agent layer above legacy applications and hide much of the user-facing cost of the old substrate.
- "Semantic" and "agent API" are therefore competitive table stakes, not sufficient moats.

This evidence does **not** establish that Word or Excel have replaced their internal document models with a Tachiko-like canonical semantic engine. It establishes that Microsoft is building an agent-facing semantic intelligence layer over the Microsoft 365 ecosystem.

### Google Workspace / Gemini

Google reported in March 2026 that [Gemini in Sheets can create, organize, and edit entire sheets from natural-language requests](https://blog.google/products-and-platforms/products/workspace/gemini-workspace-updates-march-2026/).

Google also provides [Google Workspace MCP servers](https://developers.google.com/workspace/guides/configure-mcp-servers) so AI applications can read and act on Workspace data across products such as Docs, Sheets, Drive, Gmail, Calendar, Slides, and Chat.

Strategic implication:

- Natural-language authoring can make traditional spreadsheet mechanics less visible to users without eliminating the underlying spreadsheet product.
- Supporting MCP or external agents is useful interoperability, but it is not by itself a differentiator.
- Tachiko must preserve value below the conversational surface: typed meaning, deterministic gates, validation, review, and open ownership.

### Anthropic / Claude work surfaces

Anthropic announced [Claude Docs and Claude Slides on 2026-09-16](https://claude.com/blog/cowork-is-now-claude). Claude Docs supports direct editing and collaborative work inside Claude; the [Claude Docs help documentation](https://support.claude.com/en/articles/16923645-get-started-with-claude-docs) also describes real-time collaborative editing and permission-scoped Claude actions.

Strategic implication:

- Model providers can become owners of first-party work surfaces, not merely assistants embedded into somebody else's editor.
- Tachiko should not answer this trend by binding its canonical work model to a different single model vendor.
- A durable role is to let Claude, ChatGPT, Gemini, local/self-hosted models, future agents, and non-AI clients operate over the same open semantic substrate.

## Existing categories

### Microsoft Office / Microsoft 365

Strengths:

- massive adoption and installed workflows;
- industry-standard exchange formats;
- enterprise distribution, identity, policy, and governance;
- increasingly deep Copilot / Work IQ agent integration.

Risks to Tachiko:

- semantic overlays can reduce the visible cost of legacy representation;
- Microsoft can combine work context, permissions, applications, and AI inside one tenant boundary;
- migration friction may outweigh architectural cleanliness unless Tachiko provides materially stronger guarantees.

### Google Workspace

Strengths:

- widely adopted cloud collaboration;
- increasingly capable Gemini-native authoring;
- remote MCP surfaces that permit external AI applications to act on Workspace data.

Risks to Tachiko:

- users can obtain natural-language productivity without changing their document substrate;
- open agent connectivity alone cannot justify migration.

### Anthropic Claude

Strengths:

- strong agent/conversational workflow;
- first-party Docs and Slides surfaces;
- ability to connect to external work systems.

Risks to Tachiko:

- an AI provider can own both the reasoning layer and the user's editing surface;
- users may accept provider-owned workspaces if portability and governance are "good enough."

### LibreOffice / ONLYOFFICE

Strengths:

- open-source Office alternatives;
- strong document compatibility.

Limitations relative to Tachiko's thesis:

- primarily preserve the traditional Office document model;
- do not make a shared typed semantic/computational substrate the central architectural authority.

### HackMD / Notion and related workspaces

Strengths:

- collaborative knowledge work;
- modern UX;
- increasingly agent-oriented workflows.

Competitive pressure:

- workspace products can add agent behavior incrementally over an existing data model;
- Tachiko must prove that deterministic semantic contracts, version/review quality, and open ownership create value beyond modern UX and embedded AI.

### Grist / Baserow

Strengths:

- spreadsheet plus database concepts;
- structured data.

Competitive pressure:

- validate demand for structured, user-friendly data tools;
- reduce the uniqueness of "spreadsheet, but structured" as a product story.

### Game data tools

Examples include Charon, Machinations, and other specialized game-data or balancing tools.

They validate the existence of the game-data problem, but Tachiko Work's game-development wedge is strongest when it demonstrates capabilities that generalize:

- stable semantic identity;
- deterministic calculation;
- typed validation;
- semantic diff/merge;
- reviewable AI-originated change;
- runtime projection without authoring-tool ownership.

## Strategic risk: conversational interfaces hide substrate differences

A future user may rarely manipulate cells, formulas, or document formatting directly. They may instead describe an outcome to an AI agent and inspect the result.

That world does not automatically invalidate Tachiko Work. It changes what must be valuable.

If conversational AI makes authoring cheap, the harder problems become:

- what exactly changed;
- whether the change was authorized;
- whether dependencies and constraints still hold;
- whether computation is deterministic and reproducible;
- whether the result can be reviewed, versioned, migrated, and reproduced;
- whether the work survives provider, pricing, policy, or product changes.

Tachiko therefore should not depend on direct manipulation UI as the primary moat. Grid, document, graph, and other visual surfaces remain valuable as projections for inspection, review, and thought, while canonical meaning remains below them.

## Strategic opportunity: model- and deployment-neutral substrate

The market trend toward vendor-owned AI work surfaces creates an inverse opportunity.

Tachiko Work can make the AI layer replaceable:

```text
hosted model ───────┐
self-hosted model ──┤
local/on-device ────┤
human / CLI ─────────┤
no AI ───────────────┘
                    ↓
              Semantic API
                    ↓
        canonical semantic substrate
        + deterministic computation
        + validation / authorization
        + provenance / semantic review
```

This positioning is only credible if the architecture preserves several guarantees:

- AI is a client, not semantic truth.
- Canonical work does not require one model provider.
- A remote AI service is not required for ownership or deterministic correctness.
- Local/self-hosted models cross the same safety and authorization boundaries as cloud models.
- Provider-specific protocols remain adapters rather than canonical semantics.
- Open representations and independent implementation remain real escape paths.

Accepted ADR-0007, ADR-0020, ADR-0026, and ADR-0027 provide the architectural basis for these guarantees; their provider- and deployment-neutral boundaries are design authority, not market inference. The founder-accepted direction in Issue #15 is supporting governance evidence for an open adoption/interoperability posture, and Issue #202 remains the legal implementation gate before any policy-dependent change. Issue #421 makes deployment neutrality explicit in ADR-0007.

## Strategic opportunity

The market remains fragmented across:

- Office documents;
- cloud workspaces;
- spreadsheets and databases;
- computational notebooks;
- game/domain data;
- version-control workflows;
- AI-agent work surfaces.

Tachiko Work should not attempt to win by owning every surface. The stronger hypothesis is that multiple surfaces and multiple AI providers can share one open, deterministic, reviewable semantic foundation.

That hypothesis still requires product validation. Architectural elegance alone is not evidence that users will migrate, that the format will gain network effects, or that the game-development wedge will expand into a broader platform.
