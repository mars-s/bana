//! Small icons drawn in code (original work, no image files).

use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Icon {
    Today,
    Tag,
    Home,
    Calendar,
    Trash,
    Search,
    Plus,
    Back,
    Forward,
    Sidebar,
    Keyboard,
    Recent,
    Help,
    Person,
    Pin,
    Building,
    /// Content width: a bar with arrows at both ends.
    Width,
    /// View toolbar: display fields (lines with an eye), group (rows), sort (arrows), filter
    /// (funnel lines), view as (grid).
    Display,
    Group,
    Sort,
    Filter,
    Views,
}

pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.5;
    let st = Stroke::new(1.4, color);
    let r = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    match icon {
        Icon::Today | Icon::Calendar => {
            let b = Rect::from_min_max(r(-0.75, -0.65), r(0.75, 0.8));
            p.rect_stroke(b, 2.0, st, StrokeKind::Middle);
            p.line_segment([r(-0.75, -0.2), r(0.75, -0.2)], st);
            p.line_segment([r(-0.35, -0.9), r(-0.35, -0.5)], st);
            p.line_segment([r(0.35, -0.9), r(0.35, -0.5)], st);
            if icon == Icon::Today {
                p.rect_filled(Rect::from_min_max(r(-0.1, 0.15), r(0.4, 0.55)), 1.0, color);
            }
        }
        Icon::Tag => hash(p, rect, color),
        Icon::Home => {
            p.line_segment([r(-0.85, 0.0), r(0.0, -0.8)], st);
            p.line_segment([r(0.0, -0.8), r(0.85, 0.0)], st);
            p.line_segment([r(-0.6, -0.2), r(-0.6, 0.8)], st);
            p.line_segment([r(0.6, -0.2), r(0.6, 0.8)], st);
            p.line_segment([r(-0.6, 0.8), r(0.6, 0.8)], st);
        }
        Icon::Trash => {
            p.line_segment([r(-0.8, -0.55), r(0.8, -0.55)], st);
            p.line_segment([r(-0.25, -0.85), r(0.25, -0.85)], st);
            p.line_segment([r(-0.6, -0.55), r(-0.45, 0.85)], st);
            p.line_segment([r(0.6, -0.55), r(0.45, 0.85)], st);
            p.line_segment([r(-0.45, 0.85), r(0.45, 0.85)], st);
        }
        Icon::Search => {
            p.circle_stroke(r(-0.15, -0.15), s * 0.55, st);
            p.line_segment([r(0.25, 0.25), r(0.8, 0.8)], st);
        }
        Icon::Plus => {
            p.line_segment([r(-0.7, 0.0), r(0.7, 0.0)], st);
            p.line_segment([r(0.0, -0.7), r(0.0, 0.7)], st);
        }
        Icon::Back | Icon::Forward => {
            let d = if icon == Icon::Back { 1.0 } else { -1.0 };
            p.line_segment([r(0.7 * d, 0.0), r(-0.7 * d, 0.0)], st);
            p.line_segment([r(-0.7 * d, 0.0), r(-0.2 * d, -0.5)], st);
            p.line_segment([r(-0.7 * d, 0.0), r(-0.2 * d, 0.5)], st);
        }
        Icon::Sidebar => {
            let b = Rect::from_min_max(r(-0.85, -0.7), r(0.85, 0.7));
            p.rect_stroke(b, 2.0, st, StrokeKind::Middle);
            p.line_segment([r(-0.3, -0.7), r(-0.3, 0.7)], st);
        }
        Icon::Keyboard => {
            let b = Rect::from_min_max(r(-0.9, -0.55), r(0.9, 0.55));
            p.rect_stroke(b, 2.0, st, StrokeKind::Middle);
            for i in 0..4 {
                let x = -0.55 + i as f32 * 0.37;
                p.circle_filled(r(x, -0.15), 0.9, color);
            }
            p.line_segment([r(-0.45, 0.25), r(0.45, 0.25)], st);
        }
        Icon::Display => {
            for i in 0..3 {
                let y = -0.6 + i as f32 * 0.45;
                p.line_segment([r(-0.85, y), r(0.1, y)], st);
            }
            p.circle_stroke(r(0.5, 0.35), s * 0.28, st);
            p.circle_filled(r(0.5, 0.35), 1.2, color);
        }
        Icon::Group => {
            p.rect_stroke(Rect::from_min_max(r(-0.85, -0.75), r(0.85, -0.15)), 1.5, st, StrokeKind::Middle);
            p.rect_stroke(Rect::from_min_max(r(-0.85, 0.15), r(0.85, 0.75)), 1.5, st, StrokeKind::Middle);
        }
        Icon::Sort => {
            p.line_segment([r(-0.4, -0.75), r(-0.4, 0.75)], st);
            p.line_segment([r(-0.4, -0.75), r(-0.75, -0.35)], st);
            p.line_segment([r(-0.4, -0.75), r(-0.05, -0.35)], st);
            p.line_segment([r(0.4, 0.75), r(0.4, -0.75)], st);
            p.line_segment([r(0.4, 0.75), r(0.05, 0.35)], st);
            p.line_segment([r(0.4, 0.75), r(0.75, 0.35)], st);
        }
        Icon::Filter => {
            p.line_segment([r(-0.85, -0.55), r(0.85, -0.55)], st);
            p.line_segment([r(-0.5, 0.0), r(0.5, 0.0)], st);
            p.line_segment([r(-0.2, 0.55), r(0.2, 0.55)], st);
        }
        Icon::Views => {
            for (x, y) in [(-0.8, -0.8), (0.1, -0.8), (-0.8, 0.1), (0.1, 0.1)] {
                p.rect_stroke(Rect::from_min_max(r(x, y), r(x + 0.7, y + 0.7)), 1.0, st, StrokeKind::Middle);
            }
        }
        Icon::Width => {
            p.line_segment([r(-0.9, 0.0), r(0.9, 0.0)], st);
            for d in [-1.0, 1.0] {
                p.line_segment([r(0.9 * d, 0.0), r(0.5 * d, -0.35)], st);
                p.line_segment([r(0.9 * d, 0.0), r(0.5 * d, 0.35)], st);
            }
            p.line_segment([r(-0.9, -0.7), r(-0.9, 0.7)], st);
            p.line_segment([r(0.9, -0.7), r(0.9, 0.7)], st);
        }
        Icon::Pin => {
            p.circle_stroke(r(0.0, -0.35), s * 0.4, st);
            p.line_segment([r(0.0, 0.05), r(0.0, 0.9)], st);
        }
        Icon::Building => {
            p.rect_stroke(Rect::from_min_max(r(-0.6, -0.85), r(0.6, 0.85)), 1.0, st, StrokeKind::Middle);
            for y in [-0.45, 0.0, 0.45] {
                p.line_segment([r(-0.3, y), r(-0.1, y)], st);
                p.line_segment([r(0.1, y), r(0.3, y)], st);
            }
        }
        Icon::Help => {
            p.circle_stroke(c, s * 0.85, st);
            p.text(c, egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(s * 1.3), color);
        }
        Icon::Person => {
            p.circle_stroke(r(0.0, -0.35), s * 0.35, st);
            p.line_segment([r(-0.7, 0.8), r(-0.4, 0.25)], st);
            p.line_segment([r(-0.4, 0.25), r(0.4, 0.25)], st);
            p.line_segment([r(0.4, 0.25), r(0.7, 0.8)], st);
        }
        Icon::Recent => {
            p.circle_stroke(c, s * 0.8, st);
            p.line_segment([c, r(0.0, -0.5)], st);
            p.line_segment([c, r(0.35, 0.2)], st);
        }
    }
}

/// A `#` drawn with lines (crisp at any size).
pub fn hash(p: &Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.42;
    let st = Stroke::new(1.3, color);
    let at = |x: f32, y: f32| -> Pos2 { c + vec2(x * s, y * s) };
    p.line_segment([at(-0.3, -0.9), at(-0.5, 0.9)], st);
    p.line_segment([at(0.5, -0.9), at(0.3, 0.9)], st);
    p.line_segment([at(-0.85, -0.3), at(0.9, -0.3)], st);
    p.line_segment([at(-0.9, 0.35), at(0.85, 0.35)], st);
}
