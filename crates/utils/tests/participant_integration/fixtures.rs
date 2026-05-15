//! Test module for fixtures in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

pub const BASIC_PARTICIPANTS: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Ruth Target_Child, INV Chiat Investigator
@ID:	eng|chiat|CHI|10;03.||||Target_Child|||
@ID:	eng|chiat|INV|||||Investigator|||
@Birth of CHI:	28-JUN-2001
*CHI:	hello .
*INV:	hi there .
@End
"#;

pub const NO_BIRTH_DATE: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	MOT Mother
@ID:	eng|corpus|MOT|||||Mother|||
*MOT:	hello .
@End
"#;

pub const MIXED_BIRTH_DATES: &str = r#"@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child, MOT Mother, FAT Father
@ID:	eng|corpus|CHI|2;06.00|female|||Target_Child|||
@ID:	eng|corpus|MOT|||||Mother|||
@ID:	eng|corpus|FAT|||||Father|||
@Birth of CHI:	15-MAR-2020
@Birth of FAT:	10-JUN-1985
*CHI:	hello .
*MOT:	hi .
*FAT:	hey there .
@End
"#;
