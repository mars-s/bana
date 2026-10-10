# 05 Research and reading

Books, articles, quotes and concepts linked into a graph. Scenarios:
`web/tests/use-cases/05-research-reading.spec.ts`.

### UC-05.01 Save a link: PARTIAL
- **Story:** I paste a URL to read later, and want a preview (a player for videos).
- **Steps:** Paste a link on its own line.
- **Expected:** A preview card; YouTube, Vimeo, Loom, Spotify, Figma and X links embed.
- **Source:** https://outliner.tana.inc/learn/features/node-types (Node with embedded links)
- **TanaCraft:** PARTIAL. A preview card with the site name (Settings, Link previews); no embedded players.

### UC-05.02 Quotes inherit the book's author: WORKS
- **Story:** Quotes nested under a book take its Author without retyping.
- **Steps:** An Author field set to auto-initialize from the ancestor with this field; tag a child `#quote`.
- **Expected:** The quote's Author is the book's.
- **Source:** https://outliner.tana.inc/learn/features/fields (Auto-initialize to value from ancestor)
- **TanaCraft:** WORKS.

### UC-05.03 Highlight and bold a phrase: WORKS
- **Story:** I mark the key words in a note.
- **Steps:** Select words, Ctrl+H (highlight), Cmd+B (bold).
- **Expected:** Formatting shows and is saved.
- **Source:** https://outliner.tana.inc/learn/features/nodes-and-references (Formatting text)
- **TanaCraft:** WORKS.

### UC-05.04 Link a new concept inline: WORKS
- **Story:** Mid-sentence I link to a concept page that does not exist yet.
- **Steps:** Type @ and the name, Cmd+Enter to create it.
- **Expected:** An inline link; the new node sits in the Library; its references show the note.
- **Source:** https://outliner.tana.inc/learn/features/nodes-and-references (Option 1c)
- **TanaCraft:** WORKS.

### UC-05.05 Reading list of unread items: WORKS
- **Story:** A table of articles showing only the unread ones.
- **Steps:** `#article` with a Read checkbox field; table view; filter NOT Read = Yes.
- **Expected:** Read items drop out.
- **Source:** https://outliner.tana.inc/learn/features/views (Filter)
- **TanaCraft:** WORKS.

### UC-05.06 Unlinked mentions: MISSING
- **Story:** Find notes that name this concept without linking it, and link them in one click.
- **Source:** https://outliner.tana.inc/learn/features/nodes-and-references (Reference section)
- **TanaCraft:** MISSING.

### UC-05.07 Readwise sync: MISSING
- **Story:** Highlights from Kindle and Reader arrive as tagged nodes.
- **Source:** https://outliner.tana.inc/learn/features/readwise-integration
- **TanaCraft:** MISSING (needs a Readwise token; could run locally).

### UC-05.08 Attach a PDF or an image: MISSING
- **Story:** I drop a paper's PDF on its node and preview it.
- **Source:** https://outliner.tana.inc/learn/features/nodes-and-references (Storing files on nodes)
- **TanaCraft:** MISSING (no file store).

### UC-05.09 Peek into an inline reference: MISSING
- **Story:** Shift+click an inline link to read its content in place.
- **Source:** https://outliner.tana.inc/learn/features/nodes-and-references (Edit an inline reference)
- **TanaCraft:** MISSING (a click opens it).

### UC-05.10 Lecture transcription: OUT OF SCOPE candidate (AI)
- **Source:** https://outliner.tana.inc/learn/tutorials/turn-lectures-into-lasting-knowledge, https://outliner.tana.inc/students
- **TanaCraft:** not built.
