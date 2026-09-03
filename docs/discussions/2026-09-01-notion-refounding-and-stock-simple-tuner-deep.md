# Notion Refounding and the Stock-Simple, Tuner-Deep Product Model

Status: Discussion history / product-strategy synthesis. **Not authoritative by itself.**

Captured: 2026-09-01  
Tracking issue: #215

## Why this discussion matters

This discussion began as research into Notion as a company, especially its 2015 Kyoto refounding, and became a clearer statement of what Tachiko Work should feel like as a product.

The most important clarification was not architectural. It was motivational:

> **The ordinary user should not need Microsoft Office on their computer.**

Terms such as semantic core, typed model, projections, deterministic computation, or reusable domain assets may be necessary engineering concepts, but they are not the user's reason to adopt Tachiko Work.

The product should therefore separate two questions:

1. What result does the user want?
2. What machinery must Tachiko build underneath to make that result durable, extensible, portable, and safe?

This memo preserves the reasoning that connected Notion's history to that distinction.

## Notion's original mission was not a note-taking app

Notion started in 2013 with a much broader ambition: make it possible for ordinary computer users to shape and assemble their own software tools.

Ivan Zhao later described the first several years as the company's "lost years." Notion rebuilt the product repeatedly, and the first direct attempt at making software creation accessible to non-programmers did not resonate. The problem was not simply that the interface was too difficult. Most people did not wake up wanting to "build software" at all. They wanted to finish reports, organize work, track projects, or manage information.

Notion's mission survived, but the product form changed.

Primary source:
- https://www.notion.com/blog/100-million-of-you

## The 2015 crisis and Kyoto refounding

By 2015, Notion had several failures stacked on top of one another:

- the early product had weak user resonance;
- the company had not found product-market fit;
- its implementation used a technical stack that the founders later judged unsuitable;
- the product was unstable;
- cash was running down;
- maintaining the small team at the existing burn rate was no longer credible.

Figma's 2019 retrospective records Ivan Zhao's description of the choice: the founders could keep the team and likely run out of cash, or reduce the company to the founders and start over.

Ivan and Simon Last laid off the small team, sublet their San Francisco space, and went to Japan to rebuild.

The choice of Kyoto was less mystical than later retellings sometimes suggest. In a later Sequoia interview, Ivan said they initially considered Tokyo, found the available apartments too cramped, and chose Kyoto partly because housing was larger and cheaper. Renting out their San Francisco home and office while operating cheaply in Japan even made the company cash-flow positive for the first time.

The psychological reset mattered too. After laying off friends and colleagues, they wanted a place that felt completely different.

Primary sources:
- https://sequoiacap.com/podcast/notions-ivan-zhao-the-refounder
- https://www.figma.com/blog/design-on-a-deadline-how-notion-pulled-itself-back-from-the-brink-of-failure/

## What Kyoto contributed

Kyoto did not magically cause Notion to succeed. Ivan has explicitly said he believes they probably could have rebuilt elsewhere.

But Kyoto became important to Notion's internal story because it reinforced a particular attitude toward software as craft.

In an interview published by Kyoto City, Ivan described being struck by:

- architecture that was simple, beautiful, functional, and durable;
- traditional craft refined over decades;
- the idea of making a tool exceptionally well rather than treating software as disposable;
- hospitality and service as part of product quality.

This fed Notion's long-running emphasis on craft. Ivan later compared Notion itself with tools such as knives, ceramics, and other objects that craftspeople refine over very long periods.

Primary source:
- https://kyo-working.city.kyoto.lg.jp/article/report/2022-04-04/

### A historical precision note

Public first-party accounts do not line up perfectly on the duration of the Kyoto stay.

The Kyoto City interview describes roughly one month in Kyoto. Other retrospectives describe a longer "year" of rebuilding around this period, while Ivan's later Sequoia interview says the from-scratch rebuild took about a year and a half.

The safe historical conclusion is therefore:

> Kyoto was a real refounding point and an important development base, while the complete rebuild extended beyond any single one-month stay.

Do not turn the story into an unsupported claim that the entire rebuild happened continuously inside one Kyoto apartment for a fixed number of months.

## The Figma signal

During the rebuild, Figma noticed Ivan abruptly appear at the top of its internal most-active-user list, with reported use exceeding 18 hours in a day.

This is not important because of the exact hour count. It shows how intensely the founders were iterating on the product's interaction model.

Figma's account describes a process of creating large numbers of permutations rather than polishing a single early answer:

```text
problem
  ↓
variation A / B / C / D / ...
  ↓
small changes to interaction, copy, iconography, hierarchy, layout
  ↓
compare / stress-test / discard
  ↓
keep the strongest direction
```

Ivan and Simon did not operate as a strict designer-to-engineer assembly line. Both could participate in design and implementation, and Figma became a shared product-thinking surface.

The company also deliberately preserved old design permutations. New hires could inspect how product decisions evolved instead of seeing only the final answer.

Primary source:
- https://www.figma.com/blog/design-on-a-deadline-how-notion-pulled-itself-back-from-the-brink-of-failure/

## Sugar-coated broccoli

One of Ivan's clearest explanations of Notion's early lesson is "sugar-coated broccoli."

The broccoli is the deeper mission:

> ordinary people should be able to create and adapt their own software tools.

The sugar is the familiar job users already understand and want:

- documents;
- notes;
- wikis;
- tables;
- project tracking;
- calendars;
- other ordinary productivity workflows.

Early Notion exposed too much of the broccoli first. Later Notion let users begin with a familiar productivity task and only gradually discover the deeper composability underneath.

The transferable product lesson is:

> **Do not require users to understand the platform before the platform solves their problem.**

Or, more operationally:

> **Time-to-value should come before time-to-understanding.**

Source:
- https://www.lennysnewsletter.com/p/inside-notion-ivan-zhao

## LEGO bricks and LEGO boxes

Ivan later described a related lesson using LEGO.

Hardcore users may care about the individual bricks. Most customers care about the box: a recognizable solution they can use immediately.

For Notion, the bricks include generic capabilities such as databases, relations, views, and other composable primitives. The boxes are recognizable solutions such as project management, CRM-like workflows, knowledge management, or other packaged use cases.

The lesson is not to stop building reusable primitives. It is to avoid forcing the customer to assemble those primitives before obtaining value.

Conceptually:

```text
behind the scenes
reusable bricks
    ↓
packaged solution
    ↓
user gets value
    ↓ optional
user opens the box further
    ↓
customizes / composes / builds
```

This maps directly to #192's domain-solution hypothesis, but adds a missing product constraint: a domain solution must be usable as a product, not merely representable as a collection of primitives.

## The Tachiko clarification: semantic is under the hood

A recurring architectural description of Tachiko Work is that it is semantic-first.

That is useful internally, but the discussion clarified that "semantic" is not the motivating user story.

In plain language, the semantic model exists so Tachiko can know that a value is not merely `100`, but perhaps:

```text
Product.price = 100 JPY
```

That deeper meaning enables typed relationships, formulas, validation, multiple views, meaningful history, and AI operations without asking the AI to infer everything from screen coordinates or opaque files.

But this is analogous to the engine and chassis of a vehicle. A driver should not need to know how they were manufactured in order to drive.

The visible product goal is closer to:

```text
spreadsheet-like work
+ documents
+ presentation-like surfaces where supported
+ structured/domain workflows
+ gradual Office migration
        ↓
without requiring Microsoft Office
```

Underneath, Tachiko may use a shared semantic/runtime foundation so those surfaces do not become three unrelated application silos.

## Founder metaphor: Supra, Civic, GT-R

The product model became clearest through a car analogy:

> **A driver should not need to know how the engine or chassis was manufactured. But an enthusiast who wants to modify the product should discover the kind of headroom associated with platforms such as the Supra, Civic, or GT-R.**

The point is not horsepower or motorsport branding. It is the combination of two properties that are often treated as opposites:

1. the stock product is already useful and approachable;
2. the underlying platform has unusually deep modification potential.

This leads to the phrase:

> **stock-simple, tuner-deep**

Issue #215 owns this strategy framing.

## Driver → Enthusiast → Builder

The car metaphor yields a useful product ladder.

### Driver

The Driver wants to work, not study Tachiko architecture.

Typical expectations:

```text
open
→ create / import
→ edit
→ calculate / organize
→ save / share / export
→ finish the job
```

A Driver should not need to understand semantic models, projection caches, stable IDs, package taxonomy, Git, or extension boundaries.

### Enthusiast

The Enthusiast chooses to open the hood.

Possible surfaces include:

- field/type customization;
- relationships;
- formulas;
- validation;
- views;
- templates;
- workflows / Skills;
- reusable domain assets.

The critical rule is:

> **Ordinary users must never be forced to open the hood, but the hood must not be welded shut.**

### Builder

The Builder wants to create reusable capabilities for other people:

- domain solutions;
- templates and declarative packs;
- Skills/workflows;
- connectors and integrations;
- executable extensions where genuinely required;
- services, migration packages, or support offerings.

This maps into the contribution ladder already being researched in #176.

## Why "tuner-deep" is more demanding than simple extensibility

Many applications technically support extensions while still keeping meaningful product behavior sealed.

Tachiko should distinguish cosmetic customization from genuine modification headroom.

Long-term, a strong modification platform needs properties analogous to a healthy automotive aftermarket:

### Stable modification points

Changing a view, workflow, domain asset, or integration should not require modifying Tachiko core.

### No full-product fork for routine customization

A creator should not have to fork the entire application to produce a different inventory workflow, CRM solution, validation pack, or presentation surface.

### Shared foundation

Driver, Enthusiast, and Builder experiences should not maintain different semantic truths.

### Inspectability

Users should be able to understand what a customization contributes and what authority it has.

### Boundaries that remain firm

"Modifiable" must not mean every invariant is negotiable. Correctness, authorization, deterministic authority, interoperability, and safety may require some interfaces to remain intentionally constrained.

## Product implications

This discussion suggests several durable questions for future product work:

1. **What is the minimum stock experience?**  
   Tachiko needs complete out-of-box jobs, not only powerful primitives.

2. **Where should progressive disclosure happen?**  
   Advanced structure should appear when useful rather than dominate first contact.

3. **Which internal terms are leaking into product UX?**  
   Architecture vocabulary should not become an adoption tax.

4. **Can packaged domain solutions remain editable underneath?**  
   A CRM or inventory solution should be a useful box while still exposing the bricks to users who want them.

5. **Which modification points deserve stable contracts?**  
   #176 and related research should answer this from evidence rather than from analogy alone.

6. **What should remain impossible to modify?**  
   A trustworthy platform needs invariant boundaries, not unlimited configuration.

7. **Can users own their modified vehicle?**  
   Templates, packs, workflows, and domain solutions should preserve Tachiko's existing user-ownership and portability goals rather than creating a new lock-in layer.

## Relationship to existing repository direction

This discussion does not replace existing authority.

It clarifies how several existing directions can fit into one understandable product model:

```text
Founding motivation
reduce dependence on Microsoft Office / closed document ecosystems
        ↓
stock product
familiar useful work surfaces
        ↓
under the hood
shared semantic/runtime foundation
        ↓
enthusiast layer
inspect / customize / compose
        ↓
builder layer
reusable assets / domain solutions / Skills / extensions / services
```

Relevant owners:

- `docs/vision/mission.md` — founding motivation and user ownership;
- `docs/vision/design-principles.md` — semantic core, multiple views, small core, open ecosystem;
- #18 — gradual Office/Excel migration;
- #170 — first-party non-CLI Designer workflow;
- #176 — contribution primitives and creator ladder;
- #177 — reusable template/project asset contract;
- #192 — vertical applications as composable domain solutions;
- #215 — stock-simple, tuner-deep product philosophy.

## What may eventually become canonical

The historical details about Kyoto, Figma usage, broccoli, LEGO, Supra/Civic/GT-R, and this discussion path belong in discussion history.

If product evidence supports the framing, the durable canonical principle should probably be much shorter:

> **Tachiko should be easy to use in stock form, while preserving deep, inspectable, composable modification headroom for users who choose to go further.**

A second product principle may be:

> **Do not require users to understand Tachiko's architecture before Tachiko solves their problem.**

Those principles can later be promoted into `docs/vision/mission.md` or `docs/vision/design-principles.md` through the repository's normal authority process.
