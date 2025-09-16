# Changelog

All notable changes to HyperStudy Bridge will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive release automation with GitHub Actions
- Automated changelog generation from commits
- macOS code signing and notarization pipeline
- Release creation scripts and documentation

## [0.6.0] - 2025-01-15

### Added
- Complete Phase 6: Documentation and Deployment Infrastructure
- macOS code signing with Developer ID certificates
- Apple notarization support for Gatekeeper approval
- Comprehensive API documentation
- User guide and troubleshooting documentation
- Developer guide with architecture details
- GitHub Actions workflows for automated builds
- Release automation scripts

### Fixed
- GitHub Actions secret name mismatch (APPLE_PASSWORD vs APPLE_ID_PASSWORD)
- DMG creation failures in CI/CD pipeline
- Rust formatting issues in test files

### Changed
- Removed universal binary creation from release workflow
- Simplified release process to separate Intel and ARM builds

## [0.5.0] - 2025-01-14

### Added
- Phase 4: Testing and Accessibility
- Comprehensive unit tests for all device modules
- Integration tests for WebSocket bridge
- End-to-end testing with Playwright
- Accessibility improvements (ARIA labels, keyboard navigation)
- Performance benchmarks

### Fixed
- CI/CD pipeline failures
- ESLint configuration for Svelte 5
- Rust clippy warnings

## [0.4.0] - 2025-01-13

### Added
- Lab Streaming Layer (LSL) device integration
- Device configuration persistence
- Performance monitoring dashboard
- Real-time connection status updates
- Comprehensive logging system

### Fixed
- WebSocket reconnection logic
- Device disconnection handling
- Memory leaks in data streaming

## [0.3.0] - 2025-01-12

### Added
- Svelte 5 frontend with modern UI
- Device selection modal
- Status dashboard with real-time updates
- Settings panel for configuration
- Log viewer for debugging

### Changed
- Upgraded to Svelte 5 with runes
- Migrated to Tailwind CSS v4
- Improved UI/UX consistency

## [0.2.0] - 2025-01-11

### Added
- Core backend architecture
- TTL Pulse Generator support
- Kernel Flow2 fNIRS integration
- Pupil Labs Neon eye tracker support
- Biopac physiological monitoring
- WebSocket bridge server

### Fixed
- Serial port communication issues
- TCP socket connection stability
- Data streaming performance

## [0.1.0] - 2025-01-10

### Added
- Initial project setup with Tauri
- Basic application structure
- Development environment configuration
- CI/CD pipeline foundation

[Unreleased]: https://github.com/ljchang/hyperstudy-bridge/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/ljchang/hyperstudy-bridge/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/ljchang/hyperstudy-bridge/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/ljchang/hyperstudy-bridge/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/ljchang/hyperstudy-bridge/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/ljchang/hyperstudy-bridge/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ljchang/hyperstudy-bridge/releases/tag/v0.1.0