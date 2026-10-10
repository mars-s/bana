# 03 Meetings

Meeting notes as `#meeting` nodes with attendees, a date and action items. Tana's recording,
transcripts and meeting bot are AI and cloud features. Scenarios:
`web/tests/use-cases/03-meetings.spec.ts`.

### UC-03.01 A meeting template: WORKS
- **Story:** When I tag a line `#meeting` it should fill with Agenda and Notes and my fields.
- **Steps:** Type the title, ` #meeting`, Enter.
- **Expected:** Template children and fields appear under it.
- **Source:** https://outliner.tana.inc/learn/tutorials/meeting-note-taking, https://outliner.tana.inc/learn/features/supertags
- **Bana:** WORKS.

### UC-03.02 Pick attendees from my people: WORKS
- **Story:** The Attendees field offers my `#person` nodes as I type.
- **Steps:** An Attendees field of type Options from supertag (`#person`); type part of a name, Enter.
- **Expected:** The field links to the person node.
- **Source:** https://outliner.tana.inc/learn/features/fields (Options from supertag)
- **Bana:** WORKS.

### UC-03.03 Meeting date from the day page: WORKS
- **Story:** A meeting written on Tuesday's page gets Tuesday as its date.
- **Steps:** A Date field set to auto-initialize to the date of the ancestor day.
- **Expected:** The date fills when the tag is applied.
- **Source:** https://outliner.tana.inc/learn/features/fields (Auto-initialize to date of ancestor day node)
- **Bana:** WORKS.

### UC-03.04 Action items flow to my task list: WORKS
- **Story:** A task typed inside meeting notes shows up with all my other open tasks.
- **Steps:** Under Notes type a line and ` #task`.
- **Expected:** It appears in the Open tasks search.
- **Source:** https://outliner.tana.inc/learn/features/nodes-and-references (overview)
- **Bana:** WORKS.

### UC-03.05 A person's meetings on their page: WORKS
- **Story:** Opening a person shows every meeting they attended.
- **Steps:** Related content on `#person` with a search: `#meeting`, Attendees = PARENT.
- **Expected:** Each person page lists their own meetings.
- **Source:** https://outliner.tana.inc/learn/features/related-content, https://outliner.tana.inc/learn/features/search-nodes (Searches in supertag template)
- **Bana:** WORKS. Related content on `#person` runs for each person page (PARENT is that person).

### UC-03.06 Meeting titles from fields: WORKS
- **Story:** "Catch-up with Mia Chen" built from the title and the Attendees field.
- **Steps:** Supertag, Build title from fields: `${name} with ${Attendees}`.
- **Expected:** The line shows the built title.
- **Source:** https://outliner.tana.inc/learn/features/supertags (Build title from fields)
- **Bana:** WORKS. The `|30…` length and `?` options and `sys:` attributes were not checked.

### UC-03.07 Share the notes by email: WORKS
- **Story:** I copy a meeting and paste it into an email.
- **Steps:** Select the meeting line, Cmd+C, paste elsewhere.
- **Expected:** Plain text (and HTML) with its children.
- **Source:** https://outliner.tana.inc/learn/features/copy-paste-and-export
- **Bana:** WORKS.

### UC-03.08 Today's meetings on the day page: WORKS
- **Story:** My day page lists the meetings dated that day.
- **Source:** https://outliner.tana.inc/learn/features/related-content (Meetings of the day)
- **Bana:** WORKS. Related content on `#day` with `#meeting Date:PARENT` (PARENT is the day's date).

### UC-03.09 Calendar events as the day's agenda: OUT OF SCOPE candidate (cloud)
- **Story:** My Google or Outlook events appear on today's page, ready for notes.
- **Source:** https://outliner.tana.inc/learn/features/calendar-integration
- **Bana:** not built; needs a calendar account connection.

### UC-03.10 Record a meeting without a bot: OUT OF SCOPE candidate (AI)
- **Story:** One click records system audio, then a transcript, summary and action items appear.
- **Source:** https://outliner.tana.inc/learn/features/tana-meeting-notetaker
- **Bana:** not built.

### UC-03.11 Meeting agent: OUT OF SCOPE candidate (AI)
- **Story:** A bot joins the call and files notes, tasks and people into my graph.
- **Source:** https://outliner.tana.inc/learn/features/meeting-agent
- **Bana:** not built.

### UC-03.12 Meeting reminders: OUT OF SCOPE candidate (cloud, desktop)
- **Story:** A desktop notification when a calendar meeting starts, with a record button.
- **Source:** https://outliner.tana.inc/learn/features/tana-for-desktop
- **Bana:** not built (no calendar connection).

### UC-03.13 Notify someone about an action item: MISSING
- **Story:** I notify a teammate on the line that concerns them.
- **Source:** https://outliner.tana.inc/learn/features/command-line (Collaboration: Notify)
- **Bana:** MISSING (one local user).
