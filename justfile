# Batchalign workspace runner. Build and test delegate to the authoritative
# Batchalign scope so the same target selection and memory limits apply.

set shell := ["bash", "-c"]
set positional-arguments := true

mod batchalign "just/batchalign.just"
mod docs       "just/docs.just"

default:
    @just --list

# Build the Batchalign product.
build profile="release":
    just batchalign build {{ profile }}

# Test the Batchalign product.
test profile="release":
    just batchalign test {{ profile }}

# Print Batchalign versions.
versions:
    @just batchalign versions
