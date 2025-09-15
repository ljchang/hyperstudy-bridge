# Converting Developer ID Certificate for GitHub Actions

This guide helps you convert your Developer ID Application `.cer` file to the `.p12` format required for GitHub Actions.

## Prerequisites

- Your Developer ID Application `.cer` file
- Access to macOS Keychain Access
- The private key associated with your certificate (should be in your Keychain)

## Step-by-Step Conversion Process

### Method 1: Using Keychain Access (Recommended)

1. **Import the .cer file to Keychain (if not already there)**
   ```bash
   # Double-click the .cer file, or use command line:
   security import your-certificate.cer -k ~/Library/Keychains/login.keychain
   ```

2. **Open Keychain Access**
   ```bash
   open /Applications/Utilities/Keychain\ Access.app
   ```

3. **Find your Developer ID certificate**
   - In Keychain Access, select "My Certificates" from the left sidebar
   - Look for "Developer ID Application: Your Name (TEAMID)"
   - The certificate should have a disclosure triangle showing the private key

4. **Export as .p12**
   - Right-click on the certificate (not the private key)
   - Select "Export 'Developer ID Application: Your Name'..."
   - Choose "Personal Information Exchange (.p12)" as the format
   - Save it (e.g., as `DeveloperID.p12`)
   - Set a strong password when prompted (you'll need this for GitHub Secrets)

### Method 2: Using Command Line

If your certificate and private key are already in the keychain:

```bash
# List certificates to find the exact name
security find-identity -v -p codesigning

# Export to .p12 (replace "Developer ID Application: Your Name (TEAMID)" with your actual identity)
security export -k ~/Library/Keychains/login.keychain \
  -t identities \
  -f pkcs12 \
  -o ~/Desktop/DeveloperID.p12 \
  -P "your-password-here" \
  "Developer ID Application: Your Name (TEAMID)"
```

## Convert .p12 to Base64 for GitHub Secrets

Once you have the `.p12` file:

```bash
# Convert to base64
base64 -i DeveloperID.p12 -o certificate_base64.txt

# Or copy directly to clipboard (macOS)
base64 -i DeveloperID.p12 | pbcopy
```

## Setting Up GitHub Secrets

1. Go to your repository → Settings → Secrets and variables → Actions

2. Add these secrets:

   - **APPLE_CERTIFICATE**: Paste the base64 content from above
   - **APPLE_CERTIFICATE_PASSWORD**: The password you set when exporting the .p12
   - **APPLE_SIGNING_IDENTITY**: Your identity (e.g., "Developer ID Application: Your Name (TEAMID)")
   - **APPLE_TEAM_ID**: Your Team ID (the part in parentheses)

## Verify Your Certificate

Before uploading to GitHub, verify your certificate works:

```bash
# Check certificate info
openssl pkcs12 -in DeveloperID.p12 -info -nokeys

# Verify it's a Developer ID certificate
security find-identity -v -p codesigning | grep "Developer ID Application"
```

## Finding Your Team ID

If you need to find your Team ID:

```bash
# From your certificate
security find-identity -v -p codesigning | grep "Developer ID Application"
# The Team ID is the alphanumeric code in parentheses

# Or from your .cer file
openssl x509 -in your-certificate.cer -text -noout | grep "OU="
```

## Troubleshooting

### "Certificate has no private key" Error
- The .cer file alone isn't enough; you need the private key
- Check if the private key exists in your Keychain
- If missing, you may need to re-request the certificate from Apple Developer

### "Password incorrect" Error
- Ensure you're using the password set during .p12 export
- Try re-exporting with a simpler password (no special characters initially)

### "Identity not found" Error
- Verify the certificate name matches exactly (including spaces)
- Use `security find-identity -v` to get the exact name

## Security Notes

⚠️ **IMPORTANT**:
- Never commit the .p12 file or base64 content to your repository
- Store the .p12 file securely and delete the base64 text file after adding to GitHub Secrets
- Use strong, unique passwords for the .p12 export
- Consider using separate certificates for development and production

## Clean Up

After setting up GitHub Secrets:

```bash
# Securely delete temporary files
rm -P certificate_base64.txt  # -P overwrites before deleting
rm -P DeveloperID.p12  # Keep a secure backup elsewhere
```

## Next Steps

1. Test the signing process by pushing to a branch
2. Check GitHub Actions logs for any signing errors
3. For notarization, you'll also need:
   - APPLE_ID (your Apple ID email)
   - APPLE_ID_PASSWORD (app-specific password from appleid.apple.com)

---

*For more details, see [GITHUB_SECRETS_SETUP.md](./GITHUB_SECRETS_SETUP.md)*