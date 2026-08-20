# Event Sourcing Model

## Principle

A Tachiko Work document can be understood as the result of applying semantic events.

## Model

```
Initial State
      |
      v
Semantic Events
      |
      v
Current Document State
```

## Advantages

- complete history
- reproducible states
- collaboration support
- AI reasoning over changes
- easier debugging

## Relationship with Git

Git records repository history.

Tachiko Work events record semantic history.

They complement each other.
