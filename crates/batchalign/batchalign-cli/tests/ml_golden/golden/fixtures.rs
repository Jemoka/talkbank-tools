pub(crate) const ENG_SIMPLE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\thello world .
@End
";

pub(crate) const ENG_MULTI_UTT: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tthe dog is running .
*PAR:\tI like cats .
*PAR:\tshe went to the store .
@End
";

pub(crate) const SPA_SIMPLE: &str = "\
@UTF8
@Begin
@Languages:\tspa
@Participants:\tPAR Participant
@ID:\tspa|test|PAR|||||Participant|||
*PAR:\tel gato es grande .
@End
";

pub(crate) const COMPARE_MAIN: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tthe big dog is running .
*PAR:\tI like cats .
@End
";

pub(crate) const COMPARE_GOLD: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tthe dog is running quickly .
*PAR:\tI like cats .
@End
";

pub(crate) const ENG_COREF: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI||female|||Target_Child|||
*CHI:\tthe dog ran .
*CHI:\tit was fast .
*CHI:\tthe cat slept .
@End
";

pub(crate) const ENG_RETOKENIZE: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tgonna eat cookies .
@End
";

pub(crate) const SPA_MULTI_UTT: &str = "\
@UTF8
@Begin
@Languages:\tspa
@Participants:\tPAR Participant
@ID:\tspa|test|PAR|||||Participant|||
*PAR:\tel perro corre .
*PAR:\tme gustan los gatos .
@End
";

pub(crate) const ENG_MULTI_SPEAKER_PARITY: &str =
    include_str!("../../../../../../python/batchalign/tests/support/parity/eng_multi_speaker.cha");

pub(crate) const ENG_DISFLUENCY_PARITY: &str =
    include_str!("../../../../../../python/batchalign/tests/support/parity/eng_disfluency.cha");

pub(crate) const ENG_SPA_L2: &str = "\
@UTF8
@Begin
@Languages:\teng, spa
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\tI went to the tienda@s:spa yesterday .
*PAR:\tshe was muy@s:spa nice .
*PAR:\twe talked about los@s:spa niños@s:spa .
*PAR:\tso I said hello back .
@End
";

pub(crate) const DEU_ENG_L2: &str = "\
@UTF8
@Begin
@Languages:\tdeu, eng
@Participants:\tEVA Participant
@ID:\tdeu|test|EVA|||||Participant|||
*EVA:\tich möchte film@s studies@s machen .
*EVA:\twir haben eine drug@s factory@s oben .
@End
";

pub(crate) const DEU_ENG_CONTRACTIONS: &str = "\
@UTF8
@Begin
@Languages:\tdeu, eng
@Participants:\tPAR Participant
@ID:\tdeu|test|PAR|||||Participant|||
*PAR:\tich glaube it's@s:eng working@s:eng und don't@s:eng stop@s:eng .
@End
";

pub(crate) const DEU_ENG_PHRASAL: &str = "\
@UTF8
@Begin
@Languages:\tdeu, eng
@Participants:\tPAR Participant
@ID:\tdeu|test|PAR|||||Participant|||
*PAR:\tich möchte wake@s up@s jetzt .
*PAR:\tdie kinder give@s up@s immer .
*PAR:\tsie pick@s up@s das buch .
*PAR:\tdie zeit ist time@s out@s .
@End
";

pub(crate) const CAT_SPA_L2: &str = "\
@UTF8
@Begin
@Languages:\tcat, spa
@Participants:\tMOT Mother
@ID:\tcat|test|MOT|||||Mother|||
*MOT:\tavui anem al cole@s per jugar .
*MOT:\tla nina és molt bonita@s .
@End
";

pub(crate) const DAN_ENG_L2: &str = "\
@UTF8
@Begin
@Languages:\tdan, eng
@Participants:\tPAR Participant
@ID:\tdan|test|PAR|||||Participant|||
*PAR:\tjeg kan godt lide hendes computer@s game@s .
*PAR:\thun er meget happy@s today@s .
@End
";

pub(crate) const FRA_NLD_L2: &str = "\
@UTF8
@Begin
@Languages:\tfra, nld
@Participants:\tCHI Target_Child
@ID:\tfra|test|CHI|||||Target_Child|||
*CHI:\tvoici opa@s et oma@s .
*CHI:\tje dis ja@s:nld maintenant .
@End
";
