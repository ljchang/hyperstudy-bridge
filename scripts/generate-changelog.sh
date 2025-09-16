#!/bin/bash

# Generate a formatted changelog from git commits
# Uses conventional commit format to group changes

set -e

# Get version from argument or latest tag
VERSION="${1:-$(git describe --tags --abbrev=0 2>/dev/null || echo "HEAD")}"
PREVIOUS_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")

# Color output
BOLD='\033[1m'
NC='\033[0m'

# Initialize sections
FEATURES=""
FIXES=""
DOCS=""
BREAKING=""
PERFORMANCE=""
REFACTOR=""
TESTS=""
BUILD=""
CHORE=""
OTHER=""

# Function to categorize commits
categorize_commit() {
    local commit="$1"
    local hash="$2"
    local message="${commit#* }" # Remove type prefix

    if [[ "$commit" =~ ^feat(\(.*\))?!?: ]]; then
        FEATURES="${FEATURES}- ${message} (${hash})\n"
    elif [[ "$commit" =~ ^fix(\(.*\))?: ]]; then
        FIXES="${FIXES}- ${message} (${hash})\n"
    elif [[ "$commit" =~ ^docs(\(.*\))?: ]]; then
        DOCS="${DOCS}- ${message} (${hash})\n"
    elif [[ "$commit" =~ ^BREAKING ]] || [[ "$commit" =~ !: ]]; then
        BREAKING="${BREAKING}- ${message} (${hash})\n"
    elif [[ "$commit" =~ ^perf(\(.*\))?: ]]; then
        PERFORMANCE="${PERFORMANCE}- ${message} (${hash})\n"
    elif [[ "$commit" =~ ^refactor(\(.*\))?: ]]; then
        REFACTOR="${REFACTOR}- ${message} (${hash})\n"
    elif [[ "$commit" =~ ^test(\(.*\))?: ]]; then
        TESTS="${TESTS}- ${message} (${hash})\n"
    elif [[ "$commit" =~ ^build(\(.*\))?: ]] || [[ "$commit" =~ ^ci(\(.*\))?: ]]; then
        BUILD="${BUILD}- ${message} (${hash})\n"
    elif [[ "$commit" =~ ^chore(\(.*\))?: ]] || [[ "$commit" =~ ^style(\(.*\))?: ]]; then
        CHORE="${CHORE}- ${message} (${hash})\n"
    else
        OTHER="${OTHER}- ${commit} (${hash})\n"
    fi
}

# Get commits
if [ -z "$PREVIOUS_TAG" ]; then
    COMMITS=$(git log --pretty=format:"%s|%h" --no-merges)
else
    COMMITS=$(git log --pretty=format:"%s|%h" --no-merges ${PREVIOUS_TAG}..HEAD)
fi

# Process each commit
while IFS='|' read -r msg hash; do
    categorize_commit "$msg" "$hash"
done <<< "$COMMITS"

# Generate changelog
echo -e "${BOLD}# Changelog for ${VERSION}${NC}"
echo ""

if [ -n "$PREVIOUS_TAG" ]; then
    echo "**Changes since ${PREVIOUS_TAG}**"
else
    echo "**Initial Release**"
fi
echo ""

# Output sections if they have content
if [ -n "$BREAKING" ]; then
    echo "## ⚠️ BREAKING CHANGES"
    echo -e "$BREAKING"
fi

if [ -n "$FEATURES" ]; then
    echo "## ✨ Features"
    echo -e "$FEATURES"
fi

if [ -n "$FIXES" ]; then
    echo "## 🐛 Bug Fixes"
    echo -e "$FIXES"
fi

if [ -n "$PERFORMANCE" ]; then
    echo "## ⚡ Performance Improvements"
    echo -e "$PERFORMANCE"
fi

if [ -n "$REFACTOR" ]; then
    echo "## ♻️ Code Refactoring"
    echo -e "$REFACTOR"
fi

if [ -n "$DOCS" ]; then
    echo "## 📚 Documentation"
    echo -e "$DOCS"
fi

if [ -n "$TESTS" ]; then
    echo "## 🧪 Tests"
    echo -e "$TESTS"
fi

if [ -n "$BUILD" ]; then
    echo "## 🔧 Build System"
    echo -e "$BUILD"
fi

if [ -n "$CHORE" ]; then
    echo "## 🔨 Maintenance"
    echo -e "$CHORE"
fi

if [ -n "$OTHER" ]; then
    echo "## 📝 Other Changes"
    echo -e "$OTHER"
fi

# Statistics
echo "## 📊 Statistics"
if [ -n "$PREVIOUS_TAG" ]; then
    echo "- **Commits**: $(git rev-list --count ${PREVIOUS_TAG}..HEAD)"
    echo "- **Contributors**: $(git log --format='%an' ${PREVIOUS_TAG}..HEAD | sort -u | wc -l | tr -d ' ')"
    echo "- **Files changed**: $(git diff --stat ${PREVIOUS_TAG}..HEAD | tail -1)"
else
    echo "- **Total commits**: $(git rev-list --count HEAD)"
    echo "- **Contributors**: $(git log --format='%an' | sort -u | wc -l | tr -d ' ')"
fi

echo ""
echo "---"
echo ""

# Full comparison link
if [ -n "$PREVIOUS_TAG" ]; then
    REPO_URL=$(git remote get-url origin | sed 's/\.git$//' | sed 's/git@github.com:/https:\/\/github.com\//')
    echo "**Full Changelog**: ${REPO_URL}/compare/${PREVIOUS_TAG}...${VERSION}"
fi