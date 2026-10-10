//! Ready-made setups to start from (UC-08.15): our own content, written as Tana Paste and imported
//! with `import.tanaPaste` into the Library. Each one is a single page.

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
];
