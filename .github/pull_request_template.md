# Pull Request

## Description

**What does this PR do?**

Provide a clear and concise description of the changes in this pull request.

**Related Issue(s):**
- Closes #
- Related to #
- Fixes #

---

## Type of Change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring
- [ ] Test improvements
- [ ] CI/CD changes
- [ ] Other (please describe):

---

## Changes Made

**Summary of changes:**

- Change 1: Description
- Change 2: Description
- Change 3: Description

**Key files modified:**

- `crates/mbongo-*/src/...`
- `docs/...`
- `tests/...`

---

## Testing

### Test Coverage

- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Performance benchmarks added/updated
- [ ] Manual testing completed

**Test Results:**

```bash
# Paste test output here
cargo test --all
```

### Manual Testing

Describe manual testing steps performed:

1. Step 1
2. Step 2
3. Expected result
4. Actual result

---

## Code Quality

### Rust Code Quality Checks

- [ ] `cargo fmt` - Code is properly formatted
- [ ] `cargo clippy` - No clippy warnings
- [ ] `cargo check` - Code compiles without errors
- [ ] `cargo test` - All tests pass
- [ ] No compiler warnings (`cargo build --all-features`)

**Clippy Output:**

```bash
# Paste clippy output (should be clean)
cargo clippy --all-targets --all-features
```

### Code Review

- [ ] Self-reviewed my own code
- [ ] Added comments for complex logic
- [ ] Updated inline documentation (rustdoc)
- [ ] Code follows project style guidelines
- [ ] No unnecessary dependencies added

---

## Documentation

- [ ] Updated public API documentation (rustdoc)
- [ ] Updated architectural documentation (if applicable)
- [ ] Updated README or getting started guides (if applicable)
- [ ] Added/updated code examples
- [ ] Updated CHANGELOG.md
- [ ] No documentation-only changes require changelog entry

**Documentation Changes:**

List specific documentation files modified or added.

---

## Performance Impact

**Does this PR affect performance?**

- [ ] No performance impact
- [ ] Performance improvement (include benchmarks)
- [ ] Potential performance regression (explain why acceptable)

**Benchmark Results (if applicable):**

```
# Paste benchmark output
cargo bench
```

---

## Security Considerations

**Security Review:**

- [ ] This PR does NOT affect security-critical code
- [ ] This PR affects security-critical code and requires review
- [ ] Security audit completed (for critical changes)

**Security Checklist (if applicable):**

- [ ] No hardcoded secrets or credentials
- [ ] Input validation added for user-supplied data
- [ ] No SQL injection or command injection vulnerabilities
- [ ] Cryptographic operations use approved libraries
- [ ] No exposure of sensitive information in logs
- [ ] Authentication/authorization properly implemented

---

## Breaking Changes

**Does this PR introduce breaking changes?**

- [ ] No breaking changes
- [ ] Yes, breaking changes (describe below)

**If breaking changes, describe:**

- What breaks:
- Migration path:
- Impact assessment:

---

## Deployment Notes

**Special deployment considerations:**

- [ ] No special deployment steps required
- [ ] Database migrations required
- [ ] Configuration changes required
- [ ] Requires node restart
- [ ] Backward compatibility maintained

**Deployment Instructions:**

```bash
# Add any special deployment steps
```

---

## Dependencies

**New Dependencies Added:**

- None
- Dependency 1: `crate-name = "version"` - Reason for adding

**Dependency Updates:**

- None
- Dependency 1: Updated from `x.y.z` to `x.y.z` - Reason for update

---

## Checklist

### Before Submitting

- [ ] I have read the [CONTRIBUTING.md](../CONTRIBUTING.md) guide
- [ ] My code follows the project's style guidelines
- [ ] I have performed a self-review of my own code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have made corresponding changes to the documentation
- [ ] My changes generate no new warnings
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] New and existing unit tests pass locally with my changes
- [ ] Any dependent changes have been merged and published

### Code Quality

- [ ] `cargo fmt` passes
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo test --all` passes
- [ ] `cargo build --release` succeeds

### Documentation

- [ ] Rustdoc comments added for public APIs
- [ ] Architectural docs updated (if needed)
- [ ] CHANGELOG.md updated (if needed)

### Linked Issues

- [ ] This PR is linked to at least one issue
- [ ] All acceptance criteria from linked issues are met

---

## Screenshots (if applicable)

Add screenshots to demonstrate UI changes or visual improvements.

---

## Additional Notes

Add any other context about the pull request here.

**Reviewer Notes:**

Tag specific reviewers or areas that need special attention:
- @reviewer1 - Please review consensus logic
- @reviewer2 - Please review cryptographic implementation

---

## Post-Merge Tasks

- [ ] Update related documentation
- [ ] Announce in Discord/community channels (if user-facing)
- [ ] Close related issues
- [ ] Create follow-up issues (if needed)

---

**Pull Request Author:** @[your-username]  
**Target Merge Date:** [Date if time-sensitive]
