# XLSX inspect-to-import source retention (#678 5d, #809)

Each native host and browser worker retains at most one decoded XLSX source.
The encoded inspect reply relinquishes its `SourceWorkbook` to this slot;
retention does not clone the decoded workbook. Import takes the slot before
comparing the full bytes, every `ImportOptions` field and the working-size
profile. A mismatch drops it and uses the existing full importer. Selection,
occurrence identity and installation are applied after source import and are
not part of source identity. CSV keeps its existing parse path.

The next inspect clears the slot before parsing, including failed inspections.
Import releases it whether matching, falling back or failing semantic admission.
Project close releases it even if no occurrence was installed. Native host drop
also releases it. No archive entries, XML trees or second decoded source are
retained; project-transfer and encoded-response arenas keep their existing rules.

For the #809 working-size budgets, the retained allocation consists of one
compressed input copy (at most **16 MiB**), the already-admitted decoded workbook
(at most **48 MiB serialized source**, **100,000 populated cells**, and
**10,000 total data rows × 32 column slots**), and the parsed options from the
existing **4 MiB** spreadsheet request bound. Ordinary inputs keep the existing
2 MiB source and 8 MiB expanded-archive limits; working inputs keep the existing
64 MiB expanded-archive limit. None of these admission limits changes.

These bounds are not a heap/RSS claim: empty decoded cells occupy Rust struct
storage, and strings/vectors have allocation capacity. For the qualification
workbook, the cell storage term is `300,000 * size_of::<SourceCell>()`, plus row
vectors, strings, column/layout metadata and compressed input. Against the
historical **256 MiB Stage A worker budget**, retention must be counted alongside
the resident occurrence and reply/transfer arenas, rather than treating the
48 MiB serialized-source limit as a total memory limit. During inspect this
workbook already exists; between inspect and import its lifetime is extended.
On a reuse hit it becomes the import's source without another source allocation.
On a miss it is dropped before the replacement is parsed.
