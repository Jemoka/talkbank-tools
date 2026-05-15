pub(super) const ENG_GONNA: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tgonna eat cookies .
@End
";

pub(super) const NLD_SIMPLE: &str = "\
@UTF8
@Begin
@Languages:\tnld
@Participants:\tPAR Participant
@ID:\tnld|test|PAR|||||Participant|||
*PAR:\tik ga naar huis .
@End
";

pub(super) const SPA_SIMPLE: &str = "\
@UTF8
@Begin
@Languages:\tspa
@Participants:\tPAR Participant
@ID:\tspa|test|PAR|||||Participant|||
*PAR:\tel perro corre .
*PAR:\tme gustan los gatos .
@End
";

pub(super) const YUE_GU_SHI: &str =
    include_str!("../../../../../../resources/fixtures/retok_yue_gu_shi.cha");

pub(super) const ZHO_SHANG_DIAN: &str = "\
@UTF8
@Begin
@Languages:\tzho
@Participants:\tPAR Participant
@ID:\tzho|test|PAR|||||Participant|||
*PAR:\t商 店 很 大 .
@End
";

pub(super) const ENG_SIMPLE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\thello world .
*PAR:\tthe dog runs .
@End
";

pub(super) const ENG_SPA_L2: &str = "\
@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tI went to the tienda@s:spa yesterday .
*PAR:\tshe was muy@s:spa nice .
@End
";

pub(super) const ENG_XYZ_L2: &str = "\
@UTF8
@Begin
@Languages:\teng, xyz
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tI saw the blorx@s:xyz yesterday .
@End
";

pub(super) const ENG_SPA_PRECODE: &str =
    include_str!("../../../../../../resources/fixtures/eng_spa_bilingual_code_switch.cha");

pub(super) const ENG_SIMPLE_SERVER: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\thello world .
*PAR:\tthe dog runs .
@End
";

pub(super) const DIRECT_ENG_FILE_A: &str =
    include_str!("../../../../../../resources/fixtures/morphotag/direct_eng_file_a.cha");

pub(super) const DIRECT_ENG_FILE_B: &str =
    include_str!("../../../../../../resources/fixtures/morphotag/direct_eng_file_b.cha");

pub(super) const DIRECT_ENG_BEFORE_INCREMENTAL: &str =
    include_str!("../../../../../../resources/fixtures/morphotag/direct_eng_before_incremental.cha");

pub(super) const DIRECT_ENG_AFTER_INCREMENTAL: &str =
    include_str!("../../../../../../resources/fixtures/morphotag/direct_eng_after_incremental.cha");

pub(super) const DIRECT_SPEAKER_MOTHER: &str =
    include_str!("../../../../../../resources/fixtures/morphotag/direct_speaker_mother.cha");

pub(super) const DIRECT_SPEAKER_FATHER: &str =
    include_str!("../../../../../../resources/fixtures/morphotag/direct_speaker_father.cha");
