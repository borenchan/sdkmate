# Changelog

## 0.3.0 (2026-07-01)


### Features

* impl current cmd ([d10ff7e](https://github.com/borenchan/sdkmate/commit/d10ff7e4445dcd4e049df42e55d69000100cd460))
* extend env module - add remove_sdk_path and PATH helpers ([75d1474](https://github.com/borenchan/sdkmate/commit/75d147434fa9b580ec0743ce1dca8ed104c504ff))
* enhance util layer - constants, terminal macros, path helpers ([baeeeb7](https://github.com/borenchan/sdkmate/commit/baeeeb703a15bde62e5d5891eec06da1df0a8e74))
* enhance init command - dir check, step display, directory tree ([5b2b285](https://github.com/borenchan/sdkmate/commit/5b2b2855e240ab16177a34c4d7049b0df11806ad))
* enhance switch command - PATH conflict detection, extra_paths, config fields ([9c61aec](https://github.com/borenchan/sdkmate/commit/9c61aeccfebae0b4065608bbbbc6ea43fdca1ece))
* refactor install command into modular async architecture ([2d72e73](https://github.com/borenchan/sdkmate/commit/2d72e731a535e6415e670b124ca0ce7292248a43))
* refine CLI args - help text, doc comments, subcommand descriptions ([00c49ec](https://github.com/borenchan/sdkmate/commit/00c49eced389c7b451e4c1609e4ad9b2bfce5435))
* interactive TUI list command - local/remote version selector ([0cefeec](https://github.com/borenchan/sdkmate/commit/0cefeec15f1db3bef4a0a0d9987ca7766b3d20bf))
* switch rollback mechanism + BugReportError marking system ([9f61d64](https://github.com/borenchan/sdkmate/commit/9f61d6413b68b9c743b560f8a55019b88095737a))
* CLI exit code via ExitCode + bug report URL enhancements ([fed897a](https://github.com/borenchan/sdkmate/commit/fed897a33cb72bc24326f02b0d2cf31c7a274a03))


### Bug Fixes

* change CLI clap doc comments from Chinese to English ([5e0e467](https://github.com/borenchan/sdkmate/commit/5e0e467ac755ed03a232a2d17f62b04fe27b1a4f))
* allow empty bin_dir value (means binaries in SDK root dir) ([e1de8a9](https://github.com/borenchan/sdkmate/commit/e1de8a9408ed94a784158923b2a88c1b26e62af5))
* make --bin-dir optional for add-sdk (omit means root-dir binaries) ([c483d5e](https://github.com/borenchan/sdkmate/commit/c483d5e8a659845101c371637f496492e143b31b))
* add serde(default) to bin_dir field, empty = root-dir binaries ([2d88251](https://github.com/borenchan/sdkmate/commit/2d88251f23b1ef495b38ab0f3e42eaa41d5cc191))
* fix type mismatch in unix PATH removal filter ([28b2c43](https://github.com/borenchan/sdkmate/commit/28b2c43a6b4d729a578e372ba5ed17d1d248beb5))


### Refactor

* enhance util layer - terminal output, template rendering, SDK types ([428362d](https://github.com/borenchan/sdkmate/commit/428362dc526895a36beba6b54a9bfde4d5e00fd4))
* flatten manager module tree + split config.rs into focused files ([88cda7c](https://github.com/borenchan/sdkmate/commit/88cda7c2c1617300193900305bfa195f3a0742e7))
* move .tmp dir to sdkm_home root level ([6854537](https://github.com/borenchan/sdkmate/commit/68545374e0f016d0a2fa4a7c3209092dc3f76229))
* change bin_dir from String to Option<String> ([9db43e6](https://github.com/borenchan/sdkmate/commit/9db43e66fbcbf10fd469393bcb2f93b8258b6410))
* extract version resolution into shared version module ([45f4437](https://github.com/borenchan/sdkmate/commit/45f4437b5d38a3dc57eb682e593c2a231cd3ac3c))
* return ExitCode from cli.run() ([7932c89](https://github.com/borenchan/sdkmate/commit/7932c89ed25534eda6df41f29d72f6035182a557))


### Documentation

* add project architecture and progress docs ([18c902d](https://github.com/borenchan/sdkmate/commit/18c902d10181772c328e3dfbe3353d9f0e25e02d))
* rebuild Chinese and English README ([d75bcd1](https://github.com/borenchan/sdkmate/commit/d75bcd197f81e700002d2588cfc50b231a9a0e12))
* add detailed usage documentation under docs/ ([39470cd](https://github.com/borenchan/sdkmate/commit/39470cda82339640b5e1e5524d7d0cef60402278))
* add skills/SKILL.md usage skill for agents ([7f28532](https://github.com/borenchan/sdkmate/commit/7f28532803988dafb5f982fde17d1947ffbe32c7))


## 0.1.0 (2026-05-06)


### Features

* add config interface ([5b8baea](https://github.com/borenchan/sdkmate/commit/5b8baea69629929a0dec9308f55f3cb274487365))
* add switch command ([0c4ff05](https://github.com/borenchan/sdkmate/commit/0c4ff05cfc858b5e2a94d1e986b06390741d8e26))
* add switch command ([ff613f4](https://github.com/borenchan/sdkmate/commit/ff613f4dfa414004f9d130cf32c6a64b7d3ae375))
* impl init command ([ee7c534](https://github.com/borenchan/sdkmate/commit/ee7c5341a55c90c5e9cbcf300c7090a9c7020d9c))
* impl jdk switch ([0cb9bef](https://github.com/borenchan/sdkmate/commit/0cb9bef08b6aaad6d31f821f1d9834599b754321))
* impl jdk switch ([9c41f01](https://github.com/borenchan/sdkmate/commit/9c41f01b1d427ec3374fcdd2d860a39f77c404dc))
* perf ([cd395ef](https://github.com/borenchan/sdkmate/commit/cd395eff5bc74cf47ffe2cf4f7489a1fa2730e6a))
* supplement config key ([7bcb4df](https://github.com/borenchan/sdkmate/commit/7bcb4df9f1982a9b7ec5940c772d6098d71c6754))
* supplement sdkm config ([879186e](https://github.com/borenchan/sdkmate/commit/879186e2ef86fcbec4d57a32c4b194a4ff61be80))
* supplement sdkm config ([5371aba](https://github.com/borenchan/sdkmate/commit/5371aba23355ed373aaaf6c4530d2a92aa48e534))
* supplement sdkm config ([15df466](https://github.com/borenchan/sdkmate/commit/15df4669368763f18aeb9c717ffcab1095219b88))
* supplement sdkm config ([6490e9c](https://github.com/borenchan/sdkmate/commit/6490e9ced170b5944cbd0a9c1af5878e0103a0df))
* support unix env ([31fbd84](https://github.com/borenchan/sdkmate/commit/31fbd843fa694d851999f99e14861d8141f9f8c1))
* support unix env ([657b8d9](https://github.com/borenchan/sdkmate/commit/657b8d96d8c94bf6e196baa99b1438737b44adb0))
* use release-please for automated release workflow ([159fabc](https://github.com/borenchan/sdkmate/commit/159fabc86fbe45419d85d9df8eca244a6dc93f0f))
* 新增windows env impl ([84ca596](https://github.com/borenchan/sdkmate/commit/84ca596010bc8166a36c4a0f3e2d3e95511a3482))


### Bug Fixes

* add tag trigger for build job ([709274e](https://github.com/borenchan/sdkmate/commit/709274e013d5ddc09225912fb911e9fb6ea6f464))
* only run release-please on branch push, not tag push ([46bb70d](https://github.com/borenchan/sdkmate/commit/46bb70d68908574566a4e7d5a5d02f3c2c995fc0))
* set version directly in all crates to fix release-please ([c56e181](https://github.com/borenchan/sdkmate/commit/c56e181a3ecf466247996e3099172127286dd1a4))
* set version directly in package to fix release-please ([c662db8](https://github.com/borenchan/sdkmate/commit/c662db837199d0baf18ea461de8aaa651f170dce))
* simplify release workflow - release-please on branch push only ([c1e47fb](https://github.com/borenchan/sdkmate/commit/c1e47fbd5c754eb6661a45c9d150cfaf6a2bb039))
* use correct release-please action path ([d601aeb](https://github.com/borenchan/sdkmate/commit/d601aebc70e2f8a5e072239b5f6da47f62d6564b))
