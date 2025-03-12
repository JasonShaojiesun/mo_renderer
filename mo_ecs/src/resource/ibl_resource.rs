use bevy_ecs::system::Resource;
use mo_vk::{Texture, TextureCreateInfo, VULKAN};
use std::sync::Arc;
use vulkano::image::max_mip_levels;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage},
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, layout::{
            DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
            DescriptorType,
        },
        DescriptorSet,
        WriteDescriptorSet,
    },
    format::Format,
    image::{
        sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo},
        ImageType,
    },
    pipeline::{
        compute::ComputePipelineCreateInfo, layout::{PipelineLayoutCreateInfo, PushConstantRange}, ComputePipeline, Pipeline,
        PipelineBindPoint,
        PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    shader::ShaderStages,
    sync,
    sync::GpuFuture,
};

#[derive(Resource)]
pub struct IBLResource {
    // IBL related textures
    pub cubemap_src: Arc<Texture>,
    pub environment_map: Arc<Texture>,
    pub irradiance_map: Arc<Texture>,
    pub specular_map: Arc<Texture>,
    pub brdf_lut: Arc<Texture>,
}

const IBL_IMAGE_WIDTH_HEIGHT: u32 = 512;

impl IBLResource {
    pub fn new(name: &str) -> Self {
        let equirect_info = TextureCreateInfo::default_hdr();
        let irrad_maps_info = TextureCreateInfo {
            extent: [IBL_IMAGE_WIDTH_HEIGHT, IBL_IMAGE_WIDTH_HEIGHT, 1],
            format: Format::R32G32B32A32_SFLOAT,
            hdr: true,
            ..Default::default()
        };
        let specular_map_info = TextureCreateInfo {
            extent: [IBL_IMAGE_WIDTH_HEIGHT, IBL_IMAGE_WIDTH_HEIGHT, 1],
            format: Format::R32G32B32A32_SFLOAT,
            hdr: true,
            mip_levels: max_mip_levels([IBL_IMAGE_WIDTH_HEIGHT, IBL_IMAGE_WIDTH_HEIGHT, 1]),
            ..Default::default()
        };
        let brdf_lut_info = TextureCreateInfo {
            image_type: ImageType::Dim2d,
            extent: [IBL_IMAGE_WIDTH_HEIGHT, IBL_IMAGE_WIDTH_HEIGHT, 1],
            format: Format::R16G16B16A16_SFLOAT,
            ..Default::default()
        };

        let path = format!("env/{name}.hdr");

        let equirect_map = Arc::new(Texture::load_from_file(path.as_str(), &equirect_info));

        let environment_map = Arc::new(Texture::new_cubemap(irrad_maps_info.clone()));
        let irradiance_map = Arc::new(Texture::new_cubemap(irrad_maps_info.clone()));
        let specular_map = Arc::new(Texture::new_cubemap(specular_map_info.clone()));
        let brdf_lut = Arc::new(Texture::new(brdf_lut_info));

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

        run_brdflut_cubemap_pipeline(&brdf_lut, &equirect_map, &sampler, &environment_map);
        run_irradiance_pipeline(&irradiance_map, &environment_map, &sampler);
        run_specular_pipeline(&specular_map, &environment_map, &sampler);

        tracing::info!("ECS - IBL Textures resources successfully prepared.");

        Self {
            cubemap_src: equirect_map,
            environment_map,
            irradiance_map,
            specular_map,
            brdf_lut,
        }
    }
}

impl Default for IBLResource {
    fn default() -> Self {
        Self::new("default_sky")
    }
}

fn run_brdflut_cubemap_pipeline(
    brdf_lut: &Arc<Texture>,
    equirect_map: &Arc<Texture>,
    sampler: &Arc<Sampler>,
    environment_map: &Arc<Texture>,
) {
    let pipeline = create_brdflut_cubemap_pipeline();

    let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
        VULKAN.device().clone(),
        Default::default(),
    ));

    let layout = &pipeline.layout().set_layouts()[0];
    let set0 = DescriptorSet::new(
        descriptor_set_allocator.clone(),
        layout.clone(),
        [
            WriteDescriptorSet::image_view(0, brdf_lut.image_view.clone()),
            WriteDescriptorSet::image_view(1, environment_map.image_view.clone()),
        ],
        [],
    )
    .unwrap();
    let layout = &pipeline.layout().set_layouts()[1];
    let set1 = DescriptorSet::new(
        descriptor_set_allocator.clone(),
        layout.clone(),
        [WriteDescriptorSet::image_view_sampler(
            0,
            equirect_map.image_view.clone(),
            sampler.clone(),
        )],
        [],
    )
    .unwrap();

    let mut builder = AutoCommandBufferBuilder::primary(
        VULKAN.command_buffer_allocator().clone(),
        VULKAN.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    builder
        .bind_pipeline_compute(pipeline.clone())
        .unwrap()
        .push_constants(
            pipeline.layout().clone(),
            0,
            cs::PushConsts {
                BRDF_W: IBL_IMAGE_WIDTH_HEIGHT,
                BRDF_H: IBL_IMAGE_WIDTH_HEIGHT,
            },
        )
        .unwrap()
        .bind_descriptor_sets(
            PipelineBindPoint::Compute,
            pipeline.layout().clone(),
            0,
            vec![set0, set1],
        )
        .unwrap();

    unsafe {
        builder
            // We have 8 x and y working groups in our compute shader
            .dispatch([IBL_IMAGE_WIDTH_HEIGHT / 8, IBL_IMAGE_WIDTH_HEIGHT / 8, 6])
            .unwrap();
    }

    // Finish recording the command buffer by calling `end`.
    let command_buffer = builder.build().unwrap();

    let future = sync::now(VULKAN.device().clone())
        .then_execute(VULKAN.graphics_queue().clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();

    future.wait(None).unwrap();
}

fn run_irradiance_pipeline(
    irradiance_map: &Arc<Texture>,
    environment_map: &Arc<Texture>,
    sampler: &Arc<Sampler>,
) {
    let pipeline = create_irradiance_pipeline();
    let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
        VULKAN.device().clone(),
        Default::default(),
    ));

    let layout = &pipeline.layout().set_layouts()[0];
    let set0 = DescriptorSet::new(
        descriptor_set_allocator.clone(),
        layout.clone(),
        [
            WriteDescriptorSet::image_view_sampler(
                0,
                environment_map.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view(1, irradiance_map.image_view.clone()),
        ],
        [],
    )
    .unwrap();

    let mut builder = AutoCommandBufferBuilder::primary(
        VULKAN.command_buffer_allocator().clone(),
        VULKAN.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    builder
        .bind_pipeline_compute(pipeline.clone())
        .unwrap()
        .bind_descriptor_sets(
            PipelineBindPoint::Compute,
            pipeline.layout().clone(),
            0,
            vec![set0],
        )
        .unwrap();

    unsafe {
        builder
            // We have 8 x and y working groups in our compute shader
            .dispatch([IBL_IMAGE_WIDTH_HEIGHT / 8, IBL_IMAGE_WIDTH_HEIGHT / 8, 6])
            .unwrap();
    }

    // Finish recording the command buffer by calling `end`.
    let command_buffer = builder.build().unwrap();

    let future = sync::now(VULKAN.device().clone())
        .then_execute(VULKAN.graphics_queue().clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();

    future.wait(None).unwrap();
}

fn run_specular_pipeline(
    specular_map: &Arc<Texture>,
    environment_map: &Arc<Texture>,
    sampler: &Arc<Sampler>,
) {
    let pipeline = create_specular_pipeline();
    let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
        VULKAN.device().clone(),
        Default::default(),
    ));

    let layout = &pipeline.layout().set_layouts()[0];
    let set0 = DescriptorSet::new(
        descriptor_set_allocator.clone(),
        layout.clone(),
        [
            WriteDescriptorSet::image_view_sampler(
                0,
                environment_map.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view(1, specular_map.image_view.clone()),
        ],
        [],
    )
    .unwrap();

    for i in 2..=specular_map.info.mip_levels {
        let mut builder = AutoCommandBufferBuilder::primary(
            VULKAN.command_buffer_allocator().clone(),
            VULKAN.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let roughness = i as f32 / (specular_map.info.mip_levels - 1) as f32;

        builder
            .bind_pipeline_compute(pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                pipeline.layout().clone(),
                0,
                vec![set0.clone()],
            )
            .unwrap()
            .push_constants(
                pipeline.layout().clone(),
                0,
                specular_cs::PushConsts { roughness },
            )
            .unwrap();

        unsafe {
            builder
                // We have 8 x and y working groups in our compute shader
                .dispatch([IBL_IMAGE_WIDTH_HEIGHT / 8, IBL_IMAGE_WIDTH_HEIGHT / 8, 6])
                .unwrap();
        }

        // Finish recording the command buffer by calling `end`.
        let command_buffer = builder.build().unwrap();

        let future = sync::now(VULKAN.device().clone())
            .then_execute(VULKAN.graphics_queue().clone(), command_buffer)
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap();

        future.wait(None).unwrap();

        specular_map.copy_to_mip_level(i);
    }

    let mut builder = AutoCommandBufferBuilder::primary(
        VULKAN.command_buffer_allocator().clone(),
        VULKAN.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    let roughness = 1.0 / (specular_map.info.mip_levels - 1) as f32;

    builder
        .bind_pipeline_compute(pipeline.clone())
        .unwrap()
        .bind_descriptor_sets(
            PipelineBindPoint::Compute,
            pipeline.layout().clone(),
            0,
            vec![set0],
        )
        .unwrap()
        .push_constants(
            pipeline.layout().clone(),
            0,
            specular_cs::PushConsts { roughness },
        )
        .unwrap();

    unsafe {
        builder
            // We have 8 x and y working groups in our compute shader
            .dispatch([IBL_IMAGE_WIDTH_HEIGHT / 8, IBL_IMAGE_WIDTH_HEIGHT / 8, 6])
            .unwrap();
    }

    // Finish recording the command buffer by calling `end`.
    let command_buffer = builder.build().unwrap();

    let future = sync::now(VULKAN.device().clone())
        .then_execute(VULKAN.graphics_queue().clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();

    future.wait(None).unwrap();
}

fn create_brdflut_cubemap_pipeline() -> Arc<ComputePipeline> {
    let shader = cs::load(VULKAN.device().clone())
        .unwrap()
        .entry_point("main")
        .unwrap();

    let stage = PipelineShaderStageCreateInfo::new(shader);

    let pipeline_layout = PipelineLayout::new(
        VULKAN.device().clone(),
        PipelineLayoutCreateInfo {
            set_layouts: vec![
                // Set 0 - BRDF LUT Texture.
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [
                            (
                                0,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::COMPUTE,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::StorageImage,
                                    )
                                },
                            ),
                            (
                                1,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::COMPUTE,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::StorageImage,
                                    )
                                },
                            ),
                        ]
                        .into_iter()
                        .collect(),
                        ..Default::default()
                    },
                )
                .unwrap(),
                // Input Descriptor set.  Number 1.
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
                size: size_of::<cs::PushConsts>() as u32,
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

fn create_irradiance_pipeline() -> Arc<ComputePipeline> {
    let shader = irrad_cs::load(VULKAN.device().clone())
        .unwrap()
        .entry_point("main")
        .unwrap();

    let stage = PipelineShaderStageCreateInfo::new(shader);

    let pipeline_layout = PipelineLayout::new(
        VULKAN.device().clone(),
        PipelineLayoutCreateInfo {
            set_layouts: vec![
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [
                            (
                                0,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::COMPUTE,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                1,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::COMPUTE,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::StorageImage,
                                    )
                                },
                            ),
                        ]
                        .into_iter()
                        .collect(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            ],
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

fn create_specular_pipeline() -> Arc<ComputePipeline> {
    let shader = specular_cs::load(VULKAN.device().clone())
        .unwrap()
        .entry_point("main")
        .unwrap();

    let stage = PipelineShaderStageCreateInfo::new(shader);

    let pipeline_layout = PipelineLayout::new(
        VULKAN.device().clone(),
        PipelineLayoutCreateInfo {
            set_layouts: vec![
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [
                            (
                                0,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::COMPUTE,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                1,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::COMPUTE,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::StorageImage,
                                    )
                                },
                            ),
                        ]
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
                size: size_of::<specular_cs::PushConsts>() as u32,
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

mod cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../resources/shaders/ibl/cubemap_brdflut.comp"
    }
}

mod irrad_cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../resources/shaders/ibl/irradiance.comp"
    }
}

mod specular_cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../resources/shaders/ibl/specular.comp"
    }
}
