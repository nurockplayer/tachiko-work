# Engine Integration Strategy

## Goal

Connect Tachiko Work data with game engines without making the engine the source of truth.

Current release status: engine integrations are defined as future strategy, not
implemented CLI features.

## Unity

Possible integration:

- importer
- generated assets
- validation before build
- editor tooling

## Unreal Engine

Possible integration:

- generated data assets
- JSON/structured data pipeline
- build validation

## Godot

Possible integration:

- resource generation
- project data synchronization

## Principle

Game engines consume validated data.

They should not become the place where designers maintain authoritative balance data.
