# Deployment Guide

This document outlines the deployment process for HyperStudy Bridge, including required configurations, secrets, and CI/CD workflows.

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [GitHub Secrets Configuration](#github-secrets-configuration)
- [Code Signing Setup](#code-signing-setup)
- [Release Process](#release-process)
- [CI/CD Workflows](#cicd-workflows)
- [Troubleshooting](#troubleshooting)

## Overview

HyperStudy Bridge uses GitHub Actions for automated CI/CD, supporting:

- **Continuous Integration**: Automated testing, linting, and building on push/PR
- **Automated Releases**: Cross-platform builds with code signing and notarization
- **Dependency Management**: Automated dependency updates via Dependabot
- **Security**: Vulnerability scanning and dependency review

## Prerequisites

### Development Tools

- **Node.js**: Version 20.x or later
- **Rust**: Latest stable version
- **Platform-specific requirements**:
  - **macOS**: Xcode Command Line Tools
  - **Windows**: Microsoft C++ Build Tools
  - **Linux**: Build essentials and GTK development libraries

### Accounts and Certificates

- **Apple Developer Account** (for macOS code signing)
- **Windows Code Signing Certificate** (optional, for Windows releases)
- **GitHub repository** with Actions enabled

## GitHub Secrets Configuration

Configure the following secrets in your GitHub repository settings (`Settings > Secrets and variables > Actions`):

### Required Secrets

#### Apple Code Signing (macOS)

| Secret Name | Description | Example/Format |
|-------------|-------------|----------------|
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application certificate (.p12) | `LS0tLS1CRUdJTi...` |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the .p12 certificate file | `your-cert-password` |
| `APPLE_ID` | Apple ID email for notarization | `developer@company.com` |
| `APPLE_PASSWORD` | App-specific password for Apple ID | `abcd-efgh-ijkl-mnop` |
| `APPLE_TEAM_ID` | Apple Developer Team ID | `ABCDE12345` |
| `APPLE_SIGNING_IDENTITY` | Code signing identity name | `Developer ID Application: Company Name (ABCDE12345)` |

#### Tauri Updater (Optional)

| Secret Name | Description | Example/Format |
|-------------|-------------|----------------|
| `TAURI_PRIVATE_KEY` | Private key for Tauri updater signing | `-----BEGIN PRIVATE KEY-----...` |
| `TAURI_KEY_PASSWORD` | Password for the private key | `key-password` |

#### Windows Code Signing (Optional)

| Secret Name | Description | Example/Format |
|-------------|-------------|----------------|
| `WINDOWS_CERTIFICATE` | Base64-encoded code signing certificate | `LS0tLS1CRUdJTi...` |
| `WINDOWS_CERTIFICATE_PASSWORD` | Certificate password | `cert-password` |

#### Additional Secrets

| Secret Name | Description | Example/Format |
|-------------|-------------|----------------|
| `KEYCHAIN_PASSWORD` | macOS keychain password (optional, defaults to 'actions') | `secure-password` |
| `CODECOV_TOKEN` | Codecov upload token for coverage reports | `abc123def456...` |

### Secret Setup Instructions

#### 1. Apple Code Signing Setup

**Generate Developer ID Certificate:**

1. Open **Keychain Access** on macOS
2. Go to **Keychain Access > Certificate Assistant > Request a Certificate from a Certificate Authority**
3. Enter your email and name, select "Saved to disk"
4. Upload the CSR to [Apple Developer Portal](https://developer.apple.com/account/resources/certificates/list)
5. Create a "Developer ID Application" certificate
6. Download and install the certificate in Keychain Access

**Export Certificate:**

```bash
# Export certificate from Keychain Access
# Right-click certificate > Export > Personal Information Exchange (.p12)
# Set a password and save as certificate.p12

# Convert to base64 for GitHub Secret
base64 -i certificate.p12 | pbcopy
# Paste the output as APPLE_CERTIFICATE secret
```

**Generate App-Specific Password:**

1. Go to [Apple ID Account](https://appleid.apple.com/account/manage)
2. Sign in and go to "Security" section
3. Generate an app-specific password
4. Use this as `APPLE_PASSWORD` secret

**Find Team ID:**

1. Go to [Apple Developer Portal](https://developer.apple.com/account/)
2. Find your Team ID in the top-right corner or membership details

#### 2. Tauri Updater Setup (Optional)

**Generate Updater Key Pair:**

```bash
# Generate private key
openssl genpkey -algorithm Ed25519 -out private.key

# Extract public key
openssl pkey -in private.key -pubout -out public.key

# Convert private key to PEM format for GitHub Secret
cat private.key | base64 | pbcopy
# Paste as TAURI_PRIVATE_KEY secret
```

**Add Public Key to Tauri Config:**

```json
// src-tauri/tauri.conf.json
{
  "updater": {
    "active": true,
    "endpoints": [
      "https://github.com/your-org/hyperstudy-bridge/releases/latest/download/updater.json"
    ],
    "dialog": true,
    "pubkey": "YOUR_PUBLIC_KEY_HERE"
  }
}
```

#### 3. Windows Code Signing (Optional)

**Export Certificate:**

```bash
# Export from Windows Certificate Store as .pfx
# Convert to base64
certutil -encode certificate.pfx certificate_base64.txt
# Use content as WINDOWS_CERTIFICATE secret
```

## Release Process

### Automatic Releases

Releases are automatically triggered when pushing version tags:

```bash
# Create and push a version tag
git tag v1.0.0
git push origin v1.0.0
```

### Manual Releases

Use GitHub's workflow dispatch feature:

1. Go to **Actions** tab in GitHub repository
2. Select **Release** workflow
3. Click **Run workflow**
4. Enter the tag name and options
5. Click **Run workflow**

### Release Workflow Steps

1. **Create Release**: Generates release notes from commit history
2. **Build Artifacts**: Builds for all platforms (macOS x64/ARM64, Windows x64, Linux x64)
3. **Code Signing**: Signs macOS and Windows binaries
4. **Notarization**: Notarizes macOS binaries through Apple
5. **Upload Assets**: Uploads signed binaries to GitHub release
6. **Updater Manifest**: Creates Tauri updater manifest
7. **Publish**: Publishes the release

### Supported Platforms

| Platform | Architecture | Output Format | Code Signing |
|----------|-------------|---------------|--------------|
| macOS | x86_64 (Intel) | `.dmg` | ✅ Developer ID + Notarization |
| macOS | aarch64 (Apple Silicon) | `.dmg` | ✅ Developer ID + Notarization |
| Windows | x86_64 | `.msi` | ⚠️ Optional |
| Linux | x86_64 | `.AppImage` | ❌ Not required |

## CI/CD Workflows

### CI Workflow (`.github/workflows/ci.yml`)

**Triggers:**
- Push to `main` or `develop` branches
- Pull requests to `main` or `develop` branches

**Jobs:**
1. **Format Check**: Verifies code formatting (Rust + Frontend)
2. **Lint**: Runs linting tools (Clippy + ESLint)
3. **Backend Tests**: Runs Rust tests on multiple platforms
4. **Frontend Tests**: Runs JavaScript/Svelte tests with coverage
5. **Build App**: Cross-platform build verification
6. **Security Audit**: Dependency vulnerability scanning
7. **Dependency Review**: Reviews new dependencies in PRs

### Release Workflow (`.github/workflows/release.yml`)

**Triggers:**
- Push tags matching `v*` pattern
- Manual workflow dispatch

**Jobs:**
1. **Create Release**: Generates release with automated notes
2. **Build and Upload**: Cross-platform builds with code signing
3. **Updater Manifest**: Creates Tauri updater configuration
4. **Post Release**: Updates release with download instructions

### Performance Optimizations

- **Rust Cache**: Shared cache across jobs for faster builds
- **Node Cache**: NPM dependency caching
- **Concurrency**: Cancels in-progress workflows on new pushes
- **Matrix Strategy**: Parallel builds for multiple platforms
- **Artifact Compression**: Reduces storage and transfer time

## Environment Variables

### CI Environment

```bash
# Rust configuration
RUST_BACKTRACE=1
CARGO_TERM_COLOR=always

# Tauri configuration (set automatically by workflows)
APPLE_ID=<from-secrets>
APPLE_PASSWORD=<from-secrets>
APPLE_TEAM_ID=<from-secrets>
TAURI_SIGNING_IDENTITY=<from-secrets>
TAURI_PRIVATE_KEY=<from-secrets>
TAURI_KEY_PASSWORD=<from-secrets>
```

### Local Development

```bash
# Optional: Enable debug mode for Tauri
export TAURI_DEBUG=true

# Optional: Skip code signing for local builds
export TAURI_SKIP_DEVTOOLS_CHECK=true
```

## Troubleshooting

### Common Issues

#### Code Signing Failures

**Problem**: macOS code signing fails with "identity not found"

**Solution**:
1. Verify `APPLE_SIGNING_IDENTITY` matches certificate exactly
2. Check certificate is properly base64 encoded
3. Ensure certificate password is correct
4. Verify certificate hasn't expired

**Check Certificate Identity**:
```bash
# List available signing identities
security find-identity -v -p codesigning
```

#### Notarization Failures

**Problem**: Apple notarization fails or times out

**Solution**:
1. Verify Apple ID and app-specific password
2. Check Team ID is correct
3. Ensure binary is properly signed before notarization
4. Review Apple's notarization requirements

**Check Notarization Status**:
```bash
# Check recent notarization history
xcrun notarytool history --apple-id <your-apple-id> --password <app-password> --team-id <team-id>
```

#### Build Failures

**Problem**: Rust compilation fails on specific platforms

**Solution**:
1. Check platform-specific dependencies are installed
2. Verify Rust toolchain supports target architecture
3. Review error logs for missing system libraries
4. Ensure cross-compilation is properly configured

#### Cache Issues

**Problem**: Builds are slow or use outdated dependencies

**Solution**:
1. Clear GitHub Actions cache if needed
2. Update cache keys when dependencies change significantly
3. Verify cache restore is working correctly

**Clear GitHub Cache**:
1. Go to repository **Settings > Actions > Caches**
2. Delete problematic cache entries
3. Re-run workflows to regenerate cache

### Debug Information

**Enable Verbose Logging**:

Add to workflow for debugging:

```yaml
env:
  RUST_LOG: debug
  TAURI_DEBUG: true
```

**Local Testing**:

```bash
# Test Tauri build locally
npm run tauri:build

# Test with specific target
npm run tauri:build -- --target aarch64-apple-darwin

# Check Rust formatting
cd src-tauri && cargo fmt --all -- --check

# Run security audit
cd src-tauri && cargo audit
```

## Security Best Practices

1. **Secrets Management**:
   - Never commit certificates or passwords to repository
   - Use GitHub Secrets for all sensitive information
   - Rotate certificates and passwords regularly

2. **Code Signing**:
   - Always sign releases for distribution
   - Verify signatures after build
   - Use timestamp servers for long-term validity

3. **Dependency Management**:
   - Enable Dependabot for automated updates
   - Review security advisories regularly
   - Use dependency review for PRs

4. **Access Control**:
   - Limit who can create releases
   - Require PR reviews for workflow changes
   - Use branch protection rules

## Monitoring and Maintenance

### Regular Tasks

- **Weekly**: Review Dependabot PRs and security advisories
- **Monthly**: Update toolchain versions (Node.js, Rust)
- **Quarterly**: Rotate Apple app-specific passwords
- **Annually**: Renew code signing certificates

### Metrics to Monitor

- **Build Success Rate**: CI/CD workflow success percentage
- **Build Time**: Average time for full workflow completion
- **Security Alerts**: Number of unresolved security advisories
- **Release Frequency**: Number of releases per month

---

For additional help, consult:
- [Tauri Documentation](https://tauri.app/v1/guides/distribution/)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Apple Notarization Guide](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)