# Changelog

## v0.4.5 - 2026-09-04


### ✨ Features

- two-layer ls TUI with registered SDK browser and TTY-aware fallbacks — [`262e1df`](https://github.com/borenchan/sdkmate/commit/262e1df)
- global layer dynamically reflects active sdks so enter applies switch instantly — [`be8934e`](https://github.com/borenchan/sdkmate/commit/be8934e)

### ⚡ Performance

- replace PS Remove-Item unset with null assignment — [`e4f36b4`](https://github.com/borenchan/sdkmate/commit/e4f36b4)

### 📝 Documentation

- update how-it-works for dynamic global layer and enter-to-apply — [`eae22c6`](https://github.com/borenchan/sdkmate/commit/eae22c6)

### 🧰 Chore

- release 0.4.5 — [`30003e1`](https://github.com/borenchan/sdkmate/commit/30003e1)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.4.4...v0.4.5



## v0.4.4 - 2026-08-31


### 🐛 Bug Fixes

- fish path line idempotent no-op when present and inserted above hook comment — [`923faad`](https://github.com/borenchan/sdkmate/commit/923faad)

### 🧰 Chore

- release 0.4.4 — [`067a4cf`](https://github.com/borenchan/sdkmate/commit/067a4cf)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.4.3...v0.4.4



## v0.4.3 - 2026-08-31


### 🐛 Bug Fixes

- fish path line relocated before hook to prevent base snapshot missing sdk bin — [`c110ade`](https://github.com/borenchan/sdkmate/commit/c110ade)

### 📝 Documentation

- simplify self uninstall confirmation message — [`52a9a35`](https://github.com/borenchan/sdkmate/commit/52a9a35)

### 🧰 Chore

- release 0.4.3 — [`b4caaba`](https://github.com/borenchan/sdkmate/commit/b4caaba)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.4.2...v0.4.3



## v0.4.2 - 2026-08-28


### 🐛 Bug Fixes

- hook cache invalidates on session env changes to preserve session-over-project priority — [`cb63d9f`](https://github.com/borenchan/sdkmate/commit/cb63d9f)

### 📝 Documentation

- add skill scripts — [`28a211a`](https://github.com/borenchan/sdkmate/commit/28a211a)
- update readme — [`9494c55`](https://github.com/borenchan/sdkmate/commit/9494c55)
- update star history — [`b4906e1`](https://github.com/borenchan/sdkmate/commit/b4906e1)

### 🧰 Chore

- release 0.4.2 — [`901eebc`](https://github.com/borenchan/sdkmate/commit/901eebc)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.4.1...v0.4.2



## v0.4.1 - 2026-08-27


### 🐛 Bug Fixes

- persist PATH to profile correctly on re-init — [`9fd47cb`](https://github.com/borenchan/sdkmate/commit/9fd47cb)

### 📝 Documentation

- add multi-scope version switching and shell hook docs — [`5ae3659`](https://github.com/borenchan/sdkmate/commit/5ae3659)

### 🧰 Chore

- release v0.4.1 — [`4b461f3`](https://github.com/borenchan/sdkmate/commit/4b461f3)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.4.0...v0.4.1



## v0.4.0 - 2026-08-26


### ✨ Features

- add fish support with pluggable shell backend — [`03bcc8d`](https://github.com/borenchan/sdkmate/commit/03bcc8d)
- add project-level SDK version management — [`f24bf48`](https://github.com/borenchan/sdkmate/commit/f24bf48)

### 🐛 Bug Fixes

- fuzzy-match project pin in check_project_references — [`7f312e8`](https://github.com/borenchan/sdkmate/commit/7f312e8)
- add unix stub for powershell_profile_paths to unblock linux build — [`47e3343`](https://github.com/borenchan/sdkmate/commit/47e3343)

### ♻️ Refactor

- enforce use-short-name via clippy absolute_paths lint — [`e664629`](https://github.com/borenchan/sdkmate/commit/e664629)

### ✅ Tests

- add shell backend core-entry integration tests — [`3336d07`](https://github.com/borenchan/sdkmate/commit/3336d07)

### 🧰 Chore

- release v0.4.0 — [`e8603ca`](https://github.com/borenchan/sdkmate/commit/e8603ca)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.3.5...v0.4.0



## v0.3.5 - 2026-08-12


### 🐛 Bug Fixes

- resolve dangling symlink replacement failure #6 — [`040b891`](https://github.com/borenchan/sdkmate/commit/040b891)

### 📝 Documentation

- move Windows admin note to Installation — [`5ddac03`](https://github.com/borenchan/sdkmate/commit/5ddac03)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.3.4...v0.3.5



## v0.3.4 - 2026-08-06


### 🐛 Bug Fixes

- require admin, try_step hints on perm errors, bug report adds version/OS — [`d6ebea4`](https://github.com/borenchan/sdkmate/commit/d6ebea4)

### 📝 Documentation

- update windows path desc — [`ef933c3`](https://github.com/borenchan/sdkmate/commit/ef933c3)

### 📌 Other

- Merge branch 'master' of https://github.com/borenchan/sdkmate — [`b23b326`](https://github.com/borenchan/sdkmate/commit/b23b326)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.3.3...v0.3.4



## v0.3.3 - 2026-08-06


### 🐛 Bug Fixes

- use HKCU for env vars so init works without admin — [`910de49`](https://github.com/borenchan/sdkmate/commit/910de49)

### 📝 Documentation

- add Go to built-in SDK listings in README and docs — [`77ce909`](https://github.com/borenchan/sdkmate/commit/77ce909)

### 🧰 Chore

- release v0.3.3 — [`5c8cc5d`](https://github.com/borenchan/sdkmate/commit/5c8cc5d)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.3.2...v0.3.3



## v0.3.2 - 2026-07-24


### ✨ Features

- auto-detect and add missing built-in SDKs to config — [`467e580`](https://github.com/borenchan/sdkmate/commit/467e580)
- add Go as built-in SDK — [`a25489c`](https://github.com/borenchan/sdkmate/commit/a25489c)

### 📝 Documentation

- update readme — [`08a367d`](https://github.com/borenchan/sdkmate/commit/08a367d)
- update readme — [`6a1577b`](https://github.com/borenchan/sdkmate/commit/6a1577b)

### 🧰 Chore

- release v0.3.2 — [`97a514d`](https://github.com/borenchan/sdkmate/commit/97a514d)
- ignore .workbuddy directory — [`74aa58f`](https://github.com/borenchan/sdkmate/commit/74aa58f)

### 📌 Other

- Merge branch 'master' of https://github.com/borenchan/sdkmate — [`b8df063`](https://github.com/borenchan/sdkmate/commit/b8df063)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.3.1...v0.3.2



## v0.3.1 - 2026-07-17


### 📝 Documentation

- document self update, install tmp cleanup, java aarch64 limit — [`641b866`](https://github.com/borenchan/sdkmate/commit/641b866)

### 🧰 Chore

- release v0.3.1 — [`acf04bb`](https://github.com/borenchan/sdkmate/commit/acf04bb)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.3.0...v0.3.1



## v0.3.0 - 2026-07-16


### ✨ Features

- add self update subcommand with backup and rollback — [`c4b7012`](https://github.com/borenchan/sdkmate/commit/c4b7012)
- make custom sdk download_url optional — [`e24eec5`](https://github.com/borenchan/sdkmate/commit/e24eec5)
- cache and parallelize sdk size; decouple from switch/uninstall — [`dc7407d`](https://github.com/borenchan/sdkmate/commit/dc7407d)
- add size column to ls/current and TUI uninstall key — [`d7a1a9d`](https://github.com/borenchan/sdkmate/commit/d7a1a9d)
- add self uninstall command — [`681696b`](https://github.com/borenchan/sdkmate/commit/681696b)
- add uninstall command — [`4aba334`](https://github.com/borenchan/sdkmate/commit/4aba334)

### 🐛 Bug Fixes

- clean stale install tmp; clarify java no-arch-build error — [`7802569`](https://github.com/borenchan/sdkmate/commit/7802569)
- persist size cache orphan prune on hot path — [`6760a34`](https://github.com/borenchan/sdkmate/commit/6760a34)

### ♻️ Refactor

- improve self-uninstall cleanup and confirmation UX — [`d59de3d`](https://github.com/borenchan/sdkmate/commit/d59de3d)

### 📝 Documentation

- codify review-before-deliver rule in CLAUDE.md — [`8067d3d`](https://github.com/borenchan/sdkmate/commit/8067d3d)
- update for optional download_url and size cache — [`a558629`](https://github.com/borenchan/sdkmate/commit/a558629)
- trim CLAUDE.md to keep only latest progress and non-obvious rules — [`45f29cf`](https://github.com/borenchan/sdkmate/commit/45f29cf)
- update CLAUDE.md progress for self-uninstall polish — [`fce5ba3`](https://github.com/borenchan/sdkmate/commit/fce5ba3)
- fix star-history embed URL (add repos= param) — [`239afde`](https://github.com/borenchan/sdkmate/commit/239afde)
- clarify extraction instructions to preserve .sdkm directory — [`4915bc6`](https://github.com/borenchan/sdkmate/commit/4915bc6)
- update CLAUDE.md for v0.2.7/v0.2.8 changes and branding — [`70c10c2`](https://github.com/borenchan/sdkmate/commit/70c10c2)
- switch logo to png, rename branding to sdkm, add star history chart — [`f9c868a`](https://github.com/borenchan/sdkmate/commit/f9c868a)

### 👷 CI / Build

- rename release artifact prefix from sdkmate to sdkm — [`33bfc37`](https://github.com/borenchan/sdkmate/commit/33bfc37)
- render contributors with avatars via GitHub commits API — [`459b270`](https://github.com/borenchan/sdkmate/commit/459b270)

### 🧰 Chore

- release v0.3.0 — [`7df8b23`](https://github.com/borenchan/sdkmate/commit/7df8b23)

### 🙌 Contributors

- <a href="https://github.com/borenchan"><img src="https://avatars.githubusercontent.com/u/96477641?v=4" width="32" height="32" alt="borenchan" /> borenchan</a>

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.8...v0.3.0



## v0.2.8 - 2026-07-08


### ✨ Features

- support download resume via HTTP Range requests — [`09b190a`](https://github.com/borenchan/sdkmate/commit/09b190a)

### 🧰 Chore

- release v0.2.8 — [`957139b`](https://github.com/borenchan/sdkmate/commit/957139b)

### 🙌 Contributors

- borenchan

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.7...v0.2.8



## v0.2.7 - 2026-07-08


### 🐛 Bug Fixes

- correct PATH export quoting that broke shell profile source — [`0c53f96`](https://github.com/borenchan/sdkmate/commit/0c53f96)

### 📝 Documentation

- update CLAUDE.md with v0.2.6 release postmortem — [`394a98d`](https://github.com/borenchan/sdkmate/commit/394a98d)

### 🧰 Chore

- release v0.2.7 — [`196731d`](https://github.com/borenchan/sdkmate/commit/196731d)

### 🙌 Contributors

- borenchan

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.6...v0.2.7



## v0.2.6 - 2026-07-08


### 📝 Documentation

- update CLAUDE.md for v0.2.6 cross-platform CI changes — [`4e7db26`](https://github.com/borenchan/sdkmate/commit/4e7db26)
- update binary size to ~4MB — [`52ba8fc`](https://github.com/borenchan/sdkmate/commit/52ba8fc)

### 👷 CI / Build

- fix linux cache pollution and musl target; cross-compile macos x86_64 — [`93b87ca`](https://github.com/borenchan/sdkmate/commit/93b87ca)
- add Contributors section to generated changelog — [`5546d7f`](https://github.com/borenchan/sdkmate/commit/5546d7f)
- add linux musl/gnu dual builds and pin macos deployment target — [`0ddbafd`](https://github.com/borenchan/sdkmate/commit/0ddbafd)

### 🧰 Chore

- release v0.2.6 — [`1914bf9`](https://github.com/borenchan/sdkmate/commit/1914bf9)
- release v0.2.7 — [`8c91d77`](https://github.com/borenchan/sdkmate/commit/8c91d77)
- release v0.2.6 — [`30ae5c0`](https://github.com/borenchan/sdkmate/commit/30ae5c0)

### 🙌 Contributors

- borenchan

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.5...v0.2.6



## v0.2.7 - 2026-07-08


### 📝 Documentation

- update CLAUDE.md for v0.2.6 cross-platform CI changes — [`4e7db26`](https://github.com/borenchan/sdkmate/commit/4e7db26)

### 👷 CI / Build

- fix linux cache pollution and musl target; cross-compile macos x86_64 — [`93b87ca`](https://github.com/borenchan/sdkmate/commit/93b87ca)

### 🧰 Chore

- release v0.2.7 — [`8c91d77`](https://github.com/borenchan/sdkmate/commit/8c91d77)

### 🙌 Contributors

- borenchan

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.6...v0.2.7



## v0.2.5 - 2026-07-07


### 🐛 Bug Fixes

- use |&p| pattern in filter closure to destructure &&str — [`ced01ea`](https://github.com/borenchan/sdkmate/commit/ced01ea)
- fix remove_sdk_path filter type mismatch — [`28c777d`](https://github.com/borenchan/sdkmate/commit/28c777d)
- make symlink_dir follow sdkm home and fix unix env bugs — [`6c1fa8b`](https://github.com/borenchan/sdkmate/commit/6c1fa8b)

### ⚡ Performance

- update reqwest version — [`9b84587`](https://github.com/borenchan/sdkmate/commit/9b84587)

### 🧰 Chore

- release v0.2.5 — [`2c516f3`](https://github.com/borenchan/sdkmate/commit/2c516f3)

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.4...v0.2.5



## v0.2.4 - 2026-07-06


### ✨ Features

- show sdkm home in current command; skip unregistered store dirs — [`6920d83`](https://github.com/borenchan/sdkmate/commit/6920d83)

### 👷 CI / Build

- exclude changelog backfill commit from generated changelog — [`059aba6`](https://github.com/borenchan/sdkmate/commit/059aba6)

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.3...v0.2.4



## v0.2.3 - 2026-07-03


### 🐛 Bug Fixes

- stop resolve spinner before prompt to prevent terminal output overlap — [`5df6925`](https://github.com/borenchan/sdkmate/commit/5df6925)

### 📝 Documentation

- update for v0.2.2 — [`020fe29`](https://github.com/borenchan/sdkmate/commit/020fe29)

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.2...v0.2.3



## v0.2.2 - 2026-07-03


### 🐛 Bug Fixes

- prompt users to restart terminal after init for PATH to take effect — [`2a35d0a`](https://github.com/borenchan/sdkmate/commit/2a35d0a)

### 📝 Documentation

- update for v0.2.1 — [`3651e56`](https://github.com/borenchan/sdkmate/commit/3651e56)

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.1...v0.2.2



## v0.2.1 - 2026-07-03


### 🐛 Bug Fixes

- stop flagging user-cancelled install as bug — [`5dee7de`](https://github.com/borenchan/sdkmate/commit/5dee7de)

### ⚡ Performance

- shrink release binary from 6.3MB to ~3MB — [`6dcac1d`](https://github.com/borenchan/sdkmate/commit/6dcac1d)

### ♻️ Refactor

- use resp.chunk() with 128KB BufWriter — [`5502bb5`](https://github.com/borenchan/sdkmate/commit/5502bb5)

### 📝 Documentation

- update for v0.2.1 — [`1ed580a`](https://github.com/borenchan/sdkmate/commit/1ed580a)
- update CLAUDE.md progress and notes — [`03ed2dc`](https://github.com/borenchan/sdkmate/commit/03ed2dc)
- rewrite core advantages with lightweight + AI — [`840a1f1`](https://github.com/borenchan/sdkmate/commit/840a1f1)
- update readme — [`dd8deb3`](https://github.com/borenchan/sdkmate/commit/dd8deb3)
- revamp README core advantages with competitor comparison and AI skill guide — [`67c5e85`](https://github.com/borenchan/sdkmate/commit/67c5e85)
- trim CLAUDE.md progress to latest entry + add release flow section — [`5ff76c2`](https://github.com/borenchan/sdkmate/commit/5ff76c2)

### ✅ Tests

- reorganize sdkcore tests by module name — [`45ddc11`](https://github.com/borenchan/sdkmate/commit/45ddc11)
- update test case — [`254b2c6`](https://github.com/borenchan/sdkmate/commit/254b2c6)

### 👷 CI / Build

- package binary inside .sdkm directory — [`cf8abbc`](https://github.com/borenchan/sdkmate/commit/cf8abbc)
- append changelog to root CHANGELOG.md on release — [`fd7fc37`](https://github.com/borenchan/sdkmate/commit/fd7fc37)

### 🧰 Chore

- reset CHANGELOG for v0.2.1 re-release — [`af1333b`](https://github.com/borenchan/sdkmate/commit/af1333b)
- remove stale per-crate CHANGELOG.md files — [`1a77a6f`](https://github.com/borenchan/sdkmate/commit/1a77a6f)

**Full Changelog**: https://github.com/borenchan/sdkmate/compare/v0.2.0...v0.2.1


