---
name: release
description: Publish a new version of flyway-rs to crates.io. Handles version bumping, documentation sync, git commit/push, mirror config, and crate publishing in dependency order. Use when the user asks to release, publish, or bump version.
---

# flyway-rs Release

## Version Files to Update

All version numbers are centralized in the workspace `Cargo.toml`:

```
Cargo.toml:
  - workspace.package.version
  - workspace.dependencies: flyway-sql-changelog, flyway-codegen, flyway-rbatis, flyway
```

## Documentation Version Sync

Update the dependency version in these 3 files:
- `README.md` (root)
- `README_CN.md` (root)
- `flyway/README.md` (crates.io readme)

Search for `flyway = "` and `flyway-rbatis = "` in each file and replace with the new version.

## Release Workflow

### 1. Bump Version

Update `Cargo.toml` workspace version and all 4 workspace dependency versions.

### 2. Sync Documentation

Update version numbers in all 3 README files.

### 3. Git Commit & Push

```
git add -A
git commit -m "chore: bump version to <new_version>"
git push
```

### 4. Disable Mirror (Critical)

In `.cargo/config.toml`, comment out the `replace-with` line:

```toml
# replace-with = 'ustc'
```

**Why**: ustc mirror has sync delay. Publishing dependent crates will fail if the mirror hasn't synced yet.

### 5. Publish Crates (Dependency Order)

Publish in this exact order (each depends on the previous):

```
1. cargo publish -p flyway-sql-changelog --registry crates-io
2. cargo publish -p flyway-codegen --registry crates-io
3. cargo publish -p flyway --registry crates-io
4. cargo publish -p flyway-rbatis --registry crates-io
```

Wait for each crate to be available before publishing the next.

### 6. Restore Mirror

Uncomment `replace-with = 'ustc'` in `.cargo/config.toml`.

## Checklist

- [ ] Version bumped in Cargo.toml (workspace.package.version + 4 deps)
- [ ] 3 README files updated with new version
- [ ] Git committed and pushed
- [ ] Mirror disabled in .cargo/config.toml
- [ ] 4 crates published in order
- [ ] Mirror restored in .cargo/config.toml
