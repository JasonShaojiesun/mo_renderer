use bevy_ecs::prelude::*;
use mo_vk::VULKAN;
use std::sync::Arc;
use vulkano::image::sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo};

#[derive(Resource)]
pub struct GlobalSamplers {
    pub clamp: Arc<Sampler>,
    pub wrap: Arc<Sampler>,
    pub mirror: Arc<Sampler>,
}

impl Default for GlobalSamplers {
    fn default() -> Self {
        let clamp = Sampler::new(
            VULKAN.device().clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [SamplerAddressMode::ClampToEdge; 3],
                ..Default::default()
            },
        )
        .unwrap();

        let wrap = Sampler::new(
            VULKAN.device().clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [SamplerAddressMode::Repeat; 3],
                ..Default::default()
            },
        )
        .unwrap();

        let mirror = Sampler::new(
            VULKAN.device().clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [SamplerAddressMode::MirroredRepeat; 3],
                ..Default::default()
            },
        )
        .unwrap();

        Self {
            clamp,
            wrap,
            mirror,
        }
    }
}
