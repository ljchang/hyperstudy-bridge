#!/bin/bash

# Local macOS Build and Sign Script for HyperStudy Bridge
# This script builds, signs, and optionally notarizes the app locally

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() { echo -e "${GREEN}[✓]${NC} $1"; }
print_error() { echo -e "${RED}[✗]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[!]${NC} $1"; }
print_info() { echo -e "${BLUE}[i]${NC} $1"; }

# Configuration
TARGET="aarch64-apple-darwin" # Default to Apple Silicon
NOTARIZE=false
UNIVERSAL=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --intel)
            TARGET="x86_64-apple-darwin"
            shift
            ;;
        --arm|--silicon)
            TARGET="aarch64-apple-darwin"
            shift
            ;;
        --universal)
            UNIVERSAL=true
            shift
            ;;
        --notarize)
            NOTARIZE=true
            shift
            ;;
        --help)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  --intel          Build for Intel Macs"
            echo "  --arm, --silicon Build for Apple Silicon (default)"
            echo "  --universal      Build universal binary (both architectures)"
            echo "  --notarize       Submit for Apple notarization"
            echo "  --help          Show this help message"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Check for required tools
check_requirements() {
    print_info "Checking requirements..."

    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        print_error "Rust is not installed. Please install from https://rustup.rs"
        exit 1
    fi

    # Check for Node.js
    if ! command -v node &> /dev/null; then
        print_error "Node.js is not installed. Please install Node.js v18+"
        exit 1
    fi

    # Check for codesign
    if ! command -v codesign &> /dev/null; then
        print_error "codesign not found. Please install Xcode Command Line Tools"
        exit 1
    fi

    # Check for signing identity
    if ! security find-identity -v -p codesigning | grep -q "Developer ID Application"; then
        print_warning "No Developer ID Application certificate found"
        print_info "You can still build, but the app won't be signed for distribution"
        read -p "Continue without signing? (y/n): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
        SKIP_SIGNING=true
    else
        # Get signing identity
        SIGNING_IDENTITY=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -1 | awk -F'"' '{print $2}')
        print_info "Using signing identity: $SIGNING_IDENTITY"
    fi

    print_status "Requirements check complete"
}

# Install dependencies
install_deps() {
    print_info "Installing dependencies..."

    # Install npm dependencies
    if [ ! -d "node_modules" ]; then
        npm ci
    fi

    # Add rust target if needed
    rustup target add $TARGET

    print_status "Dependencies installed"
}

# Build the application
build_app() {
    print_info "Building for $TARGET..."

    # Clean previous builds
    rm -rf src-tauri/target/$TARGET/release/bundle

    # Build with Tauri
    if [ "$SKIP_SIGNING" = true ]; then
        npm run tauri:build -- --target $TARGET
    else
        TAURI_SIGNING_IDENTITY="$SIGNING_IDENTITY" npm run tauri:build -- --target $TARGET --config src-tauri/tauri.macos.conf.json
    fi

    print_status "Build complete"
}

# Build universal binary
build_universal() {
    print_info "Building universal binary..."

    # Build for both architectures
    print_info "Building for Intel..."
    TARGET="x86_64-apple-darwin" build_app

    print_info "Building for Apple Silicon..."
    TARGET="aarch64-apple-darwin" build_app

    # Find the app bundles
    INTEL_APP=$(find src-tauri/target/x86_64-apple-darwin -name "*.app" -type d | grep -v bundle/macos | head -1)
    ARM_APP=$(find src-tauri/target/aarch64-apple-darwin -name "*.app" -type d | grep -v bundle/macos | head -1)

    if [ -z "$INTEL_APP" ] || [ -z "$ARM_APP" ]; then
        print_error "Could not find both app bundles for universal binary"
        exit 1
    fi

    # Create universal app
    UNIVERSAL_APP="src-tauri/target/universal/HyperStudy Bridge.app"
    mkdir -p "$(dirname "$UNIVERSAL_APP")"
    cp -R "$ARM_APP" "$UNIVERSAL_APP"

    # Merge the binaries
    BINARY_NAME="HyperStudy Bridge"
    lipo -create \
        "$INTEL_APP/Contents/MacOS/$BINARY_NAME" \
        "$ARM_APP/Contents/MacOS/$BINARY_NAME" \
        -output "$UNIVERSAL_APP/Contents/MacOS/$BINARY_NAME"

    # Re-sign if needed
    if [ "$SKIP_SIGNING" != true ]; then
        codesign --force --deep --sign "$SIGNING_IDENTITY" \
            --entitlements src-tauri/entitlements.plist \
            "$UNIVERSAL_APP"
    fi

    print_status "Universal binary created"
}

# Sign the application
sign_app() {
    if [ "$SKIP_SIGNING" = true ]; then
        print_warning "Skipping code signing"
        return
    fi

    print_info "Signing application..."

    # Find the app bundle
    if [ "$UNIVERSAL" = true ]; then
        APP_PATH="src-tauri/target/universal/HyperStudy Bridge.app"
    else
        APP_PATH=$(find src-tauri/target/$TARGET -name "*.app" -type d | grep -v bundle/macos | head -1)
    fi

    if [ -z "$APP_PATH" ]; then
        print_error "Could not find app bundle to sign"
        exit 1
    fi

    # Sign with hardened runtime
    codesign --force --deep --timestamp --options runtime \
        --sign "$SIGNING_IDENTITY" \
        --entitlements src-tauri/entitlements.plist \
        "$APP_PATH"

    # Verify signature
    if codesign --verify --deep --strict "$APP_PATH"; then
        print_status "Application signed successfully"
    else
        print_error "Signature verification failed"
        exit 1
    fi
}

# Create DMG
create_dmg() {
    print_info "Creating DMG..."

    # Find the app bundle
    if [ "$UNIVERSAL" = true ]; then
        APP_PATH="src-tauri/target/universal/HyperStudy Bridge.app"
        DMG_NAME="HyperStudy-Bridge-universal.dmg"
    else
        APP_PATH=$(find src-tauri/target/$TARGET -name "*.app" -type d | grep -v bundle/macos | head -1)
        DMG_NAME="HyperStudy-Bridge-$TARGET.dmg"
    fi

    # Create DMG
    hdiutil create -volname "HyperStudy Bridge" \
        -srcfolder "$APP_PATH" \
        -ov -format UDZO \
        "$DMG_NAME"

    # Sign DMG if not skipping
    if [ "$SKIP_SIGNING" != true ]; then
        codesign --force --sign "$SIGNING_IDENTITY" "$DMG_NAME"
    fi

    print_status "DMG created: $DMG_NAME"
}

# Notarize the app
notarize_app() {
    if [ "$NOTARIZE" != true ]; then
        return
    fi

    if [ "$SKIP_SIGNING" = true ]; then
        print_error "Cannot notarize unsigned app"
        exit 1
    fi

    print_info "Starting notarization..."

    # Check for required environment variables
    if [ -z "$APPLE_ID" ] || [ -z "$APPLE_PASSWORD" ] || [ -z "$APPLE_TEAM_ID" ]; then
        print_error "Missing required environment variables for notarization:"
        [ -z "$APPLE_ID" ] && echo "  - APPLE_ID"
        [ -z "$APPLE_PASSWORD" ] && echo "  - APPLE_PASSWORD"
        [ -z "$APPLE_TEAM_ID" ] && echo "  - APPLE_TEAM_ID"
        exit 1
    fi

    # Find DMG
    if [ "$UNIVERSAL" = true ]; then
        DMG_NAME="HyperStudy-Bridge-universal.dmg"
    else
        DMG_NAME="HyperStudy-Bridge-$TARGET.dmg"
    fi

    # Run notarization script
    ./scripts/notarize.sh --dmg "$DMG_NAME" --skip-signing

    print_status "Notarization complete"
}

# Main execution
main() {
    echo "======================================"
    echo "  HyperStudy Bridge Build & Sign"
    echo "======================================"
    echo

    check_requirements
    install_deps

    if [ "$UNIVERSAL" = true ]; then
        build_universal
    else
        build_app
    fi

    sign_app
    create_dmg
    notarize_app

    echo
    echo "======================================"
    echo -e "${GREEN}  Build Complete!${NC}"
    echo "======================================"

    # Show final output
    if [ "$UNIVERSAL" = true ]; then
        DMG_NAME="HyperStudy-Bridge-universal.dmg"
    else
        DMG_NAME="HyperStudy-Bridge-$TARGET.dmg"
    fi

    print_status "Output: $DMG_NAME"

    # Show next steps
    echo
    print_info "Next steps:"
    echo "  1. Test the DMG by installing on a clean system"
    echo "  2. Verify Gatekeeper acceptance: spctl -a -t open --context context:primary-signature -v '$DMG_NAME'"
    if [ "$NOTARIZE" = true ]; then
        echo "  3. Check notarization: xcrun stapler validate '$DMG_NAME'"
    elif [ "$SKIP_SIGNING" != true ]; then
        echo "  3. To notarize: APPLE_ID=... APPLE_PASSWORD=... APPLE_TEAM_ID=... $0 --notarize"
    fi
}

# Run main
main