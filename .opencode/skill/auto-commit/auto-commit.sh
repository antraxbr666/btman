#!/usr/bin/env bash
# Auto-commit for btman project
# Commits and pushes changes AFTER file edits
# Updates README.md version badge, increments patch version in Cargo.toml
# Generates descriptive commit messages based on actual changes

set -e

cd /home/antrax/Dev/overskride

# Check for changes
if git diff --quiet && git diff --cached --quiet; then
    echo "No changes to commit"
    exit 0
fi

# Get current version and calculate new version
CURRENT_VERSION=$(grep '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="${MAJOR}.${MINOR}.${NEW_PATCH}"
echo "📦 Version bumped: ${CURRENT_VERSION} → ${NEW_VERSION}"

# Update README.md version badge with new version
sed -i "s/version-[0-9]\+\.[0-9]\+\.[0-9]\+-blue/version-${NEW_VERSION}-blue/" README.md
echo "📝 Updated README.md version badge to ${NEW_VERSION}"

# Update Cargo.toml with new version
sed -i "s/^version = \"${CURRENT_VERSION}\"/version = \"${NEW_VERSION}\"/" Cargo.toml

git add -A
CHANGED_FILES=$(git diff --cached --name-only)
if [ -z "$CHANGED_FILES" ]; then
    exit 0
fi

# Generate descriptive commit message based on actual changes
COMMIT_MSG=""
while IFS= read -r file; do
    # Get a summary of what changed in this file
    STAT=$(git diff --cached --stat "$file" | tail -1 | sed 's/^[[:space:]]*//')
    SHORT_STAT=$(echo "$STAT" | sed 's/ ([0-9]* insertion.*//' | sed 's/ ([0-9]* deletion.*//')
    
    # Determine emoji based on file type
    case "$file" in
        *.rs) EMOJI="🦀" ;;
        *.md) EMOJI="📝" ;;
        *.blp|*.ui) EMOJI="🎨" ;;
        meson.build) EMOJI="🔧" ;;
        Cargo.toml) EMOJI="📦" ;;
        *.sh) EMOJI="⚙️" ;;
        *.toml) EMOJI="📋" ;;
        *.xml) EMOJI="📄" ;;
        *) EMOJI="✨" ;;
    esac
    
    # Get first meaningful code change (skip diff headers --- a/, +++ b/)
    FIRST_CHANGE=$(git diff --cached "$file" | grep -E '^[+-]' | grep -v '^[-+]{3}' | grep -v '^[-+][[:space:]]*$' | grep -v '^--- ' | grep -v '^+++ ' | head -1 | sed 's/^[+-]//' | sed 's/^[[:space:]]*//' | cut -c1-80)
    
    if [ -n "$FIRST_CHANGE" ]; then
        MSG="$EMOJI ${file}: ${FIRST_CHANGE}..."
    else
        MSG="$EMOJI ${file}: updated"
    fi
    
    if [ -z "$COMMIT_MSG" ]; then
        COMMIT_MSG="$MSG"
    else
        COMMIT_MSG="$COMMIT_MSG; $MSG"
    fi
done <<< "$CHANGED_FILES"

COMMIT_MSG="$COMMIT_MSG (v${NEW_VERSION})"

git commit -m "$COMMIT_MSG"
git push
echo "✅ Committed and pushed: $COMMIT_MSG"