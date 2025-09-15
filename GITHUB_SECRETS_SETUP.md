# GitHub Secrets Setup for HyperStudy Bridge

This document provides instructions for setting up GitHub Secrets required for CI/CD, particularly for macOS code signing and deployment.

## Required Secrets

### 1. macOS Code Signing (Required for Production Releases)

#### `APPLE_CERTIFICATE`
- **Description**: Base64-encoded Apple Developer certificate (.p12 file)
- **How to obtain**:
  1. Export your Developer ID Application certificate from Keychain Access
  2. Convert to base64: `base64 -i certificate.p12 -o certificate.txt`
  3. Copy the contents of certificate.txt

#### `APPLE_CERTIFICATE_PASSWORD`
- **Description**: Password for the .p12 certificate file
- **How to obtain**: The password you set when exporting the certificate

#### `APPLE_SIGNING_IDENTITY`
- **Description**: Your Apple Developer ID signing identity
- **Format**: `Developer ID Application: Your Name (TEAMID)`
- **How to obtain**: Run `security find-identity -v -p codesigning` in Terminal

#### `APPLE_ID`
- **Description**: Your Apple ID email used for notarization
- **Format**: `your.email@example.com`

#### `APPLE_ID_PASSWORD`
- **Description**: App-specific password for notarization
- **How to obtain**:
  1. Go to https://appleid.apple.com
  2. Sign in and navigate to Security
  3. Generate an app-specific password
  4. Save this password securely

#### `APPLE_TEAM_ID`
- **Description**: Your Apple Developer Team ID
- **How to obtain**: Found in your Apple Developer account

### 2. Cross-Platform Build Secrets

#### `TAURI_SIGNING_PRIVATE_KEY`
- **Description**: Private key for Tauri updater signatures
- **How to generate**:
  ```bash
  npm run tauri signer generate -- -w ~/.tauri/myapp.key
  ```
- **Note**: Save the public key in your repository

#### `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- **Description**: Password for the Tauri signing key
- **How to obtain**: The password you set when generating the key

### 3. Testing & Deployment

#### `CODECOV_TOKEN`
- **Description**: Token for uploading coverage reports to Codecov
- **How to obtain**:
  1. Sign up at https://codecov.io
  2. Add your repository
  3. Copy the upload token

## Setting Up Secrets in GitHub

1. Navigate to your repository on GitHub
2. Go to **Settings** → **Secrets and variables** → **Actions**
3. Click **New repository secret**
4. Add each secret with its name and value
5. Click **Add secret**

## Local Development Setup

For local development and testing, create a `.env` file in the project root:

```bash
# .env (DO NOT COMMIT THIS FILE)
APPLE_CERTIFICATE=<base64-encoded-certificate>
APPLE_CERTIFICATE_PASSWORD=<certificate-password>
APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
APPLE_ID=your.email@example.com
APPLE_ID_PASSWORD=<app-specific-password>
APPLE_TEAM_ID=<team-id>
TAURI_SIGNING_PRIVATE_KEY=<private-key>
TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<key-password>
```

## Testing Your Setup

### Local Testing
```bash
# Test code signing locally (macOS only)
npm run tauri:build -- --target universal-apple-darwin

# Verify signature
codesign -dv --verbose=4 src-tauri/target/release/bundle/macos/HyperStudy\ Bridge.app
```

### CI/CD Testing
1. Push to a feature branch
2. Check the Actions tab in GitHub
3. Verify that the build workflow runs successfully
4. For release builds, check that signing and notarization complete

## Security Best Practices

1. **Never commit secrets** to the repository
2. **Rotate secrets regularly** (every 90 days recommended)
3. **Use different secrets** for development and production
4. **Limit secret access** to only necessary workflows
5. **Monitor secret usage** in GitHub's security tab

## Troubleshooting

### Common Issues

#### "Certificate not found" error
- Ensure the certificate is properly base64-encoded
- Verify the certificate hasn't expired
- Check that the password is correct

#### "Notarization failed" error
- Verify Apple ID and app-specific password
- Ensure you're using an app-specific password, not your regular Apple ID password
- Check that your Developer ID certificate is valid for notarization

#### "Team ID not found" error
- Verify the Team ID matches your Apple Developer account
- Ensure your account has the necessary permissions

### Getting Help

If you encounter issues:
1. Check the GitHub Actions logs for detailed error messages
2. Verify all secrets are properly set in GitHub
3. Test the signing process locally first
4. Consult the [Tauri documentation](https://tauri.app/v1/guides/distribution/sign-macos/)

## Required Permissions

Ensure your Apple Developer account has:
- Valid Developer ID Application certificate
- Notarization permissions
- Active membership status

## Updates and Maintenance

- Review and update secrets when certificates expire
- Update this documentation when the CI/CD process changes
- Keep track of certificate expiration dates
- Maintain a secure backup of all certificates and keys

---

*Last Updated: 2025-09-15*
*For questions or issues, please open a GitHub issue.*