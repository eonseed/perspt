# Release Process Guide

This document outlines the release process for Perspt using the automated GitHub Actions workflows.

## 🚀 Creating a Release

### Automatic Release (Recommended)

1. **Update Version Numbers**
   - Update version in `Cargo.toml`:
     ```toml
     [package]
     version = "0.5.0"  # Update this
     ```
   - Update any version references in documentation

2. **Create and Push a Git Tag**
   ```bash
   git tag v0.5.0
   git push origin v0.5.0
   ```

3. **Automatic Process**
   - GitHub Actions will automatically:
     - Create a new GitHub release
     - Build binaries for all platforms (Linux, Windows, macOS x86_64, macOS ARM64)
     - Generate and upload documentation
     - Create checksums for all binaries
     - Upload all artifacts to the release

### Manual Release (Alternative)

If you need to create a release without pushing a tag:

1. Go to **Actions** tab in the GitHub repository (`https://github.com/eonseed/perspt/actions`)
2. Select **Release** workflow
3. Click **Run workflow**
4. Enter the desired tag name (e.g., `v0.5.0`)
5. Click **Run workflow**

## 📋 Release Checklist

Before creating a release, ensure:

- [ ] All tests pass locally: `cargo test`
- [ ] Code is properly formatted: `cargo fmt --check`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Documentation builds successfully: `cargo doc`
- [ ] Sphinx documentation builds: `cd docs/perspt_book && uv run make html`
- [ ] Version number updated in `Cargo.toml`
- [ ] CHANGELOG.md updated (if you maintain one)
- [ ] All new features are documented

## 🔄 CI/CD Workflows

### 1. CI Workflow (`.github/workflows/ci.yml`)
**Triggers:** Push to main/develop, Pull requests
**Purpose:** Continuous integration testing

- **Multi-platform testing** (Ubuntu, Windows, macOS)
- **Multiple Rust versions** (stable, beta)
- **Code quality checks** (fmt, clippy)
- **Security audit** (cargo-audit)
- **Documentation building** (both Rust and Sphinx)

### 2. Release Workflow (`.github/workflows/release.yml`)
**Triggers:** Git tags starting with 'v', Manual dispatch
**Purpose:** Automated release creation

- **Creates GitHub release** with detailed release notes
- **Builds optimized binaries** for all target platforms:
  - `x86_64-unknown-linux-gnu` (Linux)
  - `x86_64-pc-windows-msvc` (Windows)
  - `x86_64-apple-darwin` (macOS Intel)
  - `aarch64-apple-darwin` (macOS ARM64)
- **Generates documentation packages**
- **Creates SHA256 checksums**

### 3. Documentation Workflow (`.github/workflows/docs.yml`)
**Triggers:** Push to main (docs changes), Manual dispatch
**Purpose:** Deploy documentation to GitHub Pages

- **Builds Rust API documentation**
- **Builds Sphinx user documentation**
- **Creates unified documentation site**
- **Deploys to GitHub Pages** (`https://eonseed.github.io/perspt/`)

## 📦 Release Artifacts

Each release includes:

### Binaries
- `perspt-linux-x86_64` - Linux 64-bit binary
- `perspt-windows-x86_64.exe` - Windows 64-bit binary
- `perspt-macos-x86_64` - macOS Intel binary
- `perspt-macos-arm64` - macOS Apple Silicon binary

### Documentation
- `documentation.zip` - Complete documentation package containing:
  - Rust API documentation (`rust-docs/`)
  - Sphinx user guide (`sphinx-html/`)
  - PDF documentation (if successfully built)

### Verification
- `checksums.txt` - SHA256 checksums for all binaries

## 🔧 Configuration

### Platform-Specific Build Configuration

The release workflow is configured to:
- **Strip binaries** on Linux/macOS for smaller file sizes
- **Use appropriate targets** for each platform
- **Cache dependencies** to speed up builds
- **Generate detailed release notes** automatically

### Documentation Build Configuration

- **Rust docs**: Built with `--no-deps --all-features`
- **Sphinx docs**: Built using uv for dependency management
- **PDF generation**: Includes LaTeX dependencies on Ubuntu

## 🛠️ Troubleshooting

### Common Issues

1. **Build Failures**
   - Check that all dependencies are properly specified
   - Ensure code compiles on all target platforms
   - Verify that clippy passes without warnings

2. **Documentation Build Failures**
   - Check Sphinx configuration in `docs/perspt_book/`
   - Verify all required Python dependencies are in `pyproject.toml`
   - Ensure Rust documentation doesn't have broken links

3. **Release Asset Upload Failures**
   - Verify GitHub token permissions
   - Check that binary files are being generated correctly
   - Ensure target directories exist after build

### Manual Intervention

If automated release fails:

1. **Check workflow logs** in GitHub Actions
2. **Fix the issue** and push changes
3. **Delete the failed tag** if necessary:
   ```bash
   git tag -d v0.5.0
   git push origin :refs/tags/v0.5.0
   ```
4. **Recreate the tag** after fixes

## 📊 Monitoring

Monitor release health through:
- **GitHub Actions status** for workflow success (`https://github.com/eonseed/perspt/actions`)
- **Download statistics** on release pages (`https://github.com/eonseed/perspt/releases`)
- **User feedback** through issues and discussions
- **Documentation accessibility** via GitHub Pages (`https://eonseed.github.io/perspt/`)

## 🔄 Dependency Updates

The project uses Dependabot to automatically:
- **Update Rust dependencies** weekly
- **Update GitHub Actions** versions
- **Update Python dependencies** in documentation

Review and merge Dependabot PRs regularly to keep dependencies current.
