use bevy::prelude::*;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoxelId {
    Air = 0,
    Dirt = 1,
    Grass = 2,
    Stone = 3,
}

#[derive(Resource)]
pub struct VoxelMapping {
    pub colours: Vec<[f32; 4]>, // should match # of voxel IDs above
}

impl Default for VoxelMapping {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelMapping {
    pub fn new() -> Self {
        Self {
            colours: vec![
                [1.0, 1.0, 1.0, 1.0], // Air (dummy values)
                [0.5, 0.3, 0.2, 1.0], // Dirt
                [0.2, 0.8, 0.2, 1.0], // Grass
                [0.6, 0.6, 0.6, 1.0], // Stone
            ],
        }
    }
}

pub const CHUNK_DIMENSION: u32 = 32;
pub const CHUNK_DATA_SIZE: usize = (CHUNK_DIMENSION * CHUNK_DIMENSION * CHUNK_DIMENSION) as usize;

#[derive(Component)]
struct _ChunkCoord(IVec3);

#[derive(Component)]
pub struct ChunkCoord(pub IVec3);

pub struct VoxelData {
    pub voxels: Vec<VoxelId>,
    pub size: UVec3,
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
