# Tachiko Work Visual Direction

> Status: **Directional reference / non-normative**  
> Scope: architecture diagrams, product concept visuals, recruiting slides, ecosystem maps, and other high-level communication assets.

This document preserves the current visual direction so future contributors, designers, and agents have a shared starting point instead of reinventing the visual language each time.

It does **not** override the Product Constitution, Design Principles, Accepted ADRs, or normative specifications. Product or architecture labels shown in reference graphics are illustrative unless separately established by repository authority.

## Visual north star

Aim for a blend of:

- **Notion / Figma-like approachability**
- **Apple Developer-diagram-like clarity and maturity**
- **mature SaaS / developer-platform product feeling**
- **balanced product sense and technical credibility**

The result should feel like a designed product platform, not a raw infrastructure diagram.

![Tachiko Work architecture visual north star](assets/architecture-visual-north-star-v1.webp)

## Default visual language

### Calm foundation

- warm white or off-white background
- dark navy / charcoal typography
- generous whitespace
- subtle dividers and connectors
- very light shadows and elevation

### Card-based structure

Prefer:

- rounded cards
- pills / chips
- grouped panels
- simple, consistent icons
- clear visual hierarchy

Avoid relying on harsh box-and-arrow layouts when grouping and position can explain the relationship more clearly.

### Restrained color

Color should classify information, not decorate the whole canvas.

Typical accents:

- blue for core / platform / system concepts
- green for domain / application / data concepts
- purple for tooling or CLI-adjacent concepts
- warm orange for emphasis or opportunity areas

Keep the base palette neutral and let accent colors remain secondary.

### Space before arrows

Prefer spatial hierarchy over connector density.

A diagram should usually communicate its reading order through position and grouping first. Arrows and lines are supporting signals, not the primary explanation mechanism.

If a visual feels busy, remove or regroup elements before adding more decoration.

## Product language + technical language

Tachiko Work visuals may deliberately mix technical terms such as:

- Semantic Core
- Rust Engine
- Schema
- Diff
- Validation

with product-facing terms such as:

- 表格
- 看板
- 表單
- 儀表板
- 工作流程
- ERP / CRM
- 自訂 App

The goal is to stay credible to engineers without becoming inaccessible to designers or product-minded contributors.

## AI framing

AI should not visually become the center of the platform by default.

When appropriate, frame it alongside other operating surfaces such as:

`GUI · CLI · Git · AI`

This reinforces the idea that AI can operate on the same structured semantic state rather than introducing a separate data model or turning Tachiko Work into an AI wrapper.

## Recruiting visuals

When the audience is front-end engineers or designers, visuals should make the opportunity visible:

- information architecture
- interaction models
- editing experience
- view rendering
- dense-data UI
- design systems
- progressive disclosure

The intended message is that front-end and design contributors are not merely styling screens. They have meaningful product territory to define.

For Taiwan-facing recruiting material, default to **Traditional Chinese** while keeping established technical terms in English where that improves clarity.

## Avoid by default

Unless a specific context calls for it, avoid:

- cyberpunk aesthetics
- black-background neon UI
- generic blue-purple AI-startup glow
- excessive 3D or glossy effects
- dense UML / enterprise flowchart aesthetics
- excessive borders, arrows, labels, or decorative gradients
- mixed icon styles

## How to use this reference

Use this direction as the default starting point for:

- architecture diagrams
- platform concept diagrams
- recruiting slides
- product explanation graphics
- ecosystem maps
- front-end / design opportunity maps

Treat the reference image as a **visual north star, not a design law**. A future designer may evolve the system as the product matures, provided the resulting work remains clear, calm, product-led, and technically credible.
