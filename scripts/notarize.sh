#!/bin/bash

# HyperStudy Bridge macOS Notarization Script
# This script handles code signing and notarization for macOS builds

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
APP_NAME="HyperStudy Bridge"
BUNDLE_ID="com.hyperstudy.bridge"

# Function to print colored output
print_status() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

# Check for required environment variables
check_env_vars() {
    local missing_vars=()

    if [ -z "$APPLE_ID" ]; then
        missing_vars+=("APPLE_ID")
    fi

    if [ -z "$APPLE_PASSWORD" ]; then
        missing_vars+=("APPLE_PASSWORD")
    fi

    if [ -z "$APPLE_TEAM_ID" ]; then
        missing_vars+=("APPLE_TEAM_ID")
    fi

    if [ -z "$APPLE_SIGNING_IDENTITY" ]; then
        # Try to find signing identity automatically
        APPLE_SIGNING_IDENTITY=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -1 | awk -F'"' '{print $2}')
        if [ -z "$APPLE_SIGNING_IDENTITY" ]; then
            missing_vars+=("APPLE_SIGNING_IDENTITY")
        else
            print_warning "Using auto-detected signing identity: $APPLE_SIGNING_IDENTITY"
        fi
    fi

    if [ ${#missing_vars[@]} -gt 0 ]; then
        print_error "Missing required environment variables:"
        for var in "${missing_vars[@]}"; do
            echo "  - $var"
        done
        exit 1
    fi
}

# Find the app bundle
find_app_bundle() {
    local search_path="${1:-src-tauri/target}"

    # Find .app bundle
    APP_PATH=$(find "$search_path" -name "*.app" -type d | grep -v "bundle/macos" | head -1)

    if [ -z "$APP_PATH" ]; then
        print_error "Could not find .app bundle in $search_path"
        exit 1
    fi

    print_status "Found app bundle: $APP_PATH"
}

# Find the DMG file
find_dmg() {
    local search_path="${1:-src-tauri/target}"

    # Find .dmg file
    DMG_PATH=$(find "$search_path" -name "*.dmg" -type f | head -1)

    if [ -z "$DMG_PATH" ]; then
        print_error "Could not find .dmg file in $search_path"
        exit 1
    fi

    print_status "Found DMG: $DMG_PATH"
}

# Code sign the app bundle
sign_app() {
    print_status "Signing app bundle..."

    # Sign all frameworks and libraries first
    find "$APP_PATH" -type f -name "*.dylib" -o -name "*.framework" | while read -r lib; do
        codesign --force --timestamp --options runtime \
            --sign "$APPLE_SIGNING_IDENTITY" \
            --entitlements "src-tauri/entitlements.plist" \
            "$lib" || print_warning "Failed to sign: $lib"
    done

    # Sign the main app bundle
    codesign --force --deep --timestamp --options runtime \
        --sign "$APPLE_SIGNING_IDENTITY" \
        --entitlements "src-tauri/entitlements.plist" \
        "$APP_PATH"

    # Verify the signature
    if codesign --verify --deep --strict "$APP_PATH"; then
        print_status "App bundle signed successfully"
    else
        print_error "Code signing verification failed"
        exit 1
    fi
}

# Sign the DMG
sign_dmg() {
    print_status "Signing DMG..."

    codesign --force --timestamp --sign "$APPLE_SIGNING_IDENTITY" "$DMG_PATH"

    # Verify the signature
    if codesign --verify --strict "$DMG_PATH"; then
        print_status "DMG signed successfully"
    else
        print_error "DMG signing verification failed"
        exit 1
    fi
}

# Notarize the DMG
notarize_dmg() {
    print_status "Starting notarization..."

    # Submit for notarization
    NOTARIZATION_OUTPUT=$(xcrun notarytool submit "$DMG_PATH" \
        --apple-id "$APPLE_ID" \
        --password "$APPLE_PASSWORD" \
        --team-id "$APPLE_TEAM_ID" \
        --wait 2>&1)

    echo "$NOTARIZATION_OUTPUT"

    # Extract submission ID
    SUBMISSION_ID=$(echo "$NOTARIZATION_OUTPUT" | grep -E "id: [a-f0-9-]+" | head -1 | awk '{print $2}')

    if [ -z "$SUBMISSION_ID" ]; then
        print_error "Failed to get submission ID"
        exit 1
    fi

    print_status "Notarization submission ID: $SUBMISSION_ID"

    # Check if notarization was successful
    if echo "$NOTARIZATION_OUTPUT" | grep -q "status: Accepted"; then
        print_status "Notarization successful!"
    else
        print_error "Notarization failed. Fetching log..."

        # Get notarization log
        xcrun notarytool log "$SUBMISSION_ID" \
            --apple-id "$APPLE_ID" \
            --password "$APPLE_PASSWORD" \
            --team-id "$APPLE_TEAM_ID"

        exit 1
    fi
}

# Staple the notarization ticket
staple_dmg() {
    print_status "Stapling notarization ticket..."

    if xcrun stapler staple "$DMG_PATH"; then
        print_status "Notarization ticket stapled successfully"
    else
        print_error "Failed to staple notarization ticket"
        exit 1
    fi

    # Verify the stapling
    if xcrun stapler validate "$DMG_PATH"; then
        print_status "Stapled DMG validated successfully"
    else
        print_error "Stapled DMG validation failed"
        exit 1
    fi
}

# Create a notarized copy
create_notarized_copy() {
    local dmg_dir=$(dirname "$DMG_PATH")
    local dmg_name=$(basename "$DMG_PATH" .dmg)
    local notarized_dmg="${dmg_dir}/${dmg_name}-notarized.dmg"

    cp "$DMG_PATH" "$notarized_dmg"
    print_status "Created notarized copy: $notarized_dmg"
}

# Main execution
main() {
    echo "======================================"
    echo "  HyperStudy Bridge Notarization"
    echo "======================================"
    echo

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dmg)
                DMG_PATH="$2"
                shift 2
                ;;
            --app)
                APP_PATH="$2"
                shift 2
                ;;
            --skip-signing)
                SKIP_SIGNING=true
                shift
                ;;
            --help)
                echo "Usage: $0 [options]"
                echo "Options:"
                echo "  --dmg PATH         Path to DMG file"
                echo "  --app PATH         Path to app bundle"
                echo "  --skip-signing     Skip code signing (only notarize)"
                echo "  --help            Show this help message"
                exit 0
                ;;
            *)
                echo "Unknown option: $1"
                exit 1
                ;;
        esac
    done

    # Check environment variables
    check_env_vars

    # Find files if not specified
    if [ -z "$APP_PATH" ] && [ "$SKIP_SIGNING" != "true" ]; then
        find_app_bundle
    fi

    if [ -z "$DMG_PATH" ]; then
        find_dmg
    fi

    # Sign if not skipped
    if [ "$SKIP_SIGNING" != "true" ]; then
        sign_app
        sign_dmg
    fi

    # Notarize
    notarize_dmg

    # Staple
    staple_dmg

    # Create notarized copy
    create_notarized_copy

    echo
    echo "======================================"
    echo -e "${GREEN}  Notarization Complete!${NC}"
    echo "======================================"
    echo
    print_status "Your notarized DMG is ready for distribution"
}

# Run main function
main "$@"