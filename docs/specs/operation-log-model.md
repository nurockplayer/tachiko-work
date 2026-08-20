# Semantic Operation Log Model

## Overview

Tachiko Work collaboration should operate on semantic operations instead of raw file replacement.

## Example

Instead of:

```
file changed
```

Represent:

```
UpdateField
 entity: Dragon
 field: hp
 old: 8000
 new: 9000
```

## Benefits

- meaningful history
- semantic merge
- AI review
- conflict explanation
- auditability

## Future

Operations can become the foundation for:

- realtime collaboration
- event sourcing
- offline editing
- AI generated changes
