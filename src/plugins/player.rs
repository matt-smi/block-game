use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::common::*;
use crate::plugins::movement::*;
use crate::plugins::camera::Angles2D; 
//use crate::world::init_resources;

const INIT_VELOCITY: Vec3 = Vec3::ZERO;
const PLAYER_SPEED: f32 = 15.0;
const PLAYER_SCALE: f32 = 0.5;


#[derive(Component)]
pub struct Player;

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player) //after(init_resources))
            .add_systems(
                Update,
                (player_look, player_move).run_if(in_state(GameState::Playing)),
            );
    }
}

fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let body = meshes.add(Cuboid::new(
        2. * PLAYER_SCALE,
        4. * PLAYER_SCALE,
        2. * PLAYER_SCALE,
    ));

    let handle = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });

    commands.spawn((
        Mesh3d(body),
        MeshMaterial3d(handle),
        Transform {
            scale: Vec3::new(PLAYER_SCALE, PLAYER_SCALE, PLAYER_SCALE),
            ..default()
        },
        Angles2D {
            yaw: 0.0,
            pitch: 0.0,
        },
        Player,
        Velocity {
            value: INIT_VELOCITY,
        },
        default_game_action_map(),
    ));
}

fn player_look(single: Single<(Movement, &ActionState<GameAction>), With<Player>>) {
    let ((mut transform, _, mut angles), action_state) = single.into_inner();

    let mouse_delta = action_state.axis_pair(&GameAction::Look);
    angles.yaw -= mouse_delta.x * MOUSE_SENSITIVITY;
    angles.pitch = (angles.pitch - mouse_delta.y * MOUSE_SENSITIVITY)
        .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);

    transform.rotation = Quat::from_rotation_y(angles.yaw) * Quat::from_rotation_x(angles.pitch);
}

fn player_move(single: Single<(Movement, &ActionState<GameAction>), With<Player>>) {
    let ((transform, mut velocity, angles), action_state) = single.into_inner();
    let mut direction = Vec3::ZERO;
    let yaw_rot = Quat::from_rotation_y(angles.yaw);

    // Horizontal direction handling
    let hori = action_state.clamped_axis_pair(&GameAction::MoveHorizontal);
    direction += hori.x * (yaw_rot * Vec3::X).normalize();
    direction += hori.y * -(yaw_rot * Vec3::Z).normalize();

    // Vertical direction handling
    let vert = action_state.clamped_value(&GameAction::MoveVertical);
    direction += vert * Vec3::Y;

    velocity.value = direction.normalize_or_zero() * PLAYER_SPEED;

   // println!("Player position :{:?}", transform.translation);
}
