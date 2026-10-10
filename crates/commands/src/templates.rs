//! Ready-made setups to start from (UC-08.15): our own content, written as Tana Paste and imported
//! with `import.tanaPaste` into the Library. Each one is a single page. A template may also set up
//! supertags with fields ([`SCHEMAS`]), made before its page, so tagging a line shows the rows.

/// (name, description, Tana Paste).
pub const TEMPLATES: &[(&str, &str, &str)] = &[
    (
        "Task manager",
        "Tasks with due dates, an open-tasks table and a done list.",
        "%%tana%%
- Task manager
  - [ ] Plan the week #task
    - Due:: [[date:2026-10-12]]
  - [ ] Book the dentist #task
  - [x] Pay the electricity bill #task
  - Open tasks %%search%% %%view:table%%
    - #task
    - TODO
  - Done %%search%%
    - #task
    - DONE
",
    ),
    (
        "Habit tracker",
        "Daily habits as checkboxes, with a cue and a goal for each.",
        "%%tana%%
- Habit tracker
  - [ ] Drink a glass of water first thing #habit
    - Cue:: Waking up
    - Goal:: Every day
  - [ ] Walk for 20 minutes #habit
    - Cue:: After lunch
    - Goal:: 5 days a week
  - [ ] Read 10 pages #habit
    - Cue:: Before bed
    - Goal:: Every day
  - All habits %%search%% %%view:table%%
    - #habit
",
    ),
    (
        "Goals and key results",
        "Objectives with measurable key results and a table of every result.",
        "%%tana%%
- Goals and key results
  - Grow the newsletter #objective
    - Quarter:: Q4
    - Reach 1000 subscribers #keyresult
      - Progress:: 40%
    - Send one issue every week #keyresult
      - Progress:: 70%
  - Get fit for the spring hike #objective
    - Quarter:: Q4
    - Run 5 km without stopping #keyresult
      - Progress:: 20%
  - All key results %%search%% %%view:table%%
    - #keyresult
",
    ),
    (
        "Reading list",
        "Books with author and status, and a table of what is next.",
        "%%tana%%
- Reading list
  - The book I keep meaning to start #book
    - Author:: Someone Great
    - Status:: To read
  - A book I am halfway through #book
    - Author:: Another Writer
    - Status:: Reading
  - Up next %%search%% %%view:table%%
    - #book
    - Status:: To read
",
    ),
    (
        "Meeting notes",
        "A meeting with attendees, agenda, notes and action items.",
        "%%tana%%
- Meeting notes
  - Weekly sync #meeting
    - Date:: [[date:2026-10-12]]
    - Attendees:: 
    - !! Agenda
      - Last week's actions
      - This week's priorities
    - !! Notes
    - !! Action items
      - [ ] Send the summary #task
",
    ),
    (
        "Agent notebook",
        "Supertags and searches for assistants that capture, interpret and propose: captures, suggested actions, questions, mirrored to-dos and events, proposals, an agent inbox and agent activity.",
        "%%tana%%
- Agent notebook
  - Assistants write here. Tag a line with ask-agent to leave work for one; proposals wait for you to approve them outside the notes.
  - Agent inbox %%search%%
    - #ask-agent
  - Pending proposals %%search%% %%view:table%%
    - #proposal
    - NOT State:applied
    - NOT State:rejected
  - Captures this week %%search%% %%view:table%%
    - #capture
    - CREATED:7
  - Agent activity %%search%% %%group:edited%%
    - BY:*
    - EDITED:30
",
    ),
];

/// A field on a template's supertag: (name, type), the type as `field.setType` takes it.
pub type FieldSpec = (&'static str, &'static str);

/// A supertag a template sets up, with its fields.
pub type TagSpec = (&'static str, &'static [FieldSpec]);

/// Supertags a template sets up before its page: (template, supertags).
pub const SCHEMAS: &[(&str, &[TagSpec])] = &[(
    "Agent notebook",
    &[
        ("capture", &[("Capture ID", "plain"), ("Source hash", "plain"), ("Status", "plain")]),
        ("interpretation", &[("Revision hash", "plain")]),
        ("suggested-action", &[("Evidence", "plain"), ("Status", "plain"), ("Things ID", "plain")]),
        ("question", &[("Evidence", "plain")]),
        ("meeting", &[("Date", "date"), ("Attendees", "plain")]),
        (
            "thing",
            &[
                ("Things ID", "plain"),
                ("Things link", "url"),
                ("Status", "plain"),
                ("When", "date"),
                ("Deadline", "date"),
                ("Project", "plain"),
                ("Area", "plain"),
                ("Synced at", "date"),
            ],
        ),
        ("event", &[("Event ID", "plain"), ("Start", "date"), ("End", "date"), ("Calendar", "plain"), ("Synced at", "date")]),
        ("proposal", &[("Proposal ID", "plain"), ("Revision hash", "plain"), ("State", "plain")]),
        ("in-things", &[]),
        ("ask-agent", &[]),
    ],
)];
