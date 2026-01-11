use crate::{
    common::{GameState, MenuState},
    ui::components::button::*,
    ui::shared::ButtonInteraction,
};
use bevy::prelude::*;

/// A wrapper trait than handles initialization and cleanup of a menu UI's resources.
/// Menu plugins are intended to implement this and the Plugin trait,
/// and then use these methods to direct the `Plugin::build` method.
///
/// The implementor of this trait is responsible for implementing the create method.
///
pub trait Menu<C: Component + Default> {
    fn setup_sys() -> impl FnMut(Commands) + Copy + Send + Sync + 'static {
        |mut commands: Commands| {
            for bundle in Self::create() {
                commands.spawn(bundle).insert(C::default());
            }
        }
    }
    fn cleanup_sys() -> impl FnMut(Commands, Query<Entity, With<C>>) + Copy + Send + Sync + 'static
    {
        |mut commands: Commands, query: Query<Entity, With<C>>| {
            for e in &query {
                commands.entity(e).despawn();
            }
        }
    }

    /// Provides a list of bundles to be added to the ECS upon initialization.
    fn create() -> Vec<impl Bundle>;
}

/* --------------------------
 *  Main Menu UI Plugin
 * --------------------------
 */
#[derive(Component)]
enum MenuButtonAction {
    Play,
    Multiplayer,
    Quit,
}
#[derive(Component, Default)]
pub struct MainMenuUi;

pub struct MainMenu;
impl Menu<MainMenuUi> for MainMenu {
    fn create() -> Vec<impl Bundle> {
        vec![(
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            children![
                (button("Start"), MenuButtonAction::Play),
                (button("Multiplayer"), MenuButtonAction::Multiplayer),
                (button("Quit"), MenuButtonAction::Quit),
            ],
        )]
    }
}
impl Plugin for MainMenu {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MenuState::Main), MainMenu::setup_sys())
            .add_systems(OnExit(MenuState::Main), MainMenu::cleanup_sys())
            .add_systems(Update, main_menu.run_if(in_state(MenuState::Main)));
    }
}
fn main_menu(
    mut exit: MessageWriter<AppExit>,
    mut next_state: ResMut<NextState<GameState>>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
    mut query: Query<(&Interaction, &MenuButtonAction), ButtonInteraction>,
) {
    for (interaction, action) in &mut query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            MenuButtonAction::Play => next_state.set(GameState::Playing),
            MenuButtonAction::Multiplayer => next_menu_state.set(MenuState::Multiplayer),
            MenuButtonAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/* --------------------------
 *  Multiplayer Menu UI Plugin
 * --------------------------
 */
#[derive(Component)]
enum MultiplayerButtonAction {
    Server,
    Client,
    Back,
}
#[derive(Component, Default)]
pub struct MultiplayerMenuUi;

pub struct MultiplayerMenu;
impl Menu<MultiplayerMenuUi> for MultiplayerMenu {
    fn create() -> Vec<impl Bundle> {
        vec![(
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            children![
                (button("Host game"), MultiplayerButtonAction::Server),
                (button("Join game"), MultiplayerButtonAction::Client),
                (button("Back"), MultiplayerButtonAction::Back),
            ],
        )]
    }
}
impl Plugin for MultiplayerMenu {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(MenuState::Multiplayer),
            MultiplayerMenu::setup_sys(),
        )
        .add_systems(
            OnExit(MenuState::Multiplayer),
            MultiplayerMenu::cleanup_sys(),
        )
        .add_systems(
            Update,
            multiplayer_menu.run_if(in_state(MenuState::Multiplayer)),
        );
    }
}
fn multiplayer_menu(
    mut next_menu_state: ResMut<NextState<MenuState>>,
    mut query: Query<(&Interaction, &MultiplayerButtonAction), ButtonInteraction>,
) {
    for (interaction, action) in &mut query {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match action {
            MultiplayerButtonAction::Server => println!("Pressed host game"),
            MultiplayerButtonAction::Client => println!("Pressed join game"),
            MultiplayerButtonAction::Back => next_menu_state.set(MenuState::Main),
        }
    }
}
