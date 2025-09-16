#!/bin/bash

# Create a new release for HyperStudy Bridge
# Usage: ./scripts/create-release.sh [version]

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

# Get version from argument or prompt
if [ -n "$1" ]; then
    VERSION="$1"
else
    # Get current version from Cargo.toml
    CURRENT_VERSION=$(grep "^version" src-tauri/Cargo.toml | head -1 | cut -d'"' -f2)
    print_info "Current version: v$CURRENT_VERSION"

    # Suggest next version
    IFS='.' read -ra PARTS <<< "$CURRENT_VERSION"
    MAJOR="${PARTS[0]}"
    MINOR="${PARTS[1]}"
    PATCH="${PARTS[2]}"

    NEXT_PATCH="$MAJOR.$MINOR.$((PATCH + 1))"
    NEXT_MINOR="$MAJOR.$((MINOR + 1)).0"
    NEXT_MAJOR="$((MAJOR + 1)).0.0"

    echo ""
    echo "Suggested versions:"
    echo "  1) Patch release (v$NEXT_PATCH) - Bug fixes"
    echo "  2) Minor release (v$NEXT_MINOR) - New features"
    echo "  3) Major release (v$NEXT_MAJOR) - Breaking changes"
    echo "  4) Custom version"
    echo ""

    read -p "Select version type (1-4): " choice

    case $choice in
        1) VERSION="v$NEXT_PATCH" ;;
        2) VERSION="v$NEXT_MINOR" ;;
        3) VERSION="v$NEXT_MAJOR" ;;
        4)
            read -p "Enter version (e.g., v1.0.0): " VERSION
            ;;
        *)
            print_error "Invalid choice"
            exit 1
            ;;
    esac
fi

# Ensure version starts with 'v'
if [[ ! "$VERSION" =~ ^v ]]; then
    VERSION="v$VERSION"
fi

# Validate version format
if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+)?$ ]]; then
    print_error "Invalid version format. Use vX.Y.Z or vX.Y.Z-suffix"
    exit 1
fi

print_info "Creating release $VERSION"

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    print_warning "You have uncommitted changes"
    read -p "Do you want to commit them first? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git add -A
        git commit -m "chore: Prepare release $VERSION"
    else
        print_error "Please commit or stash your changes first"
        exit 1
    fi
fi

# Update version in files
VERSION_NUM="${VERSION#v}"  # Remove 'v' prefix

print_info "Updating version in Cargo.toml..."
# Update src-tauri/Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$VERSION_NUM\"/" src-tauri/Cargo.toml

print_info "Updating version in tauri.conf.json..."
# Update src-tauri/tauri.conf.json
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION_NUM\"/" src-tauri/tauri.conf.json

print_info "Updating version in package.json..."
# Update package.json
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION_NUM\"/" package.json

# Commit version updates
git add -A
git commit -m "chore: Bump version to $VERSION" || true

# Create and push tag
print_info "Creating tag $VERSION..."
git tag -a "$VERSION" -m "Release $VERSION"

# Push changes and tag
print_info "Pushing to GitHub..."
git push origin main
git push origin "$VERSION"

print_status "Release $VERSION created successfully!"
echo ""
print_info "GitHub Actions will now:"
echo "  1. Build and sign the macOS binaries"
echo "  2. Notarize them with Apple"
echo "  3. Create a GitHub release with the artifacts"
echo ""
print_info "Monitor the progress at:"
echo "  https://github.com/ljchang/hyperstudy-bridge/actions"
echo ""
print_info "The release will be available at:"
echo "  https://github.com/ljchang/hyperstudy-bridge/releases/tag/$VERSION"