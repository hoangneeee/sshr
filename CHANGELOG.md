# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed / Fixed

### Removed

---

## [0.10.4] - 2026-04-29

### Added

- Pure-Rust SFTP via `russh` + `russh-sftp` with a persistent SSH session (no more `ssh`/`scp` shell-out for SFTP)
- TUI password prompt overlay as auth fallback when ssh-agent and `~/.ssh` keys fail (with retry-on-bad-password)
- Typed `SshClientError` enum so the UI can distinguish "needs password" from other failures
- Screen stack architecture (`AppScreen` enum + `ScreenAction`) replacing the old `InputMode` flag

### Changed / Fixed

- Refactored the `App` god object: 25 → 7 fields, with a `SessionState` enum that makes SSH and SFTP mutually exclusive at compile time
- Unified everything on the tokio async runtime; removed ad-hoc threads and polling timeouts
- SFTP directory navigation: ~200ms → ~10ms by reusing the persistent SFTP session instead of opening a new SSH connection per `cd`
- SSH foreground session now suspends the TUI cleanly (zero-CPU while attached)
- Fixed upload progress bug where the loop wrote to `scp`'s stdin while `scp` was actually reading from a positional path — progress reflected local reads, not real bytes on the wire
- Fixed an upload/download race in the transfer worker
- `StrictHostKeyChecking` now honors `~/.ssh/known_hosts` through the russh client handler

### Removed

- `shell-escape` dependency (no longer shelling out for SFTP)
- `ssh`/`scp` subprocess code paths for SFTP listing and transfer
- `InputMode` enum and related branching (replaced by the screen stack)
- Obsolete constants: `SSH_REMOTE_LIST_TIMEOUT_S`, `TRANSFER_PROGRESS_POLL`, `DOWNLOAD_NO_PROGRESS_TIMEOUT_S`

---

## [0.9.0] - 2026-03-07

### Changed / Fixed

- Implement architecture
- Fixed read SSH config host on Ubuntu

---

## [0.8.0]

### Changed / Fixed

- Update UI SFTP

---

### [0.7.0] - 2025-07-01

### Added

- Update UI search mode to user friendly

### Changed / Fixed

- Fix bug restore tui when has error in ssh mode

---

### [0.5.0] - 2025-06-08

### Added

- Feature SFTP mode

### Changed / Fixed

- Update docs keyboard shortcuts
- Upgrade README.md
- Fix scroll list view ssh mode and sftp mode

---

## [0.4.0] - 2025-06-06

### Features

- Press s to search

---

## [0.3.0] - 2025-06-05

### Changed

- Use edit action instead of add, delete action
- Move logic handle pressed key to `app.rs`
- Upgrade UI with loading animation

### Performance

- Use main thread and run ssh thread

---

## [0.2.0] - 2025-06-03

### Added

- Add formula support homebrew
- Add version flag
- Read my config
- Support reload config
- Can user custom host file with `hosts.toml`

### Changed

- Upgrade README.md
- Change log file name

### Fixed

- Workflows release work on windows

---

## [0.1.0] - 2025-06-02

### Added

- Read ssh host from ~/.ssh/config
- Support connect to ssh host
- Show list ssh host in TUI
