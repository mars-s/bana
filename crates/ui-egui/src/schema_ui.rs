//! Supertag and field pages (spec 03 §2, §5, §6): view tabs, "Create new", the configure section
//! (colour, extends, base type, template, checkbox, done mapping, child supertag) and the field
//! definition form.

use egui::{Color32, Rect, RichText, Sense, Stroke, Ui, vec2};
use serde_json::json;
use tanacraft_core::{AutoInit, BASE_TYPES, FieldType, HideMode, NodeId, TAG_COLORS, ViewKind};

use crate::outline::{field_frame, field_label, type_label};
use crate::theme::{self, Tokens};
use crate::{Act, PickMode, TanaApp};

fn card(ui: &mut Ui, t: &Tokens, title: &str, summary: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(t.field_bg).stroke(Stroke::new(1.0, t.line)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(14.5));
            if !summary.is_empty() {
                ui.label(RichText::new(summary).size(12.5).color(t.faint));
            }
            ui.add_space(6.0);
            add(ui);
        },
    );
    ui.add_space(8.0);
}

fn row(ui: &mut Ui, t: &Tokens, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(190.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(190.0);
            ui.label(RichText::new(label).size(13.5).color(t.dim));
        });
        add(ui);
    });
}

/// A supertag's page.
pub fn tag_page(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, tag: NodeId) {
    let Some(def) = app.session.graph.tag_def(tag).cloned() else { return };
    if app.session.config_view {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Configure supertag").size(12.5).color(t.faint));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("Done").size(13.0)).fill(t.field_bg)).clicked() {
                    app.acts.push(Act::Ui("ui.configureSupertag"));
                }
            });
        });
        ui.add_space(6.0);
        configure(app, ui, t, tag);
        return;
    }
    let (strong, _) = theme::tag_color(&def.color, app.dark);
    // Header glow in the tag colour behind the title area.
    let top = ui.min_rect().min;
    for (i, a) in [0.05_f32, 0.035, 0.02].iter().enumerate() {
        let grow = i as f32 * 18.0;
        let glow = Rect::from_min_size(top - vec2(30.0 + grow, 10.0 + grow * 0.5), vec2(ui.available_width() * 0.7 + grow * 2.0, 70.0 + grow));
        ui.painter().rect_filled(glow, 40.0, strong.gamma_multiply(*a));
    }
    ui.horizontal(|ui| {
        let used = app.session.graph.tagged(tag).len();
        ui.label(RichText::new(format!("Supertag · used {used} times")).size(12.5).color(t.faint));
        if let Some(b) = &def.base_type {
            ui.label(RichText::new(format!("· {b} (base type)")).size(12.5).color(t.faint));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(RichText::new("Configure").size(13.0)).fill(t.field_bg)).clicked() {
                app.acts.push(Act::Ui("ui.configureSupertag"));
            }
            if ui
                .add(egui::Button::new(RichText::new("+ Create new").size(13.0)).fill(t.field_bg))
                .on_hover_text("A new node with this supertag on today's page")
                .clicked()
            {
                app.acts.push(Act::Run("view.addItem".into(), json!({"node": tag.0, "name": ""})));
            }
        });
    });
    ui.add_space(8.0);

    // View tabs: everything tagged, then the supertag's own views, then "+ Add view".
    let tabs: Vec<NodeId> = std::iter::once(tag).chain(def.views.iter().copied().filter(|v| app.session.graph.get(*v).is_some())).collect();
    let active = app.tag_tab.get(&tag).copied().filter(|v| tabs.contains(v)).unwrap_or(tag);
    let mut pick = None;
    egui::Frame::new().fill(t.field_bg).corner_radius(8.0).inner_margin(4.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            for v in &tabs {
                let label =
                    if *v == tag { format!("Everything tagged #{}", app.session.graph.name(tag)) } else { app.session.graph.name(*v).to_string() };
                let on = *v == active;
                let b = egui::Button::new(RichText::new(label).size(13.0).color(if on { t.text } else { t.dim }))
                    .fill(if on { t.hover } else { Color32::TRANSPARENT })
                    .corner_radius(6.0);
                if ui.add(b).clicked() {
                    pick = Some(*v);
                }
            }
            ui.menu_button(RichText::new("+ Add view").size(13.0).color(t.dim), |ui| {
                ui.label(RichText::new(format!("View everything tagged #{} as a:", app.session.graph.name(tag))).size(12.0).color(t.faint));
                for k in [ViewKind::Outline, ViewKind::Table, ViewKind::Cards, ViewKind::Calendar] {
                    if ui.button(k.label()).clicked() {
                        app.acts.push(Act::Run("tag.addView".into(), json!({"tag": tag.0, "kind": k.name()})));
                        ui.close();
                    }
                }
            });
        });
    });
    if let Some(p) = pick {
        app.tag_tab.insert(tag, p);
    }
    ui.add_space(8.0);
    crate::views::toolbar(app, ui, t, active);
    crate::views::render(app, ui, t, active);
}

/// A collapsible section card: title and summary always; the body while open (spec 03 §5).
fn section(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, key: &str, title: &str, summary: &str, add: impl FnOnce(&mut TanaApp, &mut Ui)) {
    let open = !app.config_closed.contains(key);
    egui::Frame::new().fill(t.field_bg).stroke(Stroke::new(1.0, t.line)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(14, 10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            let head = ui
                .horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).strong().size(14.5));
                        if !summary.is_empty() {
                            ui.label(RichText::new(summary).size(12.5).color(t.faint));
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(if open { "v" } else { ">" }).color(t.faint));
                    });
                })
                .response;
            if ui.interact(head.rect, egui::Id::new(("cfg-section", key)), Sense::click()).clicked() {
                if open {
                    app.config_closed.insert(key.to_string());
                } else {
                    app.config_closed.remove(key);
                }
            }
            if open {
                ui.add_space(8.0);
                add(app, ui);
            }
        },
    );
    ui.add_space(8.0);
}

/// A supertag's configuration, shown in its own panel (spec 03 §5).
fn configure(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, tag: NodeId) {
    let Some(def) = app.session.graph.tag_def(tag).cloned() else { return };
    let mut acts = Vec::new();
    // Colour row: no colour, the hues, and a custom colour.
    ui.horizontal(|ui| {
        for c in TAG_COLORS {
            let (strong, _) = theme::tag_color(c, app.dark);
            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            if c == "none" {
                ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(1.5, t.dim));
                ui.painter().line_segment([r.center() + vec2(-6.0, 6.0), r.center() + vec2(6.0, -6.0)], Stroke::new(1.5, t.dim));
            } else {
                ui.painter().circle_filled(r.center(), 10.0, strong);
            }
            if def.color == c || (c == "none" && def.color.is_empty()) {
                ui.painter().circle_stroke(r.center(), 12.5, Stroke::new(2.0, t.text));
            }
            if resp.on_hover_text(if c == "none" { "No colour" } else { c }).clicked() {
                acts.push(Act::Run("tag.setColor".into(), json!({"tag": tag.0, "color": c})));
            }
        }
        // Custom colour: any RGB.
        let custom = tanacraft_core::is_custom_color(&def.color);
        let mut rgb = theme::tag_color(&def.color, app.dark).0.to_array();
        let r = ui.color_edit_button_srgb((&mut rgb[..3]).try_into().unwrap_or(&mut [128, 128, 128])).on_hover_text("Custom colour");
        if custom {
            ui.painter().circle_stroke(r.rect.center(), 12.5, Stroke::new(2.0, t.text));
        }
        if r.changed() {
            acts.push(Act::Run("tag.setColor".into(), json!({"tag": tag.0, "color": format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])})));
        }
    });
    ui.add_space(10.0);
    let g = &app.session.graph;
    let extends: Vec<String> = def.extends.iter().map(|e| format!("#{}", g.name(*e))).collect();
    let summary = format!(
        "{}{}",
        def.base_type.clone().map(|b| format!("{b} (base type)")).unwrap_or("No base type".into()),
        if extends.is_empty() { String::new() } else { format!(", extends {}", extends.join(", ")) }
    );
    section(app, ui, t, "blocks", "Building blocks", &summary, |app, ui| {
        let g = &app.session.graph;
        row(ui, t, "Base type", |ui| {
            egui::ComboBox::from_id_salt(("base", tag.0)).selected_text(def.base_type.clone().unwrap_or_else(|| "None".into())).show_ui(ui, |ui| {
                if ui.selectable_label(def.base_type.is_none(), "None").clicked() {
                    acts.push(Act::Run("tag.setBaseType".into(), json!({"tag": tag.0, "base": "none"})));
                }
                for b in BASE_TYPES {
                    if ui.selectable_label(def.base_type.as_deref() == Some(b), b).clicked() {
                        acts.push(Act::Run("tag.setBaseType".into(), json!({"tag": tag.0, "base": b})));
                    }
                }
            });
        });
        row(ui, t, "Extend other supertags", |ui| {
            for e in &def.extends {
                crate::outline::chip(ui, g, *e, app.dark);
            }
            if ui.button("Extend").clicked() {
                acts.push(Act::Pick(PickMode::Extend, tag, ui.min_rect().left_bottom()));
            }
        });
    });
    section(app, ui, t, "template", "Content template", "Default content, optional fields, show checkbox", |app, ui| {
        let g = &app.session.graph;
        ui.label(RichText::new("Default content").size(13.0).color(t.dim));
        egui::Frame::new().stroke(Stroke::new(1.0, t.line)).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for f in g.template_fields(tag) {
                let inherited = !def.fields.contains(&f);
                let fd = g.field_def(f).cloned().unwrap_or_default();
                field_frame(ui, t, |ui| {
                    let r = ui.scope(|ui| field_label(ui, t, g.name(f), fd.field_type)).response;
                    if ui.interact(r.rect, egui::Id::new(("cfg-field", f.0)), Sense::click()).on_hover_text("Open field").clicked() {
                        acts.push(Act::Run("nav.open".into(), json!({"node": f.0})));
                    }
                    let default = fd.default.as_ref().map(|v| tanacraft_core::export::display_value(g, v)).unwrap_or_else(|| "Default value".into());
                    ui.label(RichText::new(default).size(13.5).color(if fd.default.is_some() { t.text } else { t.faint }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut pinned = def.pinned_fields.contains(&f);
                        if ui.checkbox(&mut pinned, "").on_hover_text("Toggle pinned field").changed() {
                            acts.push(Act::Run("tag.setPinnedField".into(), json!({"tag": tag.0, "field": f.0, "on": pinned})));
                        }
                        if inherited {
                            ui.label(RichText::new("inherited").size(12.0).color(t.faint));
                        } else if ui.small_button("x").on_hover_text("Remove from template").clicked() {
                            acts.push(Act::Run("tag.removeField".into(), json!({"tag": tag.0, "field": f.0})));
                        }
                    });
                });
            }
            for n in &def.template {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 22.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.0, t.dim);
                    ui.label(g.name(*n));
                });
            }
            let r = ui.add(egui::TextEdit::singleline(&mut app.template_buf).hint_text("Add a template node, Enter").desired_width(260.0));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.template_buf.trim().is_empty() {
                let name = std::mem::take(&mut app.template_buf);
                acts.push(Act::Run("tag.addTemplateNode".into(), json!({"tag": tag.0, "name": name})));
            }
            ui.horizontal(|ui| {
                if ui.button("New field").clicked() || ui.button("Insert existing field").clicked() {
                    acts.push(Act::Pick(PickMode::TemplateField, tag, ui.min_rect().left_bottom()));
                }
            });
        });
        ui.add_space(8.0);
        let g = &app.session.graph;
        ui.label(RichText::new("Optional fields").size(13.0).color(t.dim));
        egui::Frame::new().stroke(Stroke::new(1.0, t.line)).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if def.optional.is_empty() {
                ui.label(RichText::new("Fields offered on tagged nodes without being added up front.").size(12.5).color(t.faint));
            }
            for f in &def.optional {
                ui.horizontal(|ui| {
                    ui.label(g.name(*f));
                    if ui.small_button("x").on_hover_text("Remove optional field").clicked() {
                        acts.push(Act::Run("tag.setOptionalField".into(), json!({"tag": tag.0, "field": f.0, "on": false})));
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui.button("New field").clicked() || ui.button("Insert existing field").clicked() {
                    acts.push(Act::Pick(PickMode::OptionalField, tag, ui.min_rect().left_bottom()));
                }
            });
        });
        ui.add_space(8.0);
        row(ui, t, "Show checkbox", |ui| {
            let mut on = def.checkbox;
            if ui.checkbox(&mut on, "").changed() {
                acts.push(Act::Run("tag.setCheckbox".into(), json!({"tag": tag.0, "on": on})));
            }
        });
        let options_fields: Vec<NodeId> =
            g.template_fields(tag).into_iter().filter(|f| g.field_def(*f).is_some_and(|d| d.field_type == FieldType::Options)).collect();
        row(ui, t, "Map checkbox to field", |ui| {
            let current = def.done_map.as_ref().map(|m| g.name(m.field).to_string()).unwrap_or_else(|| "Off".into());
            egui::ComboBox::from_id_salt(("donemap", tag.0)).selected_text(current).show_ui(ui, |ui| {
                if ui.selectable_label(def.done_map.is_none(), "Off").clicked() {
                    acts.push(Act::Run("tag.setDoneMap".into(), json!({"tag": tag.0})));
                }
                for f in &options_fields {
                    let opts = g.field_def(*f).map(|d| d.options.clone()).unwrap_or_default();
                    if ui.button(g.name(*f)).clicked() {
                        let checked =
                            opts.iter().find(|o| o.eq_ignore_ascii_case("done")).cloned().or_else(|| opts.last().cloned()).unwrap_or_default();
                        let unchecked = opts.first().cloned().unwrap_or_default();
                        acts.push(Act::Run("tag.setDoneMap".into(), json!({"tag": tag.0, "field": f.0, "checked": checked, "unchecked": unchecked})));
                    }
                }
            });
        });
        if let Some(m) = &def.done_map {
            let opts = g.field_def(m.field).map(|d| d.options.clone()).unwrap_or_default();
            for (label, current, key) in [("Map checked to", &m.checked, "checked"), ("Map unchecked to", &m.unchecked, "unchecked")] {
                row(ui, t, label, |ui| {
                    ui.label(RichText::new(g.name(m.field)).size(13.0).color(t.dim));
                    egui::ComboBox::from_id_salt(("donemap", key, tag.0)).selected_text(current.clone()).show_ui(ui, |ui| {
                        for o in &opts {
                            if ui.selectable_label(o == current, o).clicked() {
                                let mut p = json!({"tag": tag.0, "field": m.field.0, "checked": m.checked, "unchecked": m.unchecked});
                                p[key] = json!(o);
                                acts.push(Act::Run("tag.setDoneMap".into(), p));
                            }
                        }
                    });
                });
            }
        }
    });
    section(app, ui, t, "advanced", "Advanced options", "Title expression, default child supertag, related content, shortcuts", |app, ui| {
        let g = &app.session.graph;
        row(ui, t, "Build title from fields", |ui| {
            let id = egui::Id::new(("title-expr", tag.0));
            let mut buf = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| def.title_expr.clone().unwrap_or_default());
            let r =
                ui.add(egui::TextEdit::singleline(&mut buf).font(egui::TextStyle::Monospace).hint_text("${name} (${Field})").desired_width(300.0));
            if r.lost_focus() {
                acts.push(Act::Run("tag.setTitleExpression".into(), json!({"tag": tag.0, "expression": buf})));
                ui.data_mut(|d| d.remove::<String>(id));
            } else if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(id, buf));
            }
        });
        row(ui, t, "Default child supertag", |ui| {
            let current = def.child_tag.map(|c| format!("#{}", g.name(c))).unwrap_or_else(|| "Select supertag for children".into());
            egui::ComboBox::from_id_salt(("child", tag.0)).selected_text(current).show_ui(ui, |ui| {
                if ui.selectable_label(def.child_tag.is_none(), "None").clicked() {
                    acts.push(Act::Run("tag.setChildTag".into(), json!({"tag": tag.0, "child": "none"})));
                }
                for other in g.tags() {
                    if other != g.sys.day_tag && ui.selectable_label(def.child_tag == Some(other), format!("#{}", g.name(other))).clicked() {
                        acts.push(Act::Run("tag.setChildTag".into(), json!({"tag": tag.0, "child": other.0})));
                    }
                }
            });
        });
        row(ui, t, "Related content", |ui| {
            ui.vertical(|ui| {
                for r in def.related.iter().filter(|r| g.get(**r).is_some()) {
                    ui.horizontal(|ui| {
                        ui.label(g.name(*r));
                        if ui.small_button("x").on_hover_text("Remove related content").clicked() {
                            acts.push(Act::Run("tag.removeRelated".into(), json!({"tag": tag.0, "node": r.0})));
                        }
                    });
                }
                let id = egui::Id::new(("related-buf", tag.0));
                let mut buf = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
                let r = ui.add(egui::TextEdit::singleline(&mut buf).hint_text("Add a search (e.g. #task AND TODO), Enter").desired_width(300.0));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !buf.trim().is_empty() {
                    acts.push(Act::Run("tag.addRelated".into(), json!({"tag": tag.0, "query": buf})));
                    buf.clear();
                }
                ui.data_mut(|d| d.insert_temp(id, buf));
            });
        });
        row(ui, t, "Shortcuts", |ui| {
            let views: Vec<String> = def.views.iter().filter(|v| g.get(**v).is_some()).map(|v| g.name(*v).to_string()).collect();
            ui.label(
                RichText::new(if views.is_empty() { "The tag page's view tabs (add with + Add view)".into() } else { views.join(", ") })
                    .size(13.0)
                    .color(t.dim),
            );
        });
    });
    let g = &app.session.graph;
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Used {} times", g.tagged(tag).len())).size(12.5).color(t.faint));
        if ui.add(egui::Button::new(RichText::new("Delete").color(Color32::from_rgb(232, 110, 96))).fill(t.field_bg)).clicked() {
            acts.push(Act::Run("node.delete".into(), json!({"node": tag.0})));
        }
    });
    ui.add_space(10.0);
    app.acts.extend(acts);
}

/// A field definition's page: type, options, range, auto-initialize, required, hide, usage.
pub fn field_page(app: &mut TanaApp, ui: &mut Ui, t: &Tokens, field: NodeId) {
    let g = &app.session.graph;
    let Some(def) = g.field_def(field).cloned() else { return };
    let mut acts = Vec::new();
    let f = json!(field.0);
    card(ui, t, "Field", "", |ui| {
        row(ui, t, "Field type", |ui| {
            egui::ComboBox::from_id_salt(("ftype", field.0)).selected_text(type_label(def.field_type)).show_ui(ui, |ui| {
                for ty in FieldType::ALL {
                    if ui.selectable_label(def.field_type == ty, type_label(ty)).clicked() {
                        acts.push(Act::Run("field.setType".into(), json!({"field": f, "type": ty.name()})));
                    }
                }
            });
        });
        match def.field_type {
            FieldType::Options => {
                row(ui, t, "Options", |ui| {
                    ui.vertical(|ui| {
                        for o in &def.options {
                            ui.horizontal(|ui| {
                                ui.label(o);
                                if ui.small_button("x").clicked() {
                                    acts.push(Act::Run("field.removeOption".into(), json!({"field": f, "option": o})));
                                }
                            });
                        }
                        let r = ui.add(egui::TextEdit::singleline(&mut app.option_buf).hint_text("Add an option, Enter").desired_width(220.0));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !app.option_buf.trim().is_empty() {
                            let o = std::mem::take(&mut app.option_buf);
                            acts.push(Act::Run("field.addOption".into(), json!({"field": f, "option": o})));
                        }
                    });
                });
            }
            FieldType::OptionsFromTag => {
                row(ui, t, "Supertag", |ui| {
                    let current = def.source_tag.map(|x| format!("#{}", g.name(x))).unwrap_or_else(|| "Select supertag".into());
                    egui::ComboBox::from_id_salt(("fsrc", field.0)).selected_text(current).show_ui(ui, |ui| {
                        for tag in g.tags() {
                            if ui.button(format!("#{}", g.name(tag))).clicked() {
                                acts.push(Act::Run("field.setSourceTag".into(), json!({"field": f, "tag": tag.0})));
                            }
                        }
                    });
                });
            }
            FieldType::Number => {
                row(ui, t, "Minimum / maximum", |ui| {
                    let (mut lo, mut hi) = app
                        .range_buf
                        .clone()
                        .unwrap_or_else(|| (def.min.map(|x| x.to_string()).unwrap_or_default(), def.max.map(|x| x.to_string()).unwrap_or_default()));
                    let a = ui.add(egui::TextEdit::singleline(&mut lo).hint_text("min").desired_width(70.0));
                    let b = ui.add(egui::TextEdit::singleline(&mut hi).hint_text("max").desired_width(70.0));
                    if a.lost_focus() || b.lost_focus() {
                        let mut p = json!({"field": f});
                        if let Ok(x) = lo.trim().parse::<f64>() {
                            p["min"] = json!(x);
                        }
                        if let Ok(x) = hi.trim().parse::<f64>() {
                            p["max"] = json!(x);
                        }
                        acts.push(Act::Run("field.setRange".into(), p));
                        app.range_buf = None;
                    } else if a.has_focus() || b.has_focus() {
                        app.range_buf = Some((lo, hi));
                    }
                });
            }
            _ => {}
        }
        row(ui, t, "Auto-initialize", |ui| {
            let label = match def.auto_init {
                None => "Off",
                Some(AutoInit::CurrentDate) => "To current date",
                Some(AutoInit::AncestorDay) => "To date of ancestor day node",
                Some(AutoInit::AncestorValue) => "To value from ancestor with this field",
            };
            egui::ComboBox::from_id_salt(("finit", field.0)).selected_text(label).show_ui(ui, |ui| {
                for (l, m) in [
                    ("Off", "none"),
                    ("To current date", "current_date"),
                    ("To date of ancestor day node", "ancestor_day"),
                    ("To value from ancestor with this field", "ancestor_value"),
                ] {
                    if ui.button(l).clicked() {
                        acts.push(Act::Run("field.setAutoInit".into(), json!({"field": f, "mode": m})));
                    }
                }
            });
        });
        if def.field_type != FieldType::Checkbox {
            row(ui, t, "Required", |ui| {
                let mut on = def.required;
                if ui.checkbox(&mut on, "").changed() {
                    acts.push(Act::Run("field.setRequired".into(), json!({"field": f, "on": on})));
                }
            });
        }
        row(ui, t, "Hide field", |ui| {
            egui::ComboBox::from_id_salt(("fhide", field.0)).selected_text(def.hide.label()).show_ui(ui, |ui| {
                for h in HideMode::ALL {
                    if ui.selectable_label(def.hide == h, h.label()).clicked() {
                        acts.push(Act::Run("field.setHide".into(), json!({"field": f, "mode": h.name()})));
                    }
                }
            });
        });
        let (nodes, tags) = g.field_usage(field);
        row(ui, t, "Used in", |ui| {
            ui.label(format!("{nodes} nodes, {tags} supertag(s)"));
        });
    });
    app.acts.extend(acts);
}
