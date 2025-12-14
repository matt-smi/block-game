use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

pub const MOUSE_SENSITIVITY: f32 = 0.002;

pub struct InputPlugin;
impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(InputManagerPlugin::<GameAction>::default())
            .add_plugins(InputManagerPlugin::<MenuAction>::default())
            .init_resource::<ActionState<MenuAction>>()
            .insert_resource(MenuAction::default_menu_action_map());
    }
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
#[actionlike(DualAxis)]
pub enum GameAction {
    MoveHorizontal,
    Look,
    #[actionlike(Axis)]
    MoveVertical,
}

pub fn default_game_action_map() -> InputMap<GameAction> {
    InputMap::default()
        .with_dual_axis(GameAction::MoveHorizontal, VirtualDPad::wasd())
        .with_dual_axis(GameAction::Look, MouseMove::default())
        .with_axis(
            GameAction::MoveVertical,
            VirtualAxis::new(KeyCode::ShiftLeft, KeyCode::Space),
        )
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum MenuAction {
    ToggleMenu,
    ToggleDebug,
}
impl MenuAction {
    fn default_menu_action_map() -> InputMap<MenuAction> {
        InputMap::default()
            .with(Self::ToggleMenu, KeyCode::Escape)
            .with(Self::ToggleDebug, KeyCode::KeyV)
    }
}
