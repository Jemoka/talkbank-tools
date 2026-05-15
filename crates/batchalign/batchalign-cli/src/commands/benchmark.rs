//! Command-owned metadata for `benchmark`.

use crate::ReleasedCommand;
use crate::commands::spec::declare_benchmark_command;

declare_benchmark_command!(
    BenchmarkCommand,
    BENCHMARK_DEFINITION,
    ReleasedCommand::Benchmark
);
