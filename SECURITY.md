# Security Policy

**Status:** Current
**Last updated:** 2026-09-06

## Reporting a Vulnerability

Please do **not** report security issues in public GitHub issues, pull
requests, discussions, or commit messages.

Preferred reporting channels:

1. GitHub private vulnerability reporting for `TalkBank/batchalign`
2. Email [franklinchen@franklinchen.com](mailto:franklinchen@franklinchen.com)

Please include:

- the affected Batchalign command, engine, application, or other repository path
- the commit SHA, branch, or released version you tested
- reproduction steps or a proof of concept
- impact assessment
- any suggested remediation or mitigation

We will acknowledge reports, investigate them privately, and coordinate a fix
and disclosure plan before public discussion when the report is validated.

## Scope and Supported Versions

For security triage, please report the issue against one of these:

- the current `main` branch, or
- the latest tagged release for the affected public surface

The current public release lines are documented in
[`book/src/operations/release-contract.md`](book/src/operations/release-contract.md) and
[`book/src/operations/versioning.md`](book/src/operations/versioning.md). Older releases may still be
investigated, but maintainers may ask you to reproduce the issue on a current
build before triage.

## What Happens Next

After a report is validated, maintainers will:

1. scope the affected surfaces
2. prepare and verify a fix
3. coordinate release timing through the existing release workflows
4. publish disclosure details once users have a remediation path

## Non-Security Bugs

For parser bugs, feature requests, documentation issues, and other non-security
reports, use the standard GitHub issue templates in `.github/ISSUE_TEMPLATE/`.
