use bevy::prelude::*;

use crate::common::GameState;
use crate::plugins::camera::Angles2D; 

pub type Movement = (
    &'static mut Transform,
    &'static mut Velocity,
    &'static mut Angles2D,
);

pub struct MovementPlugin;

#[derive(Component, Debug)]
pub struct Velocity {
    pub value: Vec3,
}

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_position.run_if(in_state(GameState::Playing)));
    }
}

pub fn update_position(mut query: Query<(&Velocity, &mut Transform)>, time: Res<Time>) {
    for (velocity, mut transform) in query.iter_mut() {
        transform.translation += time.delta_secs() * velocity.value;
    }
}


