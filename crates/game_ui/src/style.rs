//! Shared UI colours and fonts.

use bevy::prelude::*;

pub const PANEL: Color = Color::srgba(0.08, 0.08, 0.1, 0.86);
pub const TEXT: Color = Color::srgb(0.95, 0.93, 0.88);
pub const BUTTON: Color = Color::srgb(0.22, 0.22, 0.26);
pub const BUTTON_ACTIVE: Color = Color::srgb(0.55, 0.42, 0.2);

pub fn font(px: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(px),
        ..default()
    }
}

/// A text bar for a 0..1 value, e.g. `[#######---]` (ASCII: the default
/// font has no block characters).
pub fn bar(value: f32, width: usize) -> String {
    let filled = ((value.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    format!("[{}{}]", "#".repeat(filled), "-".repeat(width - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars() {
        assert_eq!(bar(0.0, 4), "[----]");
        assert_eq!(bar(0.5, 4), "[##--]");
        assert_eq!(bar(2.0, 4), "[####]");
    }
}
