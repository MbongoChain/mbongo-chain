# Security Policy — Mbongo Chain

## Supported Versions
Only the latest commit on the `main` branch is officially supported. Reports against any
released tag are welcome, since operators may still run it.

Mbongo Chain is development software. No production network and no token exist.

## Reporting Vulnerabilities

If you discover a security issue:
DO NOT create a public GitHub issue.

Instead use GitHub's private vulnerability reporting: open the repository's
**Security** tab and choose **Report a vulnerability**. The report is visible only to you
and the repository's maintainers, so reproduction details stay out of public view.

Provide:
- affected versions, by tag or commit  
- description of the issue  
- reproduction steps  
- potential impact  
- suggested mitigation  
- PoC if available  

We will acknowledge within 72 hours.

## Disclosure

We follow coordinated disclosure:
1. Fix internally  
2. Deploy patch  
3. Public disclosure after fix  

A report stays confidential until corrected versions are available. We agree the timing
with the reporter, and ask reporters not to publish details before then. Public notices
name the affected versions, and leave exploit details out until that point.

Changes to locked protocol surfaces follow [`docs/RFC_PROCESS.md`](../docs/RFC_PROCESS.md),
which has no emergency bypass.

## Operator Notices

When a defect affects a released version, the maintainer notifies known operators
privately, before public disclosure. A notice states:

- the affected versions;
- that those versions must not be used publicly or for anything of value;
- safe operating guidance;
- how corrected versions will arrive.

A notice carries no exploit details. Those are published only after corrected versions
are available.

To receive notices, operators register through the private reporting channel above. The
list of operators is held privately by the maintainer, and is never committed to this
repository.
