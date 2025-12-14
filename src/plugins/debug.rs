use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::common::MenuAction;

pub struct DebugPlugin {
    pub should_print: bool,
}
impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if self.should_print {
            app.add_systems(Update, print_position);
        }
        app.add_plugins(FpsOverlayPlugin {
            config: FpsOverlayConfig {
                text_config: TextFont {
                    font_size: 10.0,
                    ..default()
                },
                text_color: Color::WHITE,
                enabled: true,
                refresh_interval: core::time::Duration::from_millis(100),
            },
        });
        app.add_systems(Update, toggle_fps);
    }
}

fn print_position(query: Query<(Entity, &Transform)>) {
    for (entity, transform) in query.iter() {
        info!(
            "Entity {:?} is at position (x, y, z) {:?}",
            entity, transform.translation
        );
    }
}

fn toggle_fps(action_state: Res<ActionState<MenuAction>>, mut overlay: ResMut<FpsOverlayConfig>) {
    if action_state.just_pressed(&MenuAction::ToggleDebug) {
        overlay.enabled = !overlay.enabled;
    }
}
