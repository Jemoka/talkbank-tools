fn main() {
    // Propagate the repo's stamped BUILD_HASH (produced by bazel/stamp.sh:
    // `<git-sha>[-dirty]`) into the binary as a compile-time env value.
    // daemon.rs reads it via `env!("BATCHALIGN_BUILD_HASH")` and uses it
    // to invalidate the PyApp install cache when the binary's build
    // differs from the one that populated the cache — PyApp's own cache
    // key only includes the wheel name + version, so feature-set changes
    // would otherwise slip past it.
    //
    // The wrapper scripts (bazel/batchalign-tauri/{dev,bundle}.sh) call
    // bazel/stamp.sh and export the result as BATCHALIGN_BUILD_HASH
    // before invoking `cargo tauri {dev,build}`. If the env var is
    // absent (someone running cargo directly outside Bazel) we fall
    // back to a "dev-unstamped" sentinel so the wipe path still works,
    // it just wipes on every build.
    println!("cargo:rerun-if-env-changed=BATCHALIGN_BUILD_HASH");
    let stamp = std::env::var("BATCHALIGN_BUILD_HASH")
        .unwrap_or_else(|_| "dev-unstamped".to_string());
    println!("cargo:rustc-env=BATCHALIGN_BUILD_HASH={stamp}");

    tauri_build::build();
}
