use avian3d::prelude::PhysicsLayer;

pub const COLLIDER_PRUNE_DISTANCE: u32 = 3;

#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum Layers {
    #[default]
    Default, // Layer 0 - the default layer that objects are assigned to
    Player,  // Layer 1
    Terrain, // Layer 3
}

// TODO: Make nearby collider generation async.
//#[derive(Resource)]
// pub struct _ColliderChannel {
//     _sender: Sender<Collider>,
//     _reciever: Mutex<Receiver<(IVec3, Collider)>>,
// }
