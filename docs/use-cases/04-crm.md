# 04 People, companies and a light CRM

Tana pitches `#person`, `#company` and a pipeline built from supertags, fields and views.
Scenarios: `web/tests/use-cases/04-crm.spec.ts`.

### UC-04.01 People linked to their company: WORKS
- **Story:** Each contact points at their company; the company page shows who works there.
- **Steps:** A Company field (options from `#company`) on `#person`; set it.
- **Expected:** The company page's references list the people.
- **Source:** https://outliner.tana.inc/solo-entrepreneurs, https://outliner.tana.inc/learn/features/fields
- **Bana:** WORKS.

### UC-04.02 New company typed in place: WORKS
- **Story:** I type a company that does not exist yet; it becomes a `#company` node.
- **Steps:** Type a new value in the Company field, Enter.
- **Expected:** A new node tagged `#company`, linked from the field.
- **Source:** https://outliner.tana.inc/learn/features/fields (Options from supertag)
- **Bana:** WORKS.

### UC-04.03 People sorted by last contact: WORKS
- **Story:** A table of contacts, the ones I spoke to most recently on top.
- **Steps:** Search `#person`, Table view, sort by the Last contacted date.
- **Expected:** Rows in date order, both directions.
- **Source:** https://outliner.tana.inc/learn/tutorials/networking
- **Bana:** WORKS.

### UC-04.04 Who to reconnect with: WORKS
- **Story:** Show people whose next contact date has passed.
- **Steps:** Search `#person` with Next contact before today.
- **Expected:** Only the overdue people.
- **Source:** https://outliner.tana.inc/learn/tutorials/networking
- **Bana:** WORKS. Tana's template computes the next date from an interval; that needs a formula or command (UC-02.20).

### UC-04.05 A deal pipeline: WORKS
- **Story:** Deals as cards in Lead, Proposal and Won columns.
- **Steps:** `#deal` with a Stage options field; Cards grouped by Stage; add in a column; drag.
- **Expected:** Cards move between stages and the field follows.
- **Source:** https://outliner.tana.inc/learn/tutorials/tana-systems-lab-exploring-crms, https://outliner.tana.inc/learn/features/views
- **Bana:** WORKS.

### UC-04.06 Contact details checked: WORKS
- **Story:** An email field warns me when I paste something that is not an address.
- **Steps:** Type a bad value in an Email field.
- **Expected:** A warning mark; it goes when fixed.
- **Source:** https://outliner.tana.inc/learn/features/fields (Field types, data validation)
- **Bana:** WORKS.

### UC-04.07 Everything about a person: WORKS
- **Story:** Gift ideas, notes and meetings that mention Robin, in one list.
- **Steps:** A search LINKS TO the person.
- **Expected:** Every node pointing at them.
- **Source:** https://outliner.tana.inc/learn/tutorials/thoughtful-gift-tracking, https://outliner.tana.inc/learn/features/search-nodes
- **Bana:** WORKS.

### UC-04.08 Interaction log with @mentions: WORKS
- **Story:** I note "Coffee with @Mia" on today's page and it shows on Mia's page.
- **Steps:** Type @ and the name mid-line, Enter, go on typing.
- **Expected:** An inline link; Mia's references list the line.
- **Source:** https://outliner.tana.inc/learn/features/nodes-and-references (inline references)
- **Bana:** WORKS.

### UC-04.09 Sync from HubSpot or Pipedrive: OUT OF SCOPE candidate (cloud)
- **Source:** https://tana.inc/integrations/hubspot, https://tana.inc/integrations/pipedrive
- **Bana:** not built.

### UC-04.10 Scan a business card: OUT OF SCOPE candidate (mobile)
- **Source:** https://outliner.tana.inc/learn/features/tana-capture
- **Bana:** not built.

### UC-04.11 Merge duplicate contacts: WORKS
- **Story:** I created Robin twice; merge them keeping every link.
- **Steps:** Node menu on the duplicate, Merge into…, pick the one to keep.
- **Source:** https://outliner.tana.inc/learn/features/nodes-and-references (Merging duplicate nodes)
- **Bana:** WORKS (`node.merge`, as UC-06.34): fields, children and every mention move to the kept node.
