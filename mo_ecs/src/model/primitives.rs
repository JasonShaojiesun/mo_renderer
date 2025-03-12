use bevy_math::prelude::*;
use mo_vk::VULKAN;
use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
use vulkano::pipeline::graphics::vertex_input::Vertex;

#[derive(BufferContents, Vertex, Clone, Copy)]
#[repr(C)]
pub struct StaticVertex {
    #[format(R32G32B32A32_SFLOAT)]
    pub position: [f32; 4],
    #[format(R32G32B32A32_SFLOAT)]
    pub normal: [f32; 4],
    #[format(R32G32B32A32_SFLOAT)]
    pub color: [f32; 4],
    #[format(R32G32_SFLOAT)]
    pub uv0: [f32; 2],
    #[format(R32G32_SFLOAT)]
    pub uv1: [f32; 2],
    #[format(R32G32B32A32_SFLOAT)]
    pub tangent: [f32; 4],
}

impl Default for StaticVertex {
    fn default() -> Self {
        Self {
            position: [0.0; 4],
            normal: [0.0; 4],
            color: [1.0; 4],
            uv0: [0.0; 2],
            uv1: [0.0; 2],
            tangent: [0.0; 4],
        }
    }
}

pub struct MeshPrimitive {
    pub vertex_buffer: Subbuffer<[StaticVertex]>,
    pub index_buffer: Subbuffer<[u32]>,
    pub indices: Vec<u32>,
    pub vertices: Vec<StaticVertex>,
}

#[repr(u32)]
#[derive(Copy, Clone)]
pub enum AlphaMode {
    Opaque = 0,
    Mask = 1,
    Blend = 2,
}

/// Note: indexes into the Model specific texture array, not bindless indexes.
pub struct Material {
    // 贴图索引
    pub base_color_map: u32,
    pub normal_map: u32,
    pub metallic_roughness_map: u32,
    pub occlusion_map: u32,
    pub emissive_map: u32,

    // UV集配置
    pub base_color_uv_set: u32,
    pub normal_uv_set: u32,
    pub metallic_roughness_uv_set: u32,
    pub occlusion_uv_set: u32,
    pub emissive_uv_set: u32,

    // 混合模式
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,

    // 材质参数
    pub base_color_factor: Vec4,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub emissive_factor: Vec3,

    // 扩展字段
    pub material_type: u32,
    pub material_property: f32,
}

pub struct Mesh {
    pub primitive: MeshPrimitive,
    pub material: Material,
    pub gpu_mat_index: u32,
    pub world: Mat4,
}

impl MeshPrimitive {
    pub fn new(indices: Vec<u32>, vertices: Vec<StaticVertex>) -> Self {
        let memory_allocator = VULKAN.memory_allocator().clone();

        let vertex_buffer = Buffer::from_iter(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            vertices.clone(),
        )
        .unwrap();

        let index_buffer = Buffer::from_iter(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            indices.clone(),
        )
        .unwrap();

        MeshPrimitive {
            index_buffer,
            vertex_buffer,
            indices,
            vertices,
        }
    }
}
