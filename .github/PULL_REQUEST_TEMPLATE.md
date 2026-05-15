<!--
Status: Current
Last updated: 2026-04-29 07:58 EDT
-->

## Summary

- What changed?
- Why was this change needed?

## Subsystems touched

- [ ] spec / generated artifacts
- [ ] grammar / tree-sitter
- [ ] core Rust crates (`talkbank-*`)
- [ ] `chatter` CLI
- [ ] `chatter-lsp`
- [ ] CLAN commands / converters
- [ ] `batchalign3` / `batchalign-*`
- [ ] VS Code extension
- [ ] desktop / experimental surfaces
- [ ] docs only

## Contract impact

- [ ] No public contract change
- [ ] Public stable surface changed
- [ ] Public preview surface changed
- [ ] Internal-only change

Describe any user-visible, API, schema, or workflow impact:

## Generated artifact impact

- [ ] No generated artifacts changed
- [ ] Generated artifacts changed and are included in this PR
- [ ] Generated artifacts should change but require maintainer regeneration

If maintainer follow-up is needed, explain why:

## Validation

- [ ] `bazel build //... && bazel test //...`
- [ ] `bazel build //crates/batchalign/...`
- [ ] `bazel test //crates/batchalign/...`
- [ ] `bazel test //crates/batchalign/...`
- [ ] `just batchalign pytest && just batchalign lint`
- [ ] other (describe below)

List the commands you ran and any intentionally skipped checks:

## Docs and follow-up

- [ ] Docs updated
- [ ] Docs not needed
- [ ] Integrator / downstream impact documented
- [ ] No downstream impact

## Checklist

- [ ] I filled in the sections above.
- [ ] I added or updated tests where behavior changed.
- [ ] I documented any pre-existing or unrelated failures instead of hiding them.
