use bevy::prelude::*;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum _VoxelId {
    Air = 0,
    Dirt = 1,
    Grass = 2,
    Stone = 3,
}

#[derive(Component)]
struct _ChunkCoord(IVec3);

#[derive(Component)]
struct _VoxelData {
    voxels: Vec<_VoxelId>,
    size: UVec3,
}

impl _VoxelData {
    fn _index(&self, x: u32, y: u32, z: u32) -> usize {
        (x + y * self.size.x + z * self.size.x * self.size.y) as usize
    }
    fn _get(&self, x: u32, y: u32, z: u32) -> _VoxelId {
        self.voxels[self._index(x, y, z)]
    }
}

#[derive(Component)]
struct _ChunkMesh {
    handle: Handle<Mesh>,
}
