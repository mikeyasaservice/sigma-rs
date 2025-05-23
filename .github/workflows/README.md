# GitHub Actions Workflows

This directory contains automated workflows for the sigma-rs project.

## Workflows

### CI (`ci.yml`)
- **Trigger**: Push to main/master/dev branches, pull requests
- **Purpose**: Main CI pipeline for building and testing
- **Jobs**:
  - Test across multiple OS and Rust versions
  - Format checking with rustfmt
  - Linting with clippy
  - Code coverage with tarpaulin
  - Benchmarks (on main branch)
  - Documentation build and test

### Code Quality (`quality.yml`)
- **Trigger**: Push to main/master/dev branches, pull requests, weekly schedule
- **Purpose**: Code quality and maintainability checks
- **Jobs**:
  - Format checking
  - Clippy analysis (including pedantic and nursery lints)
  - Code complexity analysis
  - Dependency checks

### Security (`security.yml`)
- **Trigger**: Push to main/master/dev branches, pull requests, daily schedule
- **Purpose**: Security scanning and vulnerability detection
- **Jobs**:
  - Cargo audit for dependency vulnerabilities
  - Trivy vulnerability scanning
  - License compliance checking
  - Dependency review for PRs
  - Secrets scanning with Gitleaks and TruffleHog
  - Supply chain security with SBOM generation

### Pull Request (`pr.yml`)
- **Trigger**: Pull request events
- **Purpose**: PR validation and feedback
- **Jobs**:
  - Semantic PR title validation
  - Automatic labeling based on changed files
  - PR size labeling
  - Conflict detection
  - Coverage diff reporting
  - Benchmark comparison

### Release (`release.yml`)
- **Trigger**: Git tags (`v*`), manual workflow dispatch
- **Purpose**: Release automation
- **Jobs**:
  - Create GitHub release
  - Build binaries for multiple platforms
  - Publish to crates.io
  - Publish Docker images to GitHub Container Registry

### Legacy Test (`test.yml`)
- **Note**: This is a legacy workflow from the Go implementation
- **Will be removed**: Once the Rust migration is complete

## Setup

### Pre-commit Hooks
Run the setup script to install pre-commit hooks:
```bash
./scripts/setup-hooks.sh
```

### Required Secrets
- `CRATES_IO_TOKEN`: For publishing to crates.io
- `GITLEAKS_LICENSE`: Optional, for Gitleaks scanning

### Branch Protection
Recommended settings for protected branches:
- Require pull request reviews
- Require status checks: CI, Code Quality, Security
- Require branches to be up to date
- Include administrators

## Maintenance

### Adding New Workflows
1. Create a new YAML file in `.github/workflows/`
2. Define triggers, permissions, and jobs
3. Test in a feature branch before merging
4. Update this README

### Updating Dependencies
- GitHub Actions: Check for new versions of actions
- Rust toolchain: Update in `ci.yml` and other workflows
- External tools: Update installation commands