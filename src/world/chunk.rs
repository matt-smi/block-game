use bevy::{platform::collections::HashMap, prelude::*};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender};
use std::cmp::min;
use std::collections::HashSet;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelId {
    Air = 0,
    Dirt = 1,
    Grass = 2,
    Stone = 3,
}

pub struct VoxelMapping {
    pub colours: [[f32; 4]; 4],
}

pub const VOXEL_MAPPING: VoxelMapping = VoxelMapping {
    colours: [
        [1.0, 1.0, 1.0, 1.0], // Air (dummy values)
        [0.5, 0.3, 0.2, 1.0], // Dirt
        [0.2, 0.8, 0.2, 1.0], // Grass
        [0.6, 0.6, 0.6, 1.0], // Stone
    ],
};


pub const CHUNK_RENDER_DISTANCE: i32 = 32;
pub const CHUNK_Y_COUNT: i32 = 10;
pub const CHUNK_DIMENSION: u32 = 32;
pub const CHUNK_DATA_SIZE: usize = (CHUNK_DIMENSION * CHUNK_DIMENSION * CHUNK_DIMENSION) as usize;
pub const WORLD_VOXEL_SIZE: f32 = 1.0;
pub const CHUNK_WORLD_SIZE: f32 = CHUNK_DIMENSION as f32 * WORLD_VOXEL_SIZE;
pub const LOD_INTERVAL: u8 = 20;
/// Max chunk-grid steps the player may move between LOD refresh passes (fast movement).
pub const LOD_UPDATE_BUFFER: u32 = 3;

#[derive(Component)]
struct _ChunkCoord(IVec3);

#[derive(Component)]
pub struct ChunkCoord(pub IVec3);

#[derive(Clone)]
pub struct VoxelData {
    pub voxels: Vec<VoxelId>,
    /* 
    these two fields are easily derivable from voxels size, but I'm going to make it explicit for now

    lod value can be 0-3: 
        (0 - 24 chunks)        (24 - 48 chunks)      (48 - 72 chunks)     (72 - 96 chunks)
        lod = 0 -> 32 x 32 x 32, lod = 1 -> 16 x 16 x 16, lod = 2 -> 8 x 8 x 8, lod = 3 -> 4 x 4 x 4 
        voxel_scale:    1                        2                       4                     8                     
    */
    pub lod: u8, 
}


impl VoxelData {
    pub fn index(&self, x: u32, y: u32, z: u32) -> usize {
        (x + z * CHUNK_DIMENSION + y * CHUNK_DIMENSION * CHUNK_DIMENSION) as usize
    }

    pub fn get(&self, x: u32, y: u32, z: u32) -> VoxelId {
        self.voxels[self.index(x, y, z)]
    }

    pub fn set(&mut self, x: u32, y: u32, z: u32, voxel: VoxelId) {
        let idx = self.index(x, y, z);
        self.voxels[idx] = voxel;
    }

    pub fn get_id(&self, x: u32, y: u32, z: u32) -> u8 {
        self.get(x, y, z) as u8
    }

    pub fn set_id(&mut self, x: u32, y: u32, z: u32, id: u8) {
        let voxel = match id {
            0 => VoxelId::Air,
            1 => VoxelId::Dirt,
            2 => VoxelId::Grass,
            3 => VoxelId::Stone,
            _ => VoxelId::Air,
        };
        self.set(x, y, z, voxel);
    }
}

#[derive(Component)]
pub struct ChunkMesh {
    pub handle: Handle<Mesh>,
}

/*
    Chunk key is indexed by chunk position. E.g <-2, 0, 10> -> <-2 * CHUNK_SIZE, 0 * CHUNK_SIZE, 10 * CHUNK_SIZE> (world position).
    Also note Y is not used, since the value represents the column at the x, z coordinates.
    Leaving y in here if we want to support chunk layering later on.
*/
#[derive(Resource)]
pub struct ChunkEntities {
    pub chunks: HashMap<IVec3, Vec<Entity>>,
}

#[derive(Resource)]
pub struct ChunkVoxels {
    pub chunks: HashMap<IVec3, VoxelData>,
}

#[derive(Resource)]
pub struct LastChunk {
    pub chunk_pos: IVec3,
}

#[derive(Resource)]
pub struct ChunkChannel {
    pub processing_queue: HashSet<IVec3>, // TODO: Make it so processing queue contains (IVec3 + LOD) as key
    pub sender: Sender<(IVec3, Mesh, VoxelData)>,
    pub receiver: Mutex<Receiver<(IVec3, Mesh, VoxelData)>>,
}

pub fn world_to_global_voxel(world_position: Vec3) -> IVec3 {
    IVec3::new(
        (world_position.x / WORLD_VOXEL_SIZE).floor() as i32,
        (world_position.y / WORLD_VOXEL_SIZE).floor() as i32,
        (world_position.z / WORLD_VOXEL_SIZE).floor() as i32,
    )
}

pub fn global_voxel_to_chunk(global_voxel: IVec3) -> (IVec3, UVec3) {
    let chunk = IVec3::new(
        global_voxel.x.div_euclid(CHUNK_DIMENSION as i32),
        global_voxel.y.div_euclid(CHUNK_DIMENSION as i32),
        global_voxel.z.div_euclid(CHUNK_DIMENSION as i32),
    );
    let local = UVec3::new(
        global_voxel.x.rem_euclid(CHUNK_DIMENSION as i32) as u32,
        global_voxel.y.rem_euclid(CHUNK_DIMENSION as i32) as u32,
        global_voxel.z.rem_euclid(CHUNK_DIMENSION as i32) as u32,
    );
    (chunk, local)
}

pub fn voxel_at_global(chunks: &ChunkVoxels, global_voxel: IVec3) -> Option<VoxelId> {
    let (chunk, local) = global_voxel_to_chunk(global_voxel);
    chunks
        .chunks
        .get(&chunk)
        .map(|data| data.get(local.x, local.y, local.z))
}

pub fn is_solid_global_voxel(chunks: &ChunkVoxels, global_voxel: IVec3) -> bool {
    voxel_at_global(chunks, global_voxel).is_some_and(|voxel| voxel != VoxelId::Air)
}

pub fn chunk_world_origin(chunk: IVec3) -> Vec3 {
    Vec3::new(
        CHUNK_WORLD_SIZE * chunk.x as f32,
        CHUNK_WORLD_SIZE * chunk.y as f32,
        CHUNK_WORLD_SIZE * chunk.z as f32,
    )
}

/// Horizontal chunk-grid distance used for LOD (dx + dz; player chunk keeps y = 0).
pub fn chunk_lod_distance(curr_chunk: IVec3, target_chunk: IVec3) -> u32 {
    let dx = (curr_chunk.x - target_chunk.x).unsigned_abs();
    let dz = (curr_chunk.z - target_chunk.z).unsigned_abs();
    dx + dz
}

pub fn get_lod_from_distance(distance: u32) -> u8 {
    min((distance as u8) / LOD_INTERVAL, 3)
}

pub fn get_lod(curr_chunk: IVec3, target_chunk: IVec3) -> u8 {
    get_lod_from_distance(chunk_lod_distance(curr_chunk, target_chunk))
}

/// True if LOD could change after the player moves up to `movement_buffer` chunk steps.
///
/// Only distances near rings at 20, 40, 60 (LOD_INTERVAL) can cross a level; chunks farther
/// out stay in the same band until the player moves enough to widen the band.
pub fn lod_may_change_after_move(distance: u32, movement_buffer: u32) -> bool {
    let interval = LOD_INTERVAL as u32;
    for ring in 1..=3 {
        let boundary = ring * interval;
        if distance + movement_buffer >= boundary && distance.saturating_sub(movement_buffer) <= boundary {
            return true;
        }
    }
    false
}
