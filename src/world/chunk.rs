use bevy::prelude::*;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VoxelId {
    Air = 0,
    Dirt = 1,
    Grass = 2,
    Stone = 3,
}

#[derive(Component)]
struct ChunkCoord(IVec3);

#[derive(Component)]
struct VoxelData {
    voxels: Vec<VoxelId>,
    size: UVec3,
}
impl VoxelData {
    fn index(&self, x: u32, y: u32, z: u32) -> usize {
        (x + y * self.size.x + z * self.size.x * self.size.y) as usize
    }
    fn get(&self, x: u32, y: u32, z: u32) -> VoxelId {
        self.voxels[self.index(x, y, z)]
    }
}

#[derive(Component)]
struct ChunkMesh {
    handle: Handle<Mesh>,
}
