//! Stub: the batchalign-types-driven runtime TOML codegen retired with the
//! batchalign rewrite (see spec2.md). New python/batchalign package does not
//! consume a runtime_constants.toml. This module is retained only so existing
//! xtask subcommand dispatchers continue to link.

pub fn run(_check: bool) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("gen-runtime-toml: no-op (retired with batchalign rewrite)");
    Ok(())
}
