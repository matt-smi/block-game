pub mod chunk;
pub mod setup;

pub use chunk::VoxelMapping;
pub use chunk::{CHUNK_DATA_SIZE, CHUNK_DIMENSION, VoxelData, VoxelId};
pub use setup::VoxelResource;
pub use setup::WorldPlugin;
