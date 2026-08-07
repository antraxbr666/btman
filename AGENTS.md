# btman Project Rules

## Auto-Commit

After completing any code changes (file edits, fixes, refactors), ALWAYS run the auto-commit script:

```bash
/home/antrax/.config/opencode/skills/auto-commit/auto-commit.sh
```

This script:
- Updates README.md version badge
- Increments patch version in Cargo.toml
- Generates descriptive commit messages based on the diff
- Commits and pushes to the current branch

Do NOT ask the user for permission to commit. Just run it after finishing the work.

## Build & Run

- Build: `./build.sh`
- Run: `./run.sh`
