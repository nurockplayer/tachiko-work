# Live implementation queue

Canonical coordination: https://github.com/nurockplayer/tachiko-work/issues/357

| Stage | Owning issue | Delivery repository | Lead | Depends on |
|---|---|---|---|---|
| C0: tests/tooling-only independent bootstrap and seed qualification | https://github.com/nurockplayer/tachiko-work/issues/358 | new client + producer scratch checkout | Astra High | none |
| C1: pinned experimental kit and canonical I/O | https://github.com/nurockplayer/tachiko-work/issues/359 | tachiko-work | Terra High | qualified C0 evidence |
| C2: linked Table/Brief and real browser save/reopen | https://github.com/nurockplayer/tachiko-work/issues/360 | new tachiko-work-client | Astra High | C0, C1 |
| C3: trusted delegated consumer bridge | https://github.com/nurockplayer/tachiko-work/issues/361 | tachiko-work | Terra High | C0, stable C1 boundary |
| C4: contextual proposal review/apply | https://github.com/nurockplayer/tachiko-work/issues/362 | new tachiko-work-client | Astra Medium | C2, C3 |
| C5: same client in macOS host | https://github.com/nurockplayer/tachiko-work/issues/363 | new tachiko-work-client | Terra High / Astra Medium | C1, C2; C3/C4 only for available agent capability |

Only C0 tests/tooling preflight is currently authorized. All production children
remain refine pending the existing Steward Ready decision. This table is an index,
not a second authority or permission for an agent to promote itself to Ready.

C2 and C3 can run concurrently after the consumer boundary stabilizes. Keep one
writer per branch/PR and an explicit integration owner. Luna Medium handles
isolated mechanical, accessible presentation and unit-test work, not semantic,
authorization or storage contract design. Use fresh independent deep final-head
review for Guarded work. No named local sub-agent profile is assumed to exist.

The issues are hosted upstream for now because the new remote has not been created.
When a client issue is transferred, leave its canonical successor link upstream;
do not maintain competing editable issue specifications in both repositories.
