use eframe::egui::{self, Color32};

pub const CONTROL_RADIUS: f32 = 6.0;
pub const WINDOW_RADIUS: f32 = 12.0;

// Raycast source roles are in assets/themes/raycast; light mode is a native adaptation.
// background, surface, selection, foreground, secondary, muted, accent, border, danger
const LIGHT: [u32; 9] = [
    0xf9f9f9, 0xffffff, 0xeaf0f7, 0x101111, 0x454545, 0x666667, 0xd42e3d, 0xe2e2e3, 0xb42318,
];
const DARK: [u32; 9] = [
    0x07080a, 0x101111, 0x20252d, 0xf9f9f9, 0xcecece, 0x9c9c9d, 0xff6363, 0x282929, 0xff9696,
];

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub warm: Color32,
    pub foreground: Color32,
    pub secondary: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub brand: Color32,
    pub success: Color32,
    pub control_border: Color32,
    pub action: Color32,
    pub action_on: Color32,
    pub border: Color32,
    pub danger: Color32,
}

fn rgb(value: u32) -> Color32 {
    Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

pub fn contrast(a: Color32, b: Color32) -> f32 {
    let luminance = |color: Color32| {
        let channels = [color.r(), color.g(), color.b()].map(|channel| {
            let value = f32::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        });
        channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

// Preserve source hues while making small text readable on native opaque surfaces.
fn readable(color: Color32, target: Color32, backgrounds: &[Color32]) -> Color32 {
    for step in 0..=100 {
        let amount = step as f32 / 100.0;
        let mix =
            |a: u8, b: u8| (f32::from(a) * (1.0 - amount) + f32::from(b) * amount).round() as u8;
        let candidate = Color32::from_rgb(
            mix(color.r(), target.r()),
            mix(color.g(), target.g()),
            mix(color.b(), target.b()),
        );
        if backgrounds
            .iter()
            .all(|background| contrast(candidate, *background) >= 4.5)
        {
            return candidate;
        }
    }
    target
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        let [background, surface, warm, foreground, secondary, muted, accent, border, danger] =
            (if dark { DARK } else { LIGHT }).map(rgb);
        let target = if dark { Color32::WHITE } else { Color32::BLACK };
        let backgrounds = [background, surface];
        let brand = readable(accent, target, &backgrounds);
        let accent = readable(
            rgb(if dark { 0x55b3ff } else { 0x0967b4 }),
            target,
            &backgrounds,
        );
        let success = readable(
            rgb(if dark { 0x5fc992 } else { 0x187349 }),
            target,
            &backgrounds,
        );
        let action = foreground;
        let action_on = if contrast(Color32::WHITE, action) >= 4.5 {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        Self {
            background,
            surface,
            warm,
            foreground,
            border,
            accent,
            brand,
            success,
            control_border: rgb(if dark { 0x666a73 } else { 0x85898f }),
            action,
            action_on,
            secondary: readable(secondary, target, &[background, surface, warm]),
            muted: readable(muted, target, &backgrounds),
            danger: readable(danger, target, &backgrounds),
        }
    }

    pub fn get(ctx: &egui::Context) -> Self {
        ctx.data(|data| data.get_temp::<Self>(egui::Id::new("active-palette")))
            .unwrap_or_else(|| Self::new(ctx.style().visuals.dark_mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raycast_is_readable_in_both_appearances() {
        assert_eq!(Palette::new(true).background, rgb(0x07080a));
        assert_eq!(Palette::new(false).background, rgb(0xf9f9f9));
        for dark in [true, false] {
            let colors = Palette::new(dark);
            for background in [colors.background, colors.surface] {
                for text in [
                    colors.foreground,
                    colors.secondary,
                    colors.muted,
                    colors.accent,
                    colors.success,
                    colors.brand,
                    colors.danger,
                ] {
                    assert!(
                        contrast(text, background) >= 4.5,
                        "Unreadable Raycast text in dark={dark}"
                    );
                }
            }
            assert!(contrast(colors.control_border, colors.surface) >= 3.0);
            assert!(contrast(colors.foreground, colors.warm) >= 4.5);
            assert!(contrast(colors.secondary, colors.warm) >= 4.5);
            assert!(contrast(colors.accent, colors.warm) >= 3.0);
            assert!(contrast(colors.action_on, colors.action) >= 4.5);
        }
    }
}
