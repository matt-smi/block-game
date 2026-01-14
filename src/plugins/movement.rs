use crate::plugins::camera::Angles2D;
use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;

pub type Movement = (
    &'static mut Transform,
    &'static mut LinearVelocity,
    &'static mut Angles2D,
);
