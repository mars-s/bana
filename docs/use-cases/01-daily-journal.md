# 01 Daily journal and calendar pages

Tana's day pages are the default place to write: one page per day, grouped in weeks and years,
with an optional `#day` template. Scenarios: `web/tests/use-cases/01-daily-journal.spec.ts`
(fixture today: Sat, 10 Oct 2026).

### UC-01.01 Open today from anywhere: WORKS
- **Story:** I want one key or click that always brings me back to today's page.
- **Steps:** Click Today in the sidebar, or press Ctrl+Shift+D from any page.
- **Expected:** Today's day page opens in the panel.
- **Source:** https://outliner.tana.inc/learn/features/daily-notes, https://outliner.tana.inc/learn/features/sidebar
- **TanaCraft:** WORKS (both ways checked).

### UC-01.02 Step to the previous and next day: WORKS
- **Story:** While reviewing my week I flip back and forth between days.
- **Steps:** Use the < and > buttons under the day title; Today returns.
- **Expected:** Yesterday's and tomorrow's pages open, created if needed.
- **Source:** https://outliner.tana.inc/learn/features/dates-and-calendar-nodes
- **TanaCraft:** WORKS. (Tana also has Ctrl+[ and Ctrl+]; here Mod+[ and Mod+] are Back and Forward.)

### UC-01.03 Jump to any date: WORKS
- **Story:** I want to write on a specific future day, for example a trip day.
- **Steps:** Open the date picker in the day toolbar, pick a day, OK.
- **Expected:** That day's page opens.
- **Source:** https://outliner.tana.inc/learn/features/daily-notes
- **TanaCraft:** WORKS.

### UC-01.04 Week and year pages: WORKS
- **Story:** I keep weekly notes above my days.
- **Steps:** Click the week (or year) in the breadcrumbs of a day page.
- **Expected:** The week page lists its days; the year lists its weeks.
- **Source:** https://outliner.tana.inc/learn/features/dates-and-calendar-nodes
- **TanaCraft:** WORKS. Tana's month pages (reached from a month date) are not modelled.

### UC-01.05 A template for every day: WORKS
- **Story:** Each morning my day page should already hold my routine and a list of open tasks.
- **Steps:** Give the `#day` supertag template content or related content; open a new day.
- **Expected:** The new day carries `#day` and its template (here an Open tasks card).
- **Source:** https://outliner.tana.inc/learn/features/daily-notes
- **TanaCraft:** WORKS.

### UC-01.06 Quick add to today: WORKS
- **Story:** A thought comes while I am deep in another page; I want it on today without leaving.
- **Steps:** Press Cmd+E (Tana: Cmd+Shift+Space or Create new), type, Enter.
- **Expected:** The line lands on today's page; I stay where I was.
- **Source:** https://outliner.tana.inc/learn/features/sidebar
- **TanaCraft:** WORKS.

### UC-01.07 Dated things show on their day: WORKS
- **Story:** A task due Monday should appear on Monday's page.
- **Steps:** Set a date field on a node; open that day.
- **Expected:** The day's reference section lists the node.
- **Source:** https://outliner.tana.inc/learn/features/dates-and-calendar-nodes
- **TanaCraft:** WORKS for date fields. An inline date link inside text did not show in the day's references when probed.

### UC-01.08 Put a date in a line: PARTIAL
- **Story:** I type "call the plumber @tomorrow" and want a real date link.
- **Steps:** Type @ and a date in words (today, next friday, Nov 21 to Nov 30).
- **Expected:** A date object linked to its day page.
- **Source:** https://outliner.tana.inc/learn/features/dates-and-calendar-nodes
- **TanaCraft:** PARTIAL. The Insert date command puts a link to today's page in the line. Missing (UC-01.08b): @ with natural-language dates, date ranges and week or month dates in text.

### UC-01.09 Journal prompts: MISSING
- **Story:** My `#journal` entry should offer three random prompts from my prompt list.
- **Steps:** A field set to auto-initialize to random nodes with a supertag.
- **Expected:** New entries get random prompts.
- **Source:** https://outliner.tana.inc/learn/features/fields (Auto-initialize to random node)
- **TanaCraft:** MISSING (auto-initialize offers current date, ancestor day and ancestor value).

### UC-01.10 Habit check-off: PARTIAL
- **Story:** I tick my habits each day and want to see how often I did them.
- **Steps:** Checkboxes on the day page; a search such as "done in the last 7 days".
- **Expected:** Ticks recorded with done time; counts and a streak view.
- **Source:** https://outliner.tana.inc/learn/tutorials/habit-tracker
- **TanaCraft:** PARTIAL. Checkboxes and a DONE:7 search work; no streak or calendar view of a habit.

### UC-01.11 Weekly review page: WORKS
- **Story:** On Fridays I write a weekly review on the week's own page.
- **Steps:** Open the week page; it carries its `#week` tag and template.
- **Expected:** A week page with its own template.
- **Source:** https://outliner.tana.inc/learn/tutorials/weekly-reflection
- **TanaCraft:** WORKS.

### UC-01.12 Add to day (Remind me): MISSING
- **Story:** I want an article to come back to me next Tuesday.
- **Steps:** Run Add to day on a node and pick a day.
- **Expected:** A reference to the node appears on that day's page.
- **Source:** https://outliner.tana.inc/changelog (2026 wk 37-39, "Add to day")
- **TanaCraft:** MISSING.

### UC-01.13 Week start and time format: PARTIAL
- **Story:** My weeks start on Sunday and I read times in 12-hour format.
- **Steps:** Settings, Date and time: First day of the week, Time format.
- **Expected:** Calendars and date pickers start on that day; times show as chosen.
- **Source:** https://outliner.tana.inc/learn/features/dates-and-calendar-nodes
- **TanaCraft:** PARTIAL. Both settings save and calendar views start on Sunday; the date picker still starts on Monday. No per-workspace time zone.

### UC-01.14 Daily notes in a second workspace: MISSING
- **Story:** My club workspace should have its own day pages.
- **Steps:** Switch calendars to another workspace.
- **Expected:** That workspace gets its own day, week and year pages.
- **Source:** https://outliner.tana.inc/learn/features/workspaces
- **TanaCraft:** MISSING (Today follows the main workspace, see ROADMAP).

### UC-01.15 Tasks due on this day: WORKS
- **Story:** Each day page lists the tasks due that day, past days too.
- **Steps:** A search in the `#day` template with Due = PARENT (or LT PARENT for overdue).
- **Expected:** Results follow the page's own date.
- **Source:** https://outliner.tana.inc/learn/features/search-nodes (PARENT field values)
- **TanaCraft:** WORKS. On a day page PARENT is that day's date: `Due:PARENT` and `Due<PARENT` in a search under the day, or as related content on `#day` (`tag.addRelated`), list what is due that day and what is overdue, past days too.
