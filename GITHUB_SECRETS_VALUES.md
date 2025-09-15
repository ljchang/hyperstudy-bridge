# GitHub Secrets Configuration for HyperStudy Bridge

## Your Certificate Information
Based on your keychain, here are the exact values to use:

### Required Secrets for Code Signing

1. **APPLE_CERTIFICATE**
   - Value: [Base64-encoded .p12 file - see conversion steps below]

2. **APPLE_CERTIFICATE_PASSWORD**
   - Value: [Password you set when exporting the .p12 file]

3. **APPLE_SIGNING_IDENTITY**
   - Value: `Developer ID Application: Luke Chang (S368GH6KF7)`

4. **APPLE_TEAM_ID**
   - Value: `S368GH6KF7`

### Additional Secrets for Notarization

5. **APPLE_ID**
   - Value: [Your Apple ID email address]

6. **APPLE_ID_PASSWORD**
   - Value: [App-specific password - generate at https://appleid.apple.com]

## Quick Setup Commands

```bash
# 1. Export certificate (choose one method):

# Method A: Interactive (will prompt for password)
security export -k ~/Library/Keychains/login.keychain-db \
  -t identities -f pkcs12 -o ~/Desktop/DeveloperID.p12 -P

# Method B: With password in command (less secure, but scriptable)
security export -k ~/Library/Keychains/login.keychain-db \
  -t identities -f pkcs12 -o ~/Desktop/DeveloperID.p12 \
  -P "your-chosen-password"

# 2. Convert to base64 and copy to clipboard
base64 -i ~/Desktop/DeveloperID.p12 | pbcopy

# 3. Clean up (after adding to GitHub)
rm -P ~/Desktop/DeveloperID.p12
```

## Setting Up in GitHub

1. Go to: https://github.com/ljchang/hyperstudy-bridge/settings/secrets/actions
2. Click "New repository secret"
3. Add each secret with the exact names and values above
4. Save each secret

## Generating App-Specific Password

1. Sign in to https://appleid.apple.com
2. Go to "Sign-In and Security"
3. Select "App-Specific Passwords"
4. Click the "+" button
5. Name it "HyperStudy Bridge Notarization"
6. Copy the generated password
7. Use this as the value for `APPLE_ID_PASSWORD`

## Testing Your Setup

After adding all secrets, test by:

1. Push to a branch
2. Check GitHub Actions
3. Look for successful signing in the build logs

## Security Reminders

⚠️ **Important**:
- Delete the .p12 file after uploading to GitHub
- Never commit these values to the repository
- Use a strong password for the .p12 export
- Keep the app-specific password secure

---
*Generated for Luke Chang (S368GH6KF7)*
*Last Updated: 2025-09-15*