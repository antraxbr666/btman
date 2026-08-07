# Skill: auto-commit

Auto-commits and pushes changes after source modifications in overskride.

## Usage
Trigger AFTER file edits to automatically commit and push.

## Implementation
```bash
/home/antrax/Dev/overskride/.opencode/skill/auto-commit/auto-commit.sh
```

## Features
- Only commits if there are actual changes
- **Updates README.md version badge** before incrementing version
- **Generates descriptive commit messages** based on actual diff content (first meaningful change line)
- Uses emojis based on file type (🦀 Rust, 📝 Markdown, 🎨 Blueprint, 🔧 Meson, 📦 Cargo, ⚙️ Scripts)
- **Increments patch version in Cargo.toml before commit**
- Pushes to current branch on origin

## Example Commit Messages
```
🦀 src/bluetooth/device.rs: replaced DEVICES_LUT with devices_lut() accessor... (v0.6.6)
📝 README.md: updated version badge to 0.6.7... (v0.6.7)
🎨 src/gtk/window.blp: removed Keyboard Shortcuts menu item... (v0.6.8)
🔧 meson.build: removed help-overlay.blp from blueprints... (v0.6.9)
📦 Cargo.toml: version bump, added new dependency... (v0.6.10)
```

## Flow
1. Detect changes
2. Calculate new patch version
3. Update README.md version badge
4. Update Cargo.toml version
5. Generate descriptive commit message
6. Commit and push