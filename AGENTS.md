# Repository agent instructions

- Under no circumstances use the `yeet` skill unless the user explicitly asks for it. Unless explicitly requested otherwise, “commit/push” means only committing and pushing the current repository state; do not create a branch or pull request.
- Use `just batchalign cli` as the authoritative Batchalign CLI execution and integration entrypoint. It runs the Bazel `//python/batchalign` `py_binary`; do not substitute an ad hoc Python invocation.
- Use the repository's `just` recipes for Bazel-backed build and test workflows (for example, `just batchalign pytest` and `just batchalign test`).
