---
name: Bug report
about: Report something that is broken or behaves incorrectly
title: "[Bug] "
labels: ["bug", "triage"]
assignees: []
---

## Summary

A clear, one-sentence description of the bug.

## Environment

- **Version / commit**: (run `accelerate --version` or `git rev-parse HEAD`)
- **Operating system**: (e.g. Ubuntu 24.04, macOS 15, Windows 11)
- **Architecture**: (e.g. x86_64, aarch64)
- **Build profile**: `cargo build` (debug) or `cargo build --release`
- **Features enabled**: (e.g. `--features mimalloc`)

## Steps to Reproduce

1. `accelerate start ...`
2. `curl -X POST ...`
3. ...

## Expected Behaviour

What you expected to happen.

## Actual Behaviour

What actually happened. Include the exact error message, stack trace, or
HTTP response body. If the message references an issue URL, please open a
ticket at <https://github.com/muhammad-fiaz/AccelerateSearch/issues> as
recommended by the in-app error hint.

## Logs

Attach or paste the relevant logs. If the message is a server-side
internal error, please include the full log line including any UUIDs.

## Reproducibility

- [ ] Always reproduces
- [ ] Sometimes reproduces
- [ ] Once and could not reproduce again

## Possible Cause

If you have any insight into the root cause, share it here.
