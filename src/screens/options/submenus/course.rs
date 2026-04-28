use super::super::*;

const SHOW_RANDOM_COURSES_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_show_random_courses);
const SHOW_MOST_PLAYED_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_show_most_played_courses);
const SHOW_INDIVIDUAL_SCORES_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_show_course_individual_scores);
const AUTOSUBMIT_INDIVIDUAL_BINDING: CycleBinding =
    CycleBinding::Bool(config::update_autosubmit_course_scores_individually);

pub(in crate::screens::options) const COURSE_OPTIONS_ROWS: &[SubRow] = &[
    SubRow {
        id: RowId::CrsShowRandom,
        label: lookup_key("OptionsCourse", "ShowRandomCourses"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(SHOW_RANDOM_COURSES_BINDING),
    },
    SubRow {
        id: RowId::CrsShowMostPlayed,
        label: lookup_key("OptionsCourse", "ShowMostPlayed"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(SHOW_MOST_PLAYED_BINDING),
    },
    SubRow {
        id: RowId::CrsShowIndividualScores,
        label: lookup_key("OptionsCourse", "ShowIndividualScores"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(SHOW_INDIVIDUAL_SCORES_BINDING),
    },
    SubRow {
        id: RowId::CrsAutosubmitIndividual,
        label: lookup_key("OptionsCourse", "AutosubmitIndividual"),
        choices: &[
            localized_choice("Common", "No"),
            localized_choice("Common", "Yes"),
        ],
        inline: true,
        behavior: RowBehavior::Cycle(AUTOSUBMIT_INDIVIDUAL_BINDING),
    },
];

pub(in crate::screens::options) const COURSE_OPTIONS_ITEMS: &[Item] = &[
    Item {
        id: RowId::CrsShowRandom,
        name: lookup_key("OptionsCourse", "ShowRandomCourses"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsCourseHelp",
            "ShowRandomCoursesHelp",
        ))],
    },
    Item {
        id: RowId::CrsShowMostPlayed,
        name: lookup_key("OptionsCourse", "ShowMostPlayed"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsCourseHelp",
            "ShowMostPlayedHelp",
        ))],
    },
    Item {
        id: RowId::CrsShowIndividualScores,
        name: lookup_key("OptionsCourse", "ShowIndividualScores"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsCourseHelp",
            "ShowIndividualScoresHelp",
        ))],
    },
    Item {
        id: RowId::CrsAutosubmitIndividual,
        name: lookup_key("OptionsCourse", "AutosubmitIndividual"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsCourseHelp",
            "AutosubmitIndividualHelp",
        ))],
    },
    Item {
        id: RowId::Exit,
        name: lookup_key("Options", "Exit"),
        help: &[HelpEntry::Paragraph(lookup_key(
            "OptionsHelp",
            "ExitSubHelp",
        ))],
    },
];
