use bevy::{platform::collections::HashMap, prelude::*};
use std::cmp::{Ordering, min};
use std::collections::BinaryHeap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender};

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

pub const CHUNK_RENDER_DISTANCE: i32 = 12;
pub const CHUNK_Y_COUNT: i32 = 10;
pub const CHUNK_DIMENSION: u32 = 32;
pub const CHUNK_DATA_SIZE: usize = (CHUNK_DIMENSION * CHUNK_DIMENSION * CHUNK_DIMENSION) as usize;
pub const WORLD_VOXEL_SIZE: f32 = 1.0;
pub const CHUNK_WORLD_SIZE: f32 = CHUNK_DIMENSION as f32 * WORLD_VOXEL_SIZE;
pub const LOD_INTERVAL: u8 = 20;
pub const MAX_CONCURRENT_CHUNK_JOBS: usize = 90;
pub const DISTANCE_MAX_PRIORITY: i32 = 12;

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
        (0 - LOD_INTERVAL chunks)     (LOD_INTERVAL - LOD_INTERVAL * 2 chunks)  ...
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

    pub fn is_solid(&self, x: u32, y: u32, z: u32) -> bool {
        self.get(x, y, z) != VoxelId::Air
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

#[derive(Clone, Copy)]
pub struct ChunkLoadInfo {
    pub pos: IVec3,
    pub lod: u8,
    pub is_replacing: bool,
    pub job_id: u64,
}

#[derive(Clone, Copy)]
pub struct RunningChunkJob {
    pub lod: u8,
    pub job_id: u64,
    pub is_replacing: bool,
}

#[derive(Clone, Copy)]
pub struct ChunkTaskRequest {
    pub pos: IVec3,
    pub lod: u8,
    pub is_replacing: bool,
    pub job_id: u64,
    pub priority: i32,
    pub is_down_sample: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct QueuedChunkJob {
    pub pos: IVec3,
    pub lod: u8,
    pub is_replacing: bool,
    pub job_id: u64,
    pub priority: i32,
}

impl Ord for QueuedChunkJob {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.job_id.cmp(&other.job_id))
    }
}

impl PartialOrd for QueuedChunkJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Resource, Default)]
pub struct ChunkScheduler {
    pub next_job_id: u64,
    pub pending: BinaryHeap<QueuedChunkJob>,
    pub latest: HashMap<IVec3, ChunkTaskRequest>,
    pub in_flight: HashMap<IVec3, RunningChunkJob>,
}

impl ChunkScheduler {
    pub fn request(
        &mut self,
        pos: IVec3,
        lod: u8,
        is_replacing: bool,
        is_down_sample: bool,
        curr_chunk: IVec3,
    ) -> ChunkTaskRequest {
        self.next_job_id += 1;
        let priority = chunk_priority(curr_chunk, pos, is_replacing, is_down_sample);
        let request = ChunkTaskRequest {
            pos,
            lod,
            is_replacing,
            is_down_sample,
            job_id: self.next_job_id,
            priority,
        };
        self.latest.insert(pos, request);
        self.pending.push(QueuedChunkJob {
            pos,
            lod,
            is_replacing,
            job_id: request.job_id,
            priority,
        });
        request
    }

    pub fn pop_next_valid(&mut self, curr_chunk: IVec3) -> Option<ChunkTaskRequest> {
        while let Some(entry) = self.pending.pop() {
            let Some(mut latest) = self.latest.get(&entry.pos).copied() else {
                continue;
            };
            if latest.job_id != entry.job_id || self.in_flight.contains_key(&entry.pos) {
                continue;
            }
            latest.priority = chunk_priority(
                curr_chunk,
                latest.pos,
                latest.is_replacing,
                latest.is_down_sample,
            );
            if latest.priority != entry.priority {
                self.latest.insert(entry.pos, latest);
                self.pending.push(QueuedChunkJob {
                    pos: latest.pos,
                    lod: latest.lod,
                    is_replacing: latest.is_replacing,
                    job_id: latest.job_id,
                    priority: latest.priority,
                });
                continue;
            }
            return Some(latest);
        }
        None
    }
}

fn chunk_priority(
    curr_chunk: IVec3,
    target_chunk: IVec3,
    is_replacing: bool,
    is_down_sample: bool,
) -> i32 {
    let dx = target_chunk.x - curr_chunk.x;
    let dz = target_chunk.z - curr_chunk.z;
    let dist2 = dx * dx + dz * dz;
    if dist2 < DISTANCE_MAX_PRIORITY {
        return 500_000;
    }
    let replace_bonus = if is_replacing && !is_down_sample {
        1_000_000
    } else {
        0
    };
    -dist2 + replace_bonus
}

#[derive(Resource, Default)]
pub struct ToBeInvalidatedChunks {
    pub chunks: HashMap<IVec3, Vec<Entity>>,
}

#[derive(Resource)]
pub struct ChunkChannel {
    pub sender: Sender<(ChunkLoadInfo, Option<Mesh>, VoxelData)>,
    pub receiver: Mutex<Receiver<(ChunkLoadInfo, Option<Mesh>, VoxelData)>>,
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
pub fn xz_chunk_manhattan_distance(curr_chunk: IVec3, target_chunk: IVec3) -> u32 {
    let dx = (curr_chunk.x - target_chunk.x).unsigned_abs();
    let dz = (curr_chunk.z - target_chunk.z).unsigned_abs();
    dx + dz
}

pub fn get_lod_from_distance(distance: u32) -> u8 {
    min((distance as u8) / LOD_INTERVAL, 3)
}

pub fn get_lod(curr_chunk: IVec3, target_chunk: IVec3) -> u8 {
    get_lod_from_distance(xz_chunk_manhattan_distance(curr_chunk, target_chunk))
}
