use crate::gbuffer_pass::gbuffer_fs;
use mo_ecs::model::GltfMaterialCPU;

impl From<GltfMaterialCPU> for gbuffer_fs::GltfMaterialGPU {
    fn from(material: GltfMaterialCPU) -> Self {
        todo!()
    }
}
