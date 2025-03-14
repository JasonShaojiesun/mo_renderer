use crate::RenderPassTrait;
use bevy_ecs::prelude::World;
use mo_vk::{Texture, TextureCreateInfo, VULKAN, VulkanoWindowRenderer};
use std::sync::Arc;
use vulkano::descriptor_set::layout::{
    DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType,
};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::pipeline::compute::ComputePipelineCreateInfo;
use vulkano::pipeline::layout::{PipelineLayoutCreateInfo, PushConstantRange};
use vulkano::pipeline::{
    Pipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo,
};
use vulkano::shader::ShaderStages;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    format::Format,
    image::{
        ImageUsage,
        sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo},
        view::ImageView,
    },
    pipeline::ComputePipeline,
};
use winit::dpi::PhysicalSize;

pub struct SSAOPass {
    pipeline: Arc<ComputePipeline>,
    pub ssao_texture: Arc<Texture>,
    gbuffer_depth_texture: Arc<Texture>,
    sampler: Arc<Sampler>,
    window_size: PhysicalSize<u32>,
}

impl SSAOPass {
    pub fn new(gbuffer_depth_texture: Arc<Texture>, renderer: &VulkanoWindowRenderer) -> Self {
        let sampler = Sampler::new(
            VULKAN.device().clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [SamplerAddressMode::ClampToEdge; 3],
                ..Default::default()
            },
        )
        .unwrap();

        let ssao_tex_ci = TextureCreateInfo {
            format: Format::R8G8B8A8_UNORM,
            extent: [
                renderer.window_size()[0] as u32,
                renderer.window_size()[1] as u32,
                1,
            ],
            usage: ImageUsage::SAMPLED | ImageUsage::STORAGE,
            ..Default::default()
        };

        let ssao_texture = Arc::new(Texture::new(ssao_tex_ci));
        let pipeline = create_ssao_pipeline();

        let window_size = renderer.window_size();

        Self {
            pipeline,
            ssao_texture,
            gbuffer_depth_texture,
            sampler,
            window_size: window_size.into(),
        }
    }
}

impl RenderPassTrait for SSAOPass {
    fn render(
        &mut self,
        _image_idx: u32,
        _world: &World,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        let layout = &self.pipeline.layout().set_layouts()[0];
        let set0 = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            layout.clone(),
            [WriteDescriptorSet::image_view(
                0,
                self.ssao_texture.image_view.clone(),
            )],
            [],
        )
        .unwrap();
        let layout = &self.pipeline.layout().set_layouts()[1];
        let set1 = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            layout.clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                self.gbuffer_depth_texture.image_view.clone(),
                self.sampler.clone(),
            )],
            [],
        )
        .unwrap();

        builder
            .bind_pipeline_compute(self.pipeline.clone())
            .unwrap()
            .push_constants(
                self.pipeline.layout().clone(),
                0,
                ssao_shader::PushConsts {
                    textureResolution: [self.window_size.width, self.window_size.height],
                    frameIndex: 0,
                },
            )
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.pipeline.layout().clone(),
                0,
                vec![set0, set1],
            )
            .unwrap();

        unsafe {
            builder
                .dispatch([
                    self.window_size.width / 16 + 1,
                    self.window_size.height / 16 + 1,
                    1,
                ])
                .unwrap();
        }
    }

    fn on_swapchain_recreate(
        &mut self,
        _swapchain_images: &[Arc<ImageView>],
        window_size: PhysicalSize<u32>,
    ) {
        self.window_size = window_size;
    }
}

mod ssao_shader {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../resources/shaders/ssao.comp"
    }
}

fn create_ssao_pipeline() -> Arc<ComputePipeline> {
    let shader = ssao_shader::load(VULKAN.device().clone())
        .unwrap()
        .entry_point("main")
        .unwrap();

    let stage = PipelineShaderStageCreateInfo::new(shader);

    let pipeline_layout = PipelineLayout::new(
        VULKAN.device().clone(),
        PipelineLayoutCreateInfo {
            set_layouts: vec![
                // Set 0 - SSAO Texture.
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [(
                            0,
                            DescriptorSetLayoutBinding {
                                stages: ShaderStages::COMPUTE,
                                descriptor_count: 1,
                                ..DescriptorSetLayoutBinding::descriptor_type(
                                    DescriptorType::StorageImage,
                                )
                            },
                        )]
                        .into_iter()
                        .collect(),
                        ..Default::default()
                    },
                )
                .unwrap(),
                // Material Descriptor set.  Number 1.
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [(
                            0,
                            DescriptorSetLayoutBinding {
                                stages: ShaderStages::COMPUTE,
                                descriptor_count: 1,
                                ..DescriptorSetLayoutBinding::descriptor_type(
                                    DescriptorType::CombinedImageSampler,
                                )
                            },
                        )]
                        .into_iter()
                        .collect(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            ],
            push_constant_ranges: vec![PushConstantRange {
                stages: ShaderStages::COMPUTE,
                offset: 0,
                size: size_of::<ssao_shader::PushConsts>() as u32,
            }],
            ..Default::default()
        },
    )
    .unwrap();

    ComputePipeline::new(
        VULKAN.device().clone(),
        None,
        ComputePipelineCreateInfo::stage_layout(stage, pipeline_layout),
    )
    .unwrap()
}
