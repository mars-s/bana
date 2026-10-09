//! The built-in sample graph (our own text), used by `--sample`, screenshots and tests.

use crate::Result;
use crate::date::Date;
use crate::graph::Graph;
use crate::model::{FieldType, FieldValue};

/// A small garden-planning workspace with supertags, fields, references and a day node for `today`.
pub fn sample(today: Date) -> Result<Graph> {
    let mut g = Graph::new();
    let home = g.sys.library;

    // Schema.
    let status = g.create_field("Status", FieldType::Options)?;
    g.field_def_mut(status)?.options = vec!["Not started".into(), "In progress".into(), "Done".into()];
    g.field_def_mut(status)?.default = Some(FieldValue::Choice("Not started".into()));
    let due = g.create_field("Due", FieldType::Date)?;
    let effort = g.create_field("Effort (hours)", FieldType::Number)?;
    let email = g.create_field("Email", FieldType::Email)?;
    let site = g.create_field("Website", FieldType::Url)?;
    let outdoors = g.create_field("Outdoors", FieldType::Checkbox)?;

    let item = g.create_tag("item", "grey")?;
    let task = g.create_tag("task", "pink")?;
    g.tag_extend(task, item)?;
    g.set_tag_checkbox(task, true)?;
    g.tag_add_field(task, status)?;
    g.tag_add_field(task, due)?;
    g.tag_add_field(task, effort)?;
    let person = g.create_tag("person", "green")?;
    g.tag_add_field(person, email)?;
    let project = g.create_tag("project", "blue")?;
    g.tag_add_field(project, status)?;
    g.tag_add_field(project, site)?;
    let meeting = g.create_tag("meeting", "violet")?;
    g.tag_add_field(meeting, outdoors)?;
    g.tag_set_done_map(task, Some(crate::model::DoneMap { field: status, checked: "Done".into(), unchecked: "Not started".into() }))?;
    g.tag_set_base_type(task, Some("task"))?;
    g.tag_add_template_node(meeting, "Agenda")?;
    g.tag_add_template_node(meeting, "Notes")?;
    let owner = g.create_field("Owner", FieldType::OptionsFromTag)?;
    g.field_def_mut(owner)?.source_tag = Some(person);
    g.tag_add_field(project, owner)?;

    // Home.
    let garden = g.create(home, None, "Community garden")?;
    g.add_tag(garden, project)?;
    g.set_field_text(garden, status, "In progress")?;
    g.set_field_text(garden, site, "https://example.org/garden")?;
    g.set_field_text(garden, owner, "Robin Ash")?;
    g.set_field_text_at(garden, owner, 1, "Mia Chen")?;
    let beds = g.create(garden, None, "Raised beds")?;
    for (name, done, effort_h, status_v, due_in) in [
        ("Order cedar boards", true, "2", "Done", -3),
        ("Build four beds", false, "6", "In progress", 5),
        ("Fill with compost", false, "3", "Not started", 12),
    ] {
        let t = g.create(beds, None, name)?;
        g.add_tag(t, task)?;
        g.set_done(t, Some(done))?;
        g.set_field_text(t, effort, effort_h)?;
        g.set_field_text(t, status, status_v)?;
        g.set_field_text(t, due, &today.add_days(due_in).to_string())?;
    }
    let seeds = g.create(garden, None, "Seed list")?;
    for s in ["Tomatoes: two heirloom kinds", "Runner beans", "Marigolds along the edges"] {
        g.create(seeds, None, s)?;
    }
    let water = g.create(garden, None, "Water rota")?;
    g.create(water, None, "Weekdays: whoever opens the gate")?;
    g.create(water, None, "Weekends: sign-up sheet by the shed")?;
    g.set_expanded(garden, true);
    g.set_expanded(beds, true);

    let reading = g.create(home, None, "Reading notes")?;
    g.create(reading, None, "Soil is alive: feed it, do not just water it")?;
    g.create(reading, None, "Mulch keeps **moisture** in and ==weeds== out, *most* of the time")?;
    g.set_expanded(reading, true);
    let ideas = g.create(home, None, "Ideas")?;
    g.create(ideas, None, "A tool library in the old shed")?;

    let robin = g.tagged(person).into_iter().next();
    if let Some(r) = robin {
        g.set_field_text(r, email, "robin@example.org")?;
    }

    // Today.
    let day = g.day(today)?;
    g.create(day, None, "Seed swap at the library, bring spare bean seeds")?;
    let m = g.create(day, None, "Volunteer meetup")?;
    g.add_tag(m, meeting)?;
    g.set_field_text(m, outdoors, "yes")?;
    g.create(m, None, "Agree on the water rota")?;
    g.create(m, None, "Pick a date for the build day")?;
    g.set_expanded(m, true);
    let t = g.create(day, None, "Ask the hardware store about bulk compost")?;
    g.add_tag(t, task)?;
    g.set_field_text(t, due, &today.add_days(2).to_string())?;
    g.create_reference(day, None, garden)?;
    if let Some(r) = robin {
        g.create(day, None, &format!("Ask {} for the shed key, map at https://example.org/shed", crate::format::ref_token(r.0)))?;
    }

    // Supertag view tabs and pins.
    let board = g.tag_add_view(task, crate::view::ViewKind::Cards)?;
    g.view_mut(board)?.group = Some(crate::view::GroupBy::Field(status));
    g.set_name(board, "Task board")?;
    let cal = g.tag_add_view(task, crate::view::ViewKind::Calendar)?;
    g.set_name(cal, "Task calendar")?;
    for p in [task, meeting, project, garden] {
        g.set_pinned(p, true)?;
    }

    // A live search with a table view.
    let q = crate::query::Query::parse(&g, "#task AND TODO")?;
    let open = g.create_search(home, Some(1), "Open tasks", q)?;
    let v = g.view_mut(open)?;
    v.kind = crate::view::ViewKind::Table;
    v.sort = vec![crate::view::Sort { by: crate::view::SortBy::Field(due), desc: false }];
    // Day pages show open tasks beside the notes (related content of the day supertag).
    let open_q = crate::query::Query::parse(&g, "#task AND TODO")?;
    let day_tag = g.sys.day_tag;
    g.tag_add_related(day_tag, "Open tasks", open_q)?;
    Ok(g)
}
