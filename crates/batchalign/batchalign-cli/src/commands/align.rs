//! Command-owned metadata for `align`.

use crate::ReleasedCommand;
use crate::commands::spec::declare_forced_alignment_command;

declare_forced_alignment_command!(AlignCommand, ALIGN_DEFINITION, ReleasedCommand::Align);
