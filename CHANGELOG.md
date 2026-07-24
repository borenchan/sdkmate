# Changelog

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


