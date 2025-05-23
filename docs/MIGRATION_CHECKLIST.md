# Migration Checklist

This checklist guides you through migrating the Rust implementation to the root directory and preparing for crates.io publication.

## Pre-Migration

- [ ] Ensure all tests pass: `cd rust-implementation && cargo test`
- [ ] Run benchmarks to establish baseline: `cargo bench`
- [ ] Commit all current changes: `git add . && git commit -m "Pre-migration checkpoint"`
- [ ] Create a backup branch: `git checkout -b backup-pre-migration`

## Migration Steps

1. **Update Cargo.toml**
   - [ ] Replace `rust-implementation/Cargo.toml` with `rust-implementation/Cargo_updated.toml`
   - [ ] Update repository URL to your actual GitHub repository
   - [ ] Update authors with actual contributor information
   - [ ] Update homepage and documentation URLs if different

2. **Run Migration Script**
   - [ ] Make script executable: `chmod +x migrate_project.sh`
   - [ ] Run the migration: `./migrate_project.sh`
   - [ ] Verify the script completed successfully

3. **Post-Migration Verification**
   - [ ] Verify directory structure looks correct
   - [ ] Check that `rust-implementation/` is removed or empty
   - [ ] Ensure Go files are in `go_legacy/` directory
   - [ ] Verify new README.md is in place

4. **Update Repository Files**
   - [ ] Update `.github/workflows/` CI/CD files to use new paths
   - [ ] Update any documentation that references old paths
   - [ ] Remove any references to `rust-implementation/` directory
   - [ ] Update `.gitignore` if needed

5. **Test Everything**
   - [ ] Run `cargo build` in root directory
   - [ ] Run `cargo test` to ensure all tests pass
   - [ ] Run `cargo bench` to verify benchmarks work
   - [ ] Run `cargo doc --open` to check documentation
   - [ ] Test all examples: `cargo run --example rule_validator`

## Prepare for crates.io

1. **Final Cargo.toml Updates**
   - [ ] Set version to `0.1.0` (or appropriate initial version)
   - [ ] Ensure all metadata fields are accurate
   - [ ] Add `include` field if you want to limit published files
   - [ ] Review dependencies for minimal versions

2. **Documentation**
   - [ ] Ensure README.md has good examples
   - [ ] Add CHANGELOG.md with initial version
   - [ ] Add CONTRIBUTING.md with contribution guidelines
   - [ ] Ensure all public APIs have documentation

3. **Legal and Security**
   - [ ] Verify LICENSE file is present (Apache-2.0)
   - [ ] Add SECURITY.md for security policy
   - [ ] Review code for any sensitive information

4. **Quality Checks**
   - [ ] Run `cargo fmt -- --check`
   - [ ] Run `cargo clippy -- -D warnings`
   - [ ] Run `cargo test --all-features`
   - [ ] Check for outdated dependencies: `cargo outdated`

5. **Publish Dry Run**
   - [ ] Run `cargo publish --dry-run`
   - [ ] Review what files will be included
   - [ ] Check package size is reasonable

## Final Steps

1. **Git Cleanup**
   - [ ] Remove `go_legacy/` directory if no longer needed
   - [ ] Remove any `.go_backup` files
   - [ ] Update `.gitignore` to exclude legacy files
   - [ ] Commit all changes: `git add . && git commit -m "Complete Rust migration"`

2. **GitHub Repository**
   - [ ] Update repository description
   - [ ] Add topics: sigma, rust, security, event-detection
   - [ ] Set up GitHub Actions for CI/CD
   - [ ] Enable security scanning

3. **Publish to crates.io**
   - [ ] Create account on crates.io if needed
   - [ ] Run `cargo login` with your API token
   - [ ] Publish: `cargo publish`
   - [ ] Verify on https://crates.io/crates/sigma-rs

4. **Post-Publication**
   - [ ] Tag the release: `git tag -a v0.1.0 -m "Initial release"`
   - [ ] Push tags: `git push origin --tags`
   - [ ] Create GitHub release with changelog
   - [ ] Update README badges with actual links
   - [ ] Announce on relevant forums/communities

## Rollback Plan

If something goes wrong:

1. Restore from backup branch: `git checkout backup-pre-migration`
2. Or restore Go files from `go_legacy/` directory
3. Check `*.go_backup` files for original configurations

## Notes

- The migration script creates backups of conflicting files
- Go code is preserved in `go_legacy/` for reference
- All Rust code should now be at the repository root
- Update import paths in any external projects using this library