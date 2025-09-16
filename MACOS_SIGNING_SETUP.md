# macOS Code Signing and Notarization Setup Guide

This guide provides complete instructions for setting up macOS code signing and notarization for HyperStudy Bridge.

## Prerequisites

1. **Apple Developer Account** ($99/year)
   - Sign up at [developer.apple.com](https://developer.apple.com)
   - Enroll in the Apple Developer Program

2. **macOS Development Machine**
   - Xcode installed (for command-line tools)
   - macOS 10.15 (Catalina) or later

## Step 1: Create Developer ID Certificate

### Via Apple Developer Portal

1. Go to [Certificates, Identifiers & Profiles](https://developer.apple.com/account/resources/certificates/list)
2. Click the **+** button to create a new certificate
3. Select **Developer ID Application**
4. Follow the Certificate Signing Request (CSR) instructions

### Generate CSR on macOS

1. Open **Keychain Access** app
2. Menu: **Keychain Access → Certificate Assistant → Request a Certificate from a Certificate Authority**
3. Fill in:
   - **User Email Address**: Your Apple ID email
   - **Common Name**: Your name or company name
   - **CA Email Address**: Leave blank
   - Select **"Saved to disk"**
4. Save the CSR file

### Complete Certificate Creation

1. Upload the CSR file to Apple Developer Portal
2. Download the certificate (.cer file)
3. Double-click to install in Keychain Access

## Step 2: Export Certificate for GitHub Actions

### Export from Keychain Access

1. Open **Keychain Access**
2. Find your **Developer ID Application** certificate
3. Right-click → **Export "Developer ID Application: Your Name"**
4. Choose format: **Personal Information Exchange (.p12)**
5. Set a strong password (save this for later)
6. Save as `certificate.p12`

### Convert to Base64

```bash
# Convert certificate to base64
base64 -i certificate.p12 -o certificate_base64.txt

# Copy to clipboard (macOS)
base64 -i certificate.p12 | pbcopy

# Verify the output (should be a long string)
cat certificate_base64.txt | head -c 100
```

## Step 3: Create App-Specific Password

1. Go to [appleid.apple.com](https://appleid.apple.com)
2. Sign in with your Apple ID
3. Go to **Security** section
4. Under **App-Specific Passwords**, click **Generate Password**
5. Name it "HyperStudy Bridge Notarization"
6. Save the password (format: `xxxx-xxxx-xxxx-xxxx`)

## Step 4: Find Your Team ID

### Method 1: Apple Developer Portal
1. Go to [developer.apple.com](https://developer.apple.com/account)
2. Your Team ID is shown in the top-right corner

### Method 2: From Certificate
```bash
# List certificates and find Team ID
security find-identity -v -p codesigning | grep "Developer ID Application"
# Output: 1) HASH "Developer ID Application: Your Name (TEAM_ID)"
```

### Method 3: Using Xcode
1. Open Xcode
2. Preferences → Accounts
3. Select your Apple ID
4. Team ID is shown in the team list

## Step 5: Configure GitHub Secrets

Go to your repository's **Settings → Secrets and variables → Actions** and add:

| Secret Name | Value | Notes |
|-------------|-------|-------|
| `APPLE_CERTIFICATE` | Base64 encoded .p12 file | From Step 2 |
| `APPLE_CERTIFICATE_PASSWORD` | Password for .p12 file | From Step 2 |
| `APPLE_ID` | Your Apple ID email | developer@company.com |
| `APPLE_PASSWORD` | App-specific password | From Step 3 |
| `APPLE_TEAM_ID` | Your Team ID | From Step 4 |
| `APPLE_SIGNING_IDENTITY` | Full certificate name | See below |

### Finding Signing Identity

```bash
# Find exact signing identity name
security find-identity -v -p codesigning | grep "Developer ID Application"

# Example output:
# 1) ABC123... "Developer ID Application: John Doe (TEAMID123)"

# Use the full name in quotes as APPLE_SIGNING_IDENTITY:
# "Developer ID Application: John Doe (TEAMID123)"
```

## Step 6: Local Testing

### Test Certificate Setup

```bash
# Verify certificate is installed
security find-identity -v -p codesigning

# Test signing a file
echo "test" > test.txt
codesign -s "Developer ID Application: Your Name" test.txt
codesign -v test.txt
rm test.txt
```

### Build and Sign Locally

```bash
# Basic build with signing
./scripts/build-and-sign-mac.sh

# Build with notarization (requires env vars)
export APPLE_ID="your-apple-id@example.com"
export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"
export APPLE_TEAM_ID="TEAMID123"
./scripts/build-and-sign-mac.sh --notarize

# Build universal binary
./scripts/build-and-sign-mac.sh --universal
```

### Verify Signed App

```bash
# Check code signature
codesign -dv --verbose=4 "path/to/HyperStudy Bridge.app"

# Verify with spctl (Gatekeeper)
spctl -a -vv "path/to/HyperStudy Bridge.app"

# Check notarization status
xcrun stapler validate "path/to/HyperStudy-Bridge.dmg"
```

## Step 7: CI/CD Workflow

The GitHub Actions workflow will automatically:

1. Import certificates from secrets
2. Build the application
3. Sign with Developer ID
4. Create DMG
5. Submit for notarization
6. Wait for Apple's approval
7. Staple the ticket to DMG
8. Upload to release

### Trigger a Release

```bash
# Create and push a tag
git tag v1.0.0
git push origin v1.0.0

# Or use GitHub UI to create a release
```

## Troubleshooting

### Common Issues

#### "Certificate Not Found"
```bash
# List all certificates
security find-identity -v

# If missing, re-import:
security import certificate.p12 -P "password"
```

#### "Notarization Failed"
```bash
# Check notarization history
xcrun notarytool history \
  --apple-id "your-id" \
  --password "app-password" \
  --team-id "TEAM_ID"

# Get detailed log
xcrun notarytool log [submission-id] \
  --apple-id "your-id" \
  --password "app-password" \
  --team-id "TEAM_ID"
```

#### "The identity cannot be used for signing"
- Ensure you're using Developer ID Application, not Developer ID Installer
- Check certificate hasn't expired
- Verify private key is present in Keychain

### Validation Commands

```bash
# Validate DMG structure
hdiutil verify "HyperStudy-Bridge.dmg"

# Check if app will open without warnings
spctl --assess --type open --context context:primary-signature --verbose "HyperStudy-Bridge.dmg"

# Detailed signature check
codesign -dvvv "HyperStudy Bridge.app"

# Check entitlements
codesign -d --entitlements - "HyperStudy Bridge.app"
```

## Security Best Practices

1. **Never commit certificates or passwords to Git**
2. **Rotate app-specific passwords regularly**
3. **Use different passwords for different services**
4. **Enable 2FA on your Apple ID**
5. **Restrict GitHub secret access to necessary workflows only**
6. **Monitor certificate expiration dates** (they last 5 years)

## Certificate Renewal

Certificates expire after 5 years. To renew:

1. Create new certificate in Apple Developer Portal
2. Export and update GitHub secrets
3. Keep old certificate until all signed apps are re-released
4. Update local development environment

## Quick Reference

### Environment Variables for Scripts

```bash
# Required for notarization
export APPLE_ID="your-apple-id@example.com"
export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"  # App-specific password
export APPLE_TEAM_ID="TEAMID123"
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID123)"

# Optional
export KEYCHAIN_PASSWORD="keychain-password"  # Defaults to 'actions' in CI
```

### Useful Commands

```bash
# Find signing identities
security find-identity -v -p codesigning

# Manual notarization
xcrun notarytool submit "app.dmg" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait

# Staple notarization ticket
xcrun stapler staple "app.dmg"

# Verify notarization
xcrun stapler validate "app.dmg"

# Check Gatekeeper
spctl -a -t open --context context:primary-signature -v "app.dmg"
```

## Additional Resources

- [Apple's Notarization Documentation](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [Tauri Code Signing Guide](https://tauri.app/v1/guides/distribution/sign-macos)
- [GitHub Actions Secrets](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
- [Notarytool Documentation](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution/customizing_the_notarization_workflow)

## Support

If you encounter issues:

1. Check the [Troubleshooting Guide](TROUBLESHOOTING_GUIDE.md)
2. Review GitHub Actions logs for detailed error messages
3. Contact Apple Developer Support for certificate issues
4. Open an issue in the repository for build problems