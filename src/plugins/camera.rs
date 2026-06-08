use avian3d::prelude::PhysicsSystems;
use bevy::core_pipeline::Skybox;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::{CursorGrabMode, CursorOptions};
use leafwing_input_manager::prelude::ActionState;

use crate::common::{GameAction, GameState};
use crate::plugins::movement::Movement;
use crate::plugins::player::Player;
use crate::plugins::procedural_sky::{
    TimeOfDay, day_night_palette, generate_sunset_cubemap, moon_rotation, sun_direction,
    sun_rotation,
};
use crate::world::{CHUNK_RENDER_DISTANCE, CHUNK_WORLD_SIZE};

const ORBIT_DISTANCE: f32 = 13.;
const SKY_BRIGHTNESS: f32 = 950.0;

#[derive(Component)]
pub struct Angles2D {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Component)]
struct Sun;

#[derive(Component)]
struct Moon;

#[derive(Component)]
pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeOfDay>()
            .add_systems(Startup, setup)
            .add_systems(Update, (advance_time_of_day, apply_day_night).chain())
            .add_systems(OnEnter(GameState::Playing), lock_cursor)
            .add_systems(OnExit(GameState::Playing), unlock_cursor)
            .add_systems(
                PostUpdate,
                (orbit, camera_look)
                    .after(PhysicsSystems::Prepare)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, time_of_day: Res<TimeOfDay>) {
    let palette = day_night_palette(time_of_day.fraction);
    let sun_dir = sun_direction(time_of_day.fraction);
    let skybox_image = images.add(generate_sunset_cubemap(sun_dir));

    let fog_visibility = CHUNK_RENDER_DISTANCE as f32 * CHUNK_WORLD_SIZE * 0.9;

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Skybox {
            image: skybox_image,
            brightness: SKY_BRIGHTNESS,
            ..default()
        },
        DistanceFog {
            color: palette.fog_color,
            directional_light_color: palette.fog_light_color,
            directional_light_exponent: 8.0,
            falloff: FogFalloff::from_visibility_colors(
                fog_visibility,
                Color::srgb(0.02, 0.04, 0.08),
                Color::srgb(0.55, 0.65, 0.85),
            ),
        },
    ));

    commands.spawn((
        Sun,
        DirectionalLight {
            illuminance: palette.sun_illuminance,
            color: palette.sun_color,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(sun_rotation(time_of_day.fraction)),
    ));

    commands.spawn((
        Moon,
        DirectionalLight {
            illuminance: palette.moon_illuminance,
            color: palette.moon_color,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(moon_rotation(time_of_day.fraction)),
    ));

    commands.insert_resource(AmbientLight {
        color: palette.ambient_color,
        brightness: palette.ambient_brightness,
        ..default()
    });
}

fn advance_time_of_day(mut time_of_day: ResMut<TimeOfDay>, clock: Res<Time>) {
    if time_of_day.seconds_per_day <= 0.0 {
        return;
    }
    time_of_day.fraction =
        (time_of_day.fraction + clock.delta_secs() / time_of_day.seconds_per_day) % 1.0;
}

fn apply_day_night(
    time_of_day: Res<TimeOfDay>,
    mut ambient: ResMut<AmbientLight>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), (With<Sun>, Without<Moon>)>,
    mut moon: Query<(&mut Transform, &mut DirectionalLight), (With<Moon>, Without<Sun>)>,
    mut fog: Query<&mut DistanceFog, With<Camera3d>>,
) {
    let palette = day_night_palette(time_of_day.fraction);

    ambient.color = palette.ambient_color;
    ambient.brightness = palette.ambient_brightness;

    for (mut transform, mut light) in &mut sun {
        transform.rotation = sun_rotation(time_of_day.fraction);
        light.illuminance = palette.sun_illuminance;
        light.color = palette.sun_color;
    }

    for (mut transform, mut light) in &mut moon {
        transform.rotation = moon_rotation(time_of_day.fraction);
        light.illuminance = palette.moon_illuminance;
        light.color = palette.moon_color;
    }

    for mut fog in &mut fog {
        fog.color = palette.fog_color;
        fog.directional_light_color = palette.fog_light_color;
    }
}

fn lock_cursor(mut cursor_options: Query<&mut CursorOptions>) {
    let mut cursor_options = cursor_options.single_mut().unwrap();
    cursor_options.grab_mode = CursorGrabMode::Locked;
    cursor_options.visible = false;
}

fn unlock_cursor(mut cursor_options: Query<&mut CursorOptions>) {
    let mut cursor_options = cursor_options.single_mut().unwrap();
    cursor_options.grab_mode = CursorGrabMode::None;
    cursor_options.visible = true;
}

fn orbit(
    mut camera: Single<&mut Transform, With<Camera>>,
    player_transform: Query<&Transform, (With<Player>, Without<Camera>)>,
) {
    let target = player_transform.single().unwrap();
    camera.translation = target.translation - camera.forward() * ORBIT_DISTANCE;
    camera.rotation = target.rotation;
}

fn camera_look(
    single_player: Single<(Movement, &ActionState<GameAction>), With<Player>>,
    camera: Single<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    let ((_transform, _, mut angles), action_state) = single_player.into_inner();
    let mut camera_transform = camera.into_inner();

    let mouse_delta = action_state.axis_pair(&GameAction::Look);

    angles.yaw -= mouse_delta.x * 0.005;
    angles.pitch = (angles.pitch - mouse_delta.y * 0.005)
        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

    let yaw_q = Quat::from_rotation_y(angles.yaw);
    let pitch_q = Quat::from_rotation_x(angles.pitch);
    camera_transform.rotation = yaw_q * pitch_q;
}
