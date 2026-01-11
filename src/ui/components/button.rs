use bevy::prelude::*;

pub const NORMAL_BUTTON: Color = Color::BLACK;
pub const HOVERED_BUTTON: Color = Color::srgb(0.0, 0.0, 1.0);
pub const PRESSED_BUTTON: Color = Color::srgb(0.5, 0.5, 0.5);
pub const RED: Color = Color::srgb(1.0, 0.0, 0.0);

pub fn button(label: &str) -> impl Bundle {
    (
        Node {
            width: Val::Px(400.0),
            height: Val::Px(65.0),
            border: UiRect::all(Val::Px(5.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            margin: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        Button,
        BackgroundColor(Color::BLACK),
        BorderColor::all(Color::WHITE),
        BorderRadius::MAX,
        children![(
            Text::new(label),
            TextFont {
                font_size: 30.0,
                ..default()
            },
            TextColor(Color::WHITE),
            TextShadow::default(),
        )],
    )
}
