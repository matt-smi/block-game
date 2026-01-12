use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use crate::plugins::camera::Angles2D;

pub type Movement = (
    &'static mut Transform,
    &'static mut LinearVelocity,
    &'static mut Angles2D,
);
