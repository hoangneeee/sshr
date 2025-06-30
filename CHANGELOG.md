# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added


### Changed / Fixed


### Removed


---

### [0.6.0] - 2025-06-30

### Changed / Fixed
- Update UI v2
- Fix minor bug

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

