# 02 Tasks and projects

Checkboxes on any line, a `#task` supertag with fields, searches that gather tasks, boards and
calendars. Scenarios: `web/tests/use-cases/02-tasks-projects.spec.ts`.

### UC-02.01 Turn any line into a checkbox: WORKS
- **Story:** While writing notes I mark a line as something to do, and later as done.
- **Steps:** Cmd+Enter on a line: checkbox, again: done, again: no checkbox.
- **Expected:** Three states, cycled from the keyboard.
- **Source:** https://outliner.tana.inc/learn/features/checkboxes-and-progress-bars
- **TanaCraft:** WORKS.

### UC-02.02 A quick to-do list: WORKS
- **Story:** I type a list of chores, each one a checkbox.
- **Steps:** Make one line a checkbox, press Enter, type the next.
- **Expected:** The new line is a checkbox too.
- **Source:** https://outliner.tana.inc/learn/features/checkboxes-and-progress-bars
- **TanaCraft:** WORKS.

### UC-02.03 Progress of a list: WORKS
- **Story:** A project line should show how much of the work below it is done.
- **Steps:** Put checkboxes under a line.
- **Expected:** A progress bar appears by itself and fills as boxes are ticked.
- **Source:** https://outliner.tana.inc/learn/features/checkboxes-and-progress-bars
- **TanaCraft:** WORKS. The bar shows by itself once a checkbox sits below and fills as boxes are ticked. No recency colours.

### UC-02.04 Project progress in the sidebar: WORKS
- **Story:** My pinned projects show a progress ring in the sidebar.
- **Source:** https://outliner.tana.inc/learn/features/sidebar (Progress bars in the sidebar)
- **TanaCraft:** WORKS. A pinned node with checkboxes below it shows a ring of done out of all.

### UC-02.05 Make a task with #task: WORKS
- **Story:** Typing #task on a line should give it a checkbox and my task fields.
- **Steps:** Type ` #task`, Enter.
- **Expected:** Tag chip, checkbox, Status, Due and Effort fields.
- **Source:** https://outliner.tana.inc/learn/features/supertags
- **TanaCraft:** WORKS.

### UC-02.06 Checkbox mapped to a Status field: WORKS
- **Story:** Ticking a task should set Status to Done, unticking back to Todo.
- **Steps:** Configure the supertag: map checked and unchecked to field values.
- **Expected:** Field and checkbox stay in step both ways.
- **Source:** https://outliner.tana.inc/learn/features/supertags (Done state mapping)
- **TanaCraft:** WORKS.

### UC-02.07 All open tasks in one list: WORKS
- **Story:** One page shows every unchecked task wherever it was written.
- **Steps:** A search node for `#task` and NOT DONE.
- **Expected:** New tasks appear; done ones leave.
- **Source:** https://outliner.tana.inc/learn/features/search-nodes
- **TanaCraft:** WORKS.

### UC-02.08 Overdue tasks: WORKS
- **Story:** Show me unchecked tasks whose due date has passed.
- **Steps:** Search `#task`, TODO, Due before today.
- **Expected:** Only late tasks.
- **Source:** https://outliner.tana.inc/learn/features/search-nodes (Find overdue tasks)
- **TanaCraft:** WORKS relative to today (see UC-01.15 for PARENT on day pages).

### UC-02.09 Kanban board by status: WORKS
- **Story:** I move tasks across Not started, In progress and Done.
- **Steps:** View as Cards, group by Status, drag a card.
- **Expected:** The card's Status changes.
- **Source:** https://outliner.tana.inc/learn/features/views (Cards view)
- **TanaCraft:** WORKS.

### UC-02.10 A project page lists its tasks: WORKS
- **Story:** Each project shows the tasks that point at it.
- **Steps:** A Project field (options from `#project`) on tasks; a search on the project with Project = PARENT.
- **Expected:** Only that project's tasks.
- **Source:** https://outliner.tana.inc/learn/tutorials/project, https://outliner.tana.inc/learn/features/search-nodes
- **TanaCraft:** WORKS for a search placed on the project. A shared one in the `#project` template is covered by UC-03.05's gap.

### UC-02.11 Gather loose to-dos under a page: PARTIAL
- **Story:** After a long meeting page I collect every checkbox written in it.
- **Steps:** Find todos within (Tana command).
- **Expected:** A list of the checkboxes below.
- **Source:** https://outliner.tana.inc/learn/features/checkboxes-and-progress-bars
- **TanaCraft:** PARTIAL. A search `TODO` inside the parent does it; there is no one-step command.

### UC-02.12 Move a line to another page: WORKS
- **Story:** I file a captured line into the right project.
- **Steps:** Select the line, Move in the toolbar, type the target, Enter.
- **Expected:** The line moves (optionally leaving a reference).
- **Source:** https://outliner.tana.inc/learn/features/outline-editor (The Move to menu)
- **TanaCraft:** WORKS. Cmd+Shift+M opens it from the keyboard (UC-02.19).

### UC-02.13 Calendar of tasks: WORKS
- **Story:** I see my tasks on a month calendar and reschedule by dragging.
- **Steps:** View as Calendar, drag an event to another day.
- **Expected:** The task's date field changes.
- **Source:** https://outliner.tana.inc/learn/features/views (Calendar view)
- **TanaCraft:** WORKS (month, week and day grids; undated items listed).

### UC-02.14 Subtasks tagged by themselves: WORKS
- **Story:** Every line I add under a project becomes a task.
- **Steps:** Default child supertag on `#project` set to `#task`.
- **Expected:** New children get `#task`.
- **Source:** https://outliner.tana.inc/learn/features/supertags (Default child supertag)
- **TanaCraft:** WORKS.

### UC-02.15 Tag several lines at once: WORKS
- **Story:** I select five lines and tag them all `#task`.
- **Steps:** Select, then # or Add tag.
- **Expected:** Every selected line gets the tag.
- **Source:** https://outliner.tana.inc/learn/features/supertags (bulk add or remove)
- **TanaCraft:** WORKS with the selection toolbar's # Tag and the node menu's Add tag on a selected line (`tag.add` with `nodes`). Typing `#` on a selection types into its last line, as typing does there (DIFFS N27).

### UC-02.16 Process an inbox into move targets: WORKS
- **Story:** GTD style, I send items to fixed places with one command.
- **Steps:** Set move target on a page; on an item run Move to move target.
- **Expected:** The item moves there.
- **Source:** https://outliner.tana.inc/learn/features/outline-editor, https://outliner.tana.inc/learn/tutorials/gtd-in-tana
- **TanaCraft:** WORKS.

### UC-02.17 Sum hours in a task table: WORKS
- **Story:** My task table totals the effort column.
- **Steps:** Table view, right-click the number column, Calculate, Sum.
- **Expected:** A total under the column.
- **Source:** https://outliner.tana.inc/learn/features/views (Column calculations)
- **TanaCraft:** WORKS. Tana's click-to-copy on a total is not there.

### UC-02.18 Lanes: WORKS
- **Story:** A project's sections side by side as columns.
- **Steps:** View as Lanes.
- **Expected:** Each child in its own column.
- **Source:** https://outliner.tana.inc/changelog (Oct 9, 2026: Lanes view)
- **TanaCraft:** WORKS.

### UC-02.19 Move to from the keyboard: WORKS
- **Story:** Cmd+Shift+M opens Move to on the focused line.
- **Source:** https://outliner.tana.inc/learn/features/outline-editor
- **TanaCraft:** WORKS. Cmd+Shift+M (`ui.moveTo`) opens the Move to picker under the line, as the toolbar and the command line step do.

### UC-02.20 Commands on events: WORKS
- **Story:** When a task is checked, stamp a field; when a child is added to my Inbox, tag it.
- **Source:** https://outliner.tana.inc/learn/features/supertags (Trigger commands on events), https://outliner.tana.inc/learn/features/nodes-and-references (On child added/removed)
- **TanaCraft:** WORKS. A command node runs when a supertag is added or a tagged node is checked (Configure supertag, Commands on events) or when a child comes under a node (command line on the line: "On new children run …"); `commandNode.on`. It runs in the same undo step and starts no further events. Not built: on child removed, on field changed.
