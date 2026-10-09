//! Colours and sizes. Our own palette (not taken from any other product).

use egui::Color32;

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub bg: Color32,
    pub sidebar: Color32,
    pub panel: Color32,
    pub text: Color32,
    pub dim: Color32,
    pub faint: Color32,
    pub hover: Color32,
    pub field_bg: Color32,
    pub accent: Color32,
    pub line: Color32,
    pub link: Color32,
}

impl Tokens {
    pub fn dark() -> Tokens {
        Tokens {
            bg: Color32::from_rgb(22, 23, 25),
            sidebar: Color32::from_rgb(22, 23, 25),
            panel: Color32::from_rgb(31, 32, 35),
            text: Color32::from_rgb(226, 227, 230),
            dim: Color32::from_rgb(150, 153, 160),
            faint: Color32::from_rgb(92, 95, 102),
            hover: Color32::from_rgb(44, 46, 50),
            field_bg: Color32::from_rgb(38, 39, 43),
            accent: Color32::from_rgb(98, 142, 255),
            line: Color32::from_rgb(52, 54, 59),
            link: Color32::from_rgb(120, 162, 255),
        }
    }

    pub fn light() -> Tokens {
        Tokens {
            bg: Color32::from_rgb(242, 241, 238),
            sidebar: Color32::from_rgb(242, 241, 238),
            panel: Color32::from_rgb(255, 255, 255),
            text: Color32::from_rgb(32, 33, 36),
            dim: Color32::from_rgb(104, 108, 116),
            faint: Color32::from_rgb(170, 172, 178),
            hover: Color32::from_rgb(236, 236, 240),
            field_bg: Color32::from_rgb(246, 246, 248),
            accent: Color32::from_rgb(52, 98, 230),
            line: Color32::from_rgb(222, 222, 226),
            link: Color32::from_rgb(40, 90, 220),
        }
    }
}

/// Supertag colour by name (see `tanacraft_core::TAG_COLORS`): strong (text, bullet) and soft (chip fill).
pub fn tag_color(name: &str, dark: bool) -> (Color32, Color32) {
    let (r, g, b) = match name {
        "red" => (232, 84, 84),
        "orange" => (236, 132, 60),
        "amber" => (224, 168, 48),
        "yellow" => (206, 190, 60),
        "lime" => (140, 196, 64),
        "green" => (70, 180, 110),
        "teal" => (44, 174, 160),
        "cyan" => (60, 172, 214),
        "blue" => (76, 132, 240),
        "indigo" => (110, 112, 236),
        "violet" => (154, 104, 232),
        "pink" => (226, 84, 166),
        c if tanacraft_core::is_custom_color(c) => {
            let h = |i: usize| u8::from_str_radix(c.get(i..i + 2).unwrap_or("80"), 16).unwrap_or(128);
            (h(1), h(3), h(5))
        }
        _ => (140, 144, 152),
    };
    let strong = Color32::from_rgb(r, g, b);
    let soft = if dark { Color32::from_rgba_unmultiplied(r, g, b, 46) } else { Color32::from_rgba_unmultiplied(r, g, b, 34) };
    (strong, soft)
}

pub const ROW_H: f32 = 28.0;
pub const INDENT: f32 = 26.0;
pub const FONT: f32 = 15.0;
