---
description: bump the version, generate changelog from commits, and trigger a new release with annotated tag
---

# Release Workflow for Keson GUI

This workflow bumps the version across all config files, generates a changelog from commits since the last tag, creates an annotated git tag with the changelog, and triggers the GitHub Actions release.

## Prerequisites

- Ensure all changes are committed and pushed to main
- Ensure all tests pass locally

## Steps

### 1. Determine Version Number

If the user didn't provide a version number:

- Check the current version: `grep '"version"' src-tauri/tauri.conf.json`
- Ask the user for the new version (e.g., 0.9.15)

### 2. Update Version in All Config Files

Update the version field in these three files (they must all match):

- `package.json` (line 3)
- `src-tauri/Cargo.toml` (line 3)
- `src-tauri/tauri.conf.json` (line 31)

### 3. Generate Changelog from Commits

// turbo
Run this command to get all commits since the last tag:

```bash
git log $(git describe --tags --abbrev=0)..HEAD --pretty=format:"- %s" --no-merges
```

### 4. Categorize Changes

Organize the commit messages into categories:

- **🚀 Features**: New functionality (commits with `feat:` prefix or feature-related)
- **🐛 Bug Fixes**: Bug fixes (commits with `fix:` prefix or bugfix-related)
- **🔧 Improvements**: Refactoring, performance, cleanup (commits with `chore:`, `refactor:`, `perf:`)
- **📚 Documentation**: Doc updates (commits with `docs:`)

If commits don't follow conventional commits format, categorize based on content.

### 5. Create Tag Description

Format the changelog as:

```
## What's New in v<VERSION>

### 🚀 Features
- Feature 1 description
- Feature 2 description

### 🐛 Bug Fixes
- Fix 1 description
- Fix 2 description

### 🔧 Improvements
- Improvement 1 description
```

Remove any empty sections. Keep descriptions concise and user-friendly.

### 6. Stage and Commit Version Bump

// turbo

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
```

// turbo

```bash
git commit -m "chore: bump version to <VERSION>"
```

### 7. Create Annotated Tag

Create an annotated tag with the changelog as the message:

```bash
git tag -a v<VERSION> -m "<CHANGELOG_MESSAGE>"
```

**Note**: For multi-line tag messages, create a temporary file with the message and use:

```bash
git tag -a v<VERSION> -F /tmp/tag_message.txt
```

### 8. Push Changes and Tag

// turbo

```bash
git push origin main
```

// turbo

```bash
git push origin v<VERSION>
```

### 9. Verify Release Triggered

Confirm the release was triggered by providing the user with:

- Link to GitHub Actions: `https://github.com/Cartasiane/keson-spectral-improver-gui/actions`
- Note that builds will be created for: macOS (Intel + Apple Silicon), Windows, and Linux

## Quick Reference Commands

// turbo

```bash
# View current version
grep '"version"' src-tauri/tauri.conf.json

# List recent tags
git tag --list --sort=-version:refname | head -5

# View commits since last tag (for changelog)
git log $(git describe --tags --abbrev=0)..HEAD --pretty=format:"- %s" --no-merges

# View diff of files changed since last tag
git diff $(git describe --tags --abbrev=0)..HEAD --stat
```

## Rollback (if needed)

If something goes wrong:

```bash
# Delete local tag
git tag -d v<VERSION>

# Delete remote tag (if pushed)
git push origin :refs/tags/v<VERSION>

# Revert commit
git reset --soft HEAD~1
```
