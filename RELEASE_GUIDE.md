# Release Guide for HyperStudy Bridge

## Overview
This guide explains how to create and manage releases for HyperStudy Bridge using GitHub's release system.

## Release Strategy

### Version Numbering
We follow [Semantic Versioning](https://semver.org/) (SemVer):
- **MAJOR.MINOR.PATCH** (e.g., v1.0.0)
  - **MAJOR**: Breaking changes
  - **MINOR**: New features (backward compatible)
  - **PATCH**: Bug fixes

### Pre-releases
- **Alpha**: `v1.0.0-alpha.1` - Early testing, may be unstable
- **Beta**: `v1.0.0-beta.1` - Feature complete, testing phase
- **RC**: `v1.0.0-rc.1` - Release candidate, final testing

## Creating a Release

### Method 1: Automated Script (Recommended)
```bash
# Create a patch release (bug fixes)
./scripts/create-release.sh

# Or specify version directly
./scripts/create-release.sh v1.0.0
```

The script will:
1. Update version in all config files
2. Commit the changes
3. Create and push a git tag
4. Trigger GitHub Actions to build and release

### Method 2: Manual Process
```bash
# 1. Update version in files
vim src-tauri/Cargo.toml      # Update version
vim src-tauri/tauri.conf.json  # Update version
vim package.json               # Update version

# 2. Commit changes
git add -A
git commit -m "chore: Bump version to v1.0.0"

# 3. Create tag
git tag -a v1.0.0 -m "Release v1.0.0"

# 4. Push everything
git push origin main
git push origin v1.0.0
```

### Method 3: GitHub UI
1. Go to https://github.com/ljchang/hyperstudy-bridge/actions
2. Click on "Create Release" workflow
3. Click "Run workflow"
4. Enter version (e.g., v1.0.0)
5. Click "Run workflow"

## What Happens After Creating a Tag

1. **GitHub Actions Triggered**: The `create-release.yml` workflow starts
2. **Release Created**: A draft release is created on GitHub
3. **macOS Builds**: Both Intel and ARM versions are built
4. **Code Signing**: Binaries are signed with Developer ID certificate
5. **Notarization**: Apple notarizes the apps
6. **Upload**: DMG files are uploaded to the release
7. **Published**: Release becomes publicly available

## Release Checklist

### Before Release
- [ ] All tests passing
- [ ] Update CHANGELOG.md with release notes
- [ ] Version numbers updated in all files
- [ ] No uncommitted changes
- [ ] Branch is main and up to date

### After Release
- [ ] Verify release appears on GitHub
- [ ] Download and test DMG files
- [ ] Verify Gatekeeper acceptance (macOS)
- [ ] Update documentation if needed
- [ ] Announce release (if applicable)

## GitHub Release Page

Releases appear at: https://github.com/ljchang/hyperstudy-bridge/releases

Each release includes:
- Release notes (auto-generated from commits)
- Binary downloads for each platform
- Source code archives
- Installation instructions

## Best Practices

### DO:
- ✅ Test thoroughly before releasing
- ✅ Use semantic versioning consistently
- ✅ Tag from main branch only
- ✅ Write clear commit messages (they become release notes)
- ✅ Use pre-release tags for testing

### DON'T:
- ❌ Delete or modify existing tags
- ❌ Release with failing tests
- ❌ Skip version numbers
- ❌ Release from feature branches
- ❌ Forget to update version in config files

## Troubleshooting

### Release workflow fails
1. Check GitHub Actions logs
2. Verify certificates haven't expired
3. Check that all secrets are configured

### Notarization fails
1. Verify APPLE_ID_PASSWORD is app-specific password
2. Check Apple Developer account is active
3. Ensure certificate matches Team ID

### DMG not appearing in release
1. Wait for workflow to complete (5-10 minutes)
2. Check artifacts in workflow run
3. Manually upload if needed

## Rolling Back a Release

If you need to remove a release:
1. Delete the release on GitHub (keeps the tag)
2. Delete the tag locally: `git tag -d v1.0.0`
3. Delete remote tag: `git push origin :refs/tags/v1.0.0`
4. Fix issues and create new release

## Monitoring Releases

### Download Statistics
View download counts on the releases page to track adoption.

### Update Notifications
Future versions will include auto-update functionality using Tauri's updater.

## Security

All macOS releases are:
- Signed with Developer ID certificate
- Notarized by Apple
- Verified by Gatekeeper
- Include hardened runtime protections

## Support

For release-related issues:
1. Check GitHub Actions logs
2. Review this guide
3. Check certificate expiration dates
4. Contact repository maintainers