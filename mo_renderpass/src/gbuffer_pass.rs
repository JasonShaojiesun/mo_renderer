use crate::BufferPassTrait;
use bevy_ecs::prelude::*;
use mo_ecs::resource::GlobalSamplers;
use mo_ecs::{
    component::Transform,
    model::{Model, StaticVertex, DEFAULT_TEXTURE_MAP},
    resource::{Camera, DefaultTextures},
};
use mo_vk::{Texture, TextureCreateInfo, VulkanoWindowRenderer, VULKAN};
use std::{cell::RefCell, sync::Arc};
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::pipeline::graphics::color_blend::ColorComponents;
use vulkano::pipeline::graphics::depth_stencil::CompareOp;
use vulkano::pipeline::graphics::rasterization::CullMode;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderPassBeginInfo},
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, layout::{
            DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
            DescriptorType,
        },
        DescriptorSet,
        WriteDescriptorSet,
    },
    format::ClearValue,
    image::{sampler::Sampler, view::ImageView, ImageUsage},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    pipeline::{
        graphics::{
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            depth_stencil::{DepthState, DepthStencilState},
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::RasterizationState,
            vertex_input::{Vertex, VertexDefinition},
            viewport::{Viewport, ViewportState},
            GraphicsPipelineCreateInfo,
        }, layout::{PipelineLayoutCreateInfo, PushConstantRange}, DynamicState, GraphicsPipeline, Pipeline,
        PipelineBindPoint,
        PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    shader::ShaderStages,
};
use winit::dpi::PhysicalSize;

pub struct GBufferTextures {
    pub base_color: Arc<Texture>,
    pub position: Arc<Texture>,
    pub normal: Arc<Texture>,
    pub emissive: Arc<Texture>,
    /// Occlusion, Roughness, Metallic, Material ID
    pub pbr: Arc<Texture>,
    pub velocity: Arc<Texture>,
    pub depth: Arc<Texture>,
}

impl GBufferTextures {
    pub fn new(size: [f32; 2]) -> Self {
        let size = (size[0] as u32, size[1] as u32);

        let color_info = TextureCreateInfo {
            format: vulkano::format::Format::R8G8B8A8_UNORM,
            extent: [size.0, size.1, 1],
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            ..Default::default()
        };
        let base_color = Arc::new(Texture::new(color_info));

        let position_info = TextureCreateInfo {
            format: vulkano::format::Format::R32G32B32A32_SFLOAT,
            extent: [size.0, size.1, 1],
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            ..Default::default()
        };
        let position = Arc::new(Texture::new(position_info));

        let normal_info = TextureCreateInfo {
            format: vulkano::format::Format::R16G16B16A16_SFLOAT,
            extent: [size.0, size.1, 1],
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            ..Default::default()
        };
        let normal = Arc::new(Texture::new(normal_info));

        let emissive_info = TextureCreateInfo {
            format: vulkano::format::Format::R16G16B16A16_SFLOAT,
            extent: [size.0, size.1, 1],
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            ..Default::default()
        };
        let emissive = Arc::new(Texture::new(emissive_info));

        let pbr_info = TextureCreateInfo {
            format: vulkano::format::Format::R8G8B8A8_UNORM,
            extent: [size.0, size.1, 1],
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            ..Default::default()
        };
        let pbr = Arc::new(Texture::new(pbr_info));

        let velocity_info = TextureCreateInfo {
            format: vulkano::format::Format::R32G32_SFLOAT,
            extent: [size.0, size.1, 1],
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            ..Default::default()
        };
        let velocity = Arc::new(Texture::new(velocity_info));

        let depth_info = TextureCreateInfo {
            format: vulkano::format::Format::D32_SFLOAT,
            extent: [size.0, size.1, 1],
            usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
            ..Default::default()
        };
        let depth = Arc::new(Texture::new(depth_info));

        Self {
            base_color,
            position,
            normal,
            emissive,
            pbr,
            velocity,
            depth,
        }
    }
}

pub struct GBufferPass {
    gbuffer_renderpass: Arc<RenderPass>,
    gbuffer_framebuffer: Arc<Framebuffer>,
    gbuffer_pipeline: Arc<GraphicsPipeline>,
    pub gbuffer_textures: Arc<GBufferTextures>,
    texture_descriptor_set: Arc<DescriptorSet>,
    material_descriptor_set: Arc<DescriptorSet>,
    uniform_buffer: SubbufferAllocator,

    viewport: Viewport,
}

impl GBufferPass {
    pub fn new(
        world: &RefCell<World>,
        vulkano_window_renderer: &VulkanoWindowRenderer,
        descriptor_set_alloc: Arc<StandardDescriptorSetAllocator>,
    ) -> Self {
        let gbuffer_textures = GBufferTextures::new(vulkano_window_renderer.window_size());

        let render_pass = vulkano::single_pass_renderpass!(
            VULKAN.device().clone(),
            attachments: {
                base_color: {
                    format: gbuffer_textures.base_color.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                position: {
                    format: gbuffer_textures.position.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                normal: {
                    format: gbuffer_textures.normal.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                emissive: {
                    format: gbuffer_textures.emissive.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                pbr: {
                    format: gbuffer_textures.pbr.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                velocity: {
                    format: gbuffer_textures.velocity.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                depth: {
                    format: gbuffer_textures.depth.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                }
            },
            pass: {
                color: [base_color, position, normal, emissive, pbr, velocity],
                depth_stencil: {depth},
            },
        )
        .unwrap();

        let mut default_diffuse_map_index = 0u32;
        let mut default_normal_map_index = 0u32;
        let mut default_occlusion_map_index = 0u32;
        let mut default_metallic_roughness_map_index = 0u32;
        let mut default_black_map_index = 0u32;
        let mut next_bindless_image_index = 0u32;
        let mut textures: Vec<(Arc<ImageView>, Arc<Sampler>)> = Vec::new();

        let mut gpu_materials: Vec<vs::GpuMaterial> = Vec::new();

        let sampler = world.borrow().resource::<GlobalSamplers>().clamp.clone();

        // Add default textures to the bindless descriptor set and update the indices for them.
        add_default_textures(
            &world.borrow(),
            &mut default_diffuse_map_index,
            &mut default_normal_map_index,
            &mut default_occlusion_map_index,
            &mut default_metallic_roughness_map_index,
            &mut default_black_map_index,
            &mut textures,
            &sampler,
            &mut next_bindless_image_index,
        );

        let mut query = world.borrow_mut().query::<(&Transform, &mut Model)>();
        for (&_, model) in query.iter_mut(&mut world.borrow_mut()) {
            add_model(
                &mut gpu_materials,
                model.into_inner(),
                default_diffuse_map_index,
                default_normal_map_index,
                default_occlusion_map_index,
                default_metallic_roughness_map_index,
                default_black_map_index,
                &mut textures,
                &sampler,
                &mut next_bindless_image_index,
            );
        }

        let pipeline = {
            let vs = vs::load(VULKAN.device().clone())
                .unwrap()
                .entry_point("main")
                .unwrap();
            let fs = fs::load(VULKAN.device().clone())
                .unwrap()
                .entry_point("main")
                .unwrap();

            let vertex_input_state = [StaticVertex::per_vertex()].definition(&vs).unwrap();
            let stages = [
                PipelineShaderStageCreateInfo::new(vs.clone()),
                PipelineShaderStageCreateInfo::new(fs.clone()),
            ];

            let pipeline_layout = PipelineLayout::new(
                VULKAN.device().clone(),
                PipelineLayoutCreateInfo {
                    set_layouts: vec![
                        // We separate the bindless resource arrays to different sets.
                        // To avoid having more descriptors than max descriptor in one set.
                        // Texture descriptor set. Number 0.
                        DescriptorSetLayout::new(
                            VULKAN.device().clone(),
                            DescriptorSetLayoutCreateInfo {
                                bindings: [(
                                    0,
                                    DescriptorSetLayoutBinding {
                                        stages: ShaderStages::FRAGMENT,
                                        descriptor_count: textures.len() as u32,
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
                        // Material Descriptor set.  Number 1.
                        DescriptorSetLayout::new(
                            VULKAN.device().clone(),
                            DescriptorSetLayoutCreateInfo {
                                bindings: [(
                                    0,
                                    DescriptorSetLayoutBinding {
                                        stages: ShaderStages::FRAGMENT,
                                        descriptor_count: gpu_materials.len() as u32,
                                        ..DescriptorSetLayoutBinding::descriptor_type(
                                            DescriptorType::StorageBuffer,
                                        )
                                    },
                                )]
                                .into_iter()
                                .collect(),
                                ..Default::default()
                            },
                        )
                        .unwrap(),
                        // Uniform Descriptor set.  Number 2.
                        DescriptorSetLayout::new(
                            VULKAN.device().clone(),
                            DescriptorSetLayoutCreateInfo {
                                bindings: [(
                                    0,
                                    DescriptorSetLayoutBinding {
                                        stages: ShaderStages::VERTEX,
                                        descriptor_count: 1,
                                        ..DescriptorSetLayoutBinding::descriptor_type(
                                            DescriptorType::UniformBuffer,
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
                        stages: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                        offset: 0,
                        size: size_of::<vs::PushConsts>() as u32,
                    }],
                    ..Default::default()
                },
            )
            .unwrap();

            let subpass = Subpass::from(render_pass.clone(), 0).unwrap();

            GraphicsPipeline::new(
                VULKAN.device().clone(),
                None,
                GraphicsPipelineCreateInfo {
                    stages: stages.into_iter().collect(),
                    vertex_input_state: Some(vertex_input_state),
                    input_assembly_state: Some(InputAssemblyState::default()),
                    viewport_state: Some(ViewportState::default()),
                    rasterization_state: Some(RasterizationState {
                        cull_mode: CullMode::Back,
                        ..Default::default()
                    }),
                    multisample_state: Some(MultisampleState::default()),
                    color_blend_state: Some(ColorBlendState::with_attachment_states(
                        subpass.num_color_attachments(),
                        ColorBlendAttachmentState {
                            color_write_mask: ColorComponents::all(),
                            ..Default::default()
                        },
                    )),
                    depth_stencil_state: Some(DepthStencilState {
                        depth: Some(DepthState {
                            compare_op: CompareOp::LessOrEqual,
                            write_enable: true,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    dynamic_state: [DynamicState::Viewport].into_iter().collect(),
                    subpass: Some(subpass.into()),
                    ..GraphicsPipelineCreateInfo::layout(pipeline_layout)
                },
            )
            .unwrap()
        };

        let frame_buffer = recreate_framebuffer(&gbuffer_textures, &render_pass);

        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: vulkano_window_renderer.window_size().into(),
            depth_range: 0.0..=1.0,
        };

        let memory_allocator = VULKAN.memory_allocator().clone();

        let gpu_materials_buffer = Buffer::from_iter(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            gpu_materials,
        )
        .unwrap();

        let texture_descriptor_writes =
            WriteDescriptorSet::image_view_sampler_array(0, 0, textures);
        let gpu_materials_writes = WriteDescriptorSet::buffer(0, gpu_materials_buffer);

        let layout = &pipeline.layout().set_layouts()[0];
        let texture_descriptor_set = DescriptorSet::new(
            descriptor_set_alloc.clone(),
            layout.clone(),
            [texture_descriptor_writes],
            [],
        )
        .unwrap();

        let layout = &pipeline.layout().set_layouts()[1];
        let material_descriptor_set = DescriptorSet::new(
            descriptor_set_alloc.clone(),
            layout.clone(),
            [gpu_materials_writes],
            [],
        )
        .unwrap();

        let uniform_buffer_allocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        tracing::info!("Render - Render pass GBuffer Pass successfully created.");

        GBufferPass {
            gbuffer_renderpass: render_pass,
            gbuffer_framebuffer: frame_buffer,
            gbuffer_pipeline: pipeline,
            gbuffer_textures: Arc::new(gbuffer_textures),
            texture_descriptor_set,
            material_descriptor_set,
            uniform_buffer: uniform_buffer_allocator,

            viewport,
        }
    }
}

impl BufferPassTrait for GBufferPass {
    fn render(
        &mut self,
        _image_idx: u32,
        world: &World,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        let camera = world.resource::<Camera>();

        let uniform_buffer_subbuffer = {
            let uniform_data = vs::UBO_projview {
                projection: camera.projection().to_cols_array_2d(),
                view: camera.view().to_cols_array_2d(),
                prev_view: camera.prev_view().to_cols_array_2d(),
            };

            let subbuffer = self.uniform_buffer.allocate_sized().unwrap();
            *subbuffer.write().unwrap() = uniform_data;

            subbuffer
        };

        let layout = &self.gbuffer_pipeline.layout().set_layouts()[2];
        let descriptor_set = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            layout.clone(),
            [WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)],
            [],
        )
        .unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![
                        Some(ClearValue::Float([0.0, 0.0, 0.0, 1.0])),
                        Some(ClearValue::Float([0.0, 0.0, 0.0, 1.0])),
                        Some(ClearValue::Float([0.0, 0.0, 0.0, 1.0])),
                        Some(ClearValue::Float([0.0, 0.0, 0.0, 1.0])),
                        Some(ClearValue::Float([1.0, 0.0, 0.0, 1.0])),
                        Some(ClearValue::Float([0.0, 0.0, 0.0, 1.0])),
                        Some(ClearValue::Depth(1.0)),
                    ],
                    render_pass: self.gbuffer_renderpass.clone(),
                    ..RenderPassBeginInfo::framebuffer(self.gbuffer_framebuffer.clone())
                },
                Default::default(),
            )
            .unwrap()
            .bind_pipeline_graphics(self.gbuffer_pipeline.clone())
            .unwrap()
            .set_viewport(0, [self.viewport.clone()].into_iter().collect())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.gbuffer_pipeline.layout().clone(),
                0,
                vec![
                    self.texture_descriptor_set.clone(),
                    self.material_descriptor_set.clone(),
                    descriptor_set,
                ],
            )
            .unwrap();

        for entity in world.iter_entities().into_iter() {
            if entity.get::<Model>().is_some() {
                let transform = entity.get::<Transform>().unwrap();
                let model = entity.get::<Model>().unwrap();

                for mesh in &model.meshes {
                    let world_matrix = transform.model_matrix() * mesh.world;
                    let normal_matrix = world_matrix.clone().inverse().transpose();
                    builder
                        .push_constants(
                            self.gbuffer_pipeline.layout().clone(),
                            0,
                            vs::PushConsts {
                                world: world_matrix.to_cols_array_2d(),
                                mat_index: mesh.gpu_mat_index.into(),
                                normal_matrix: normal_matrix.to_cols_array_2d(),
                                pad: [0, 0, 0],
                            },
                        )
                        .unwrap()
                        .bind_vertex_buffers(0, mesh.primitive.vertex_buffer.clone())
                        .unwrap()
                        .bind_index_buffer(mesh.primitive.index_buffer.clone())
                        .unwrap();

                    unsafe {
                        builder
                            .draw_indexed(mesh.primitive.index_buffer.len() as u32, 1, 0, 0, 0)
                            .unwrap();
                    }
                }
            }
        }

        builder.end_render_pass(Default::default()).unwrap();
    }

    fn on_swapchain_recreate(
        &mut self,
        _swapchain_images: &[Arc<ImageView>],
        window_size: PhysicalSize<u32>,
    ) {
        // TODO: See why this causes not rendering problems.
        // let new_gbuffer_textures =
        //     GBufferTextures::new([window_size.width as f32, window_size.height as f32]);
        // self.gbuffer_textures = new_gbuffer_textures;
        // self.gbuffer_framebuffer =
        //     recreate_framebuffer(&self.gbuffer_textures, &self.gbuffer_renderpass);

        self.viewport.extent = [window_size.width as f32, window_size.height as f32];
    }
}

fn recreate_framebuffer(
    gbuffer_textures: &GBufferTextures,
    render_pass: &Arc<RenderPass>,
) -> Arc<Framebuffer> {
    Framebuffer::new(
        render_pass.clone(),
        FramebufferCreateInfo {
            // Attach the offscreen image to the framebuffer.
            attachments: vec![
                gbuffer_textures.base_color.image_view.clone(),
                gbuffer_textures.position.image_view.clone(),
                gbuffer_textures.normal.image_view.clone(),
                gbuffer_textures.emissive.image_view.clone(),
                gbuffer_textures.pbr.image_view.clone(),
                gbuffer_textures.velocity.image_view.clone(),
                gbuffer_textures.depth.image_view.clone(),
            ],
            ..Default::default()
        },
    )
    .unwrap()
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../resources/shaders/gbuffer.vert",
        vulkan_version: "1.2",
        spirv_version: "1.5",
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../resources/shaders/gbuffer.frag",
        vulkan_version: "1.2",
        spirv_version: "1.5",
    }
}

fn add_default_textures(
    world: &World,
    default_diffuse_map_index: &mut u32,
    default_normal_map_index: &mut u32,
    default_occlusion_map_index: &mut u32,
    default_metallic_roughness_map_index: &mut u32,
    default_black_map_index: &mut u32,
    textures: &mut Vec<(Arc<ImageView>, Arc<Sampler>)>,
    sampler: &Arc<Sampler>,
    next_bindless_image_index: &mut u32,
) {
    let default_textures = world.resource::<DefaultTextures>();
    *default_diffuse_map_index = add_bindless_texture(
        textures,
        &default_textures.diffuse,
        sampler,
        next_bindless_image_index,
    );
    *default_normal_map_index = add_bindless_texture(
        textures,
        &default_textures.normal,
        sampler,
        next_bindless_image_index,
    );
    *default_occlusion_map_index = add_bindless_texture(
        textures,
        &default_textures.occlusion,
        sampler,
        next_bindless_image_index,
    );
    *default_metallic_roughness_map_index = add_bindless_texture(
        textures,
        &default_textures.metallic_roughness,
        sampler,
        next_bindless_image_index,
    );
    *default_black_map_index = add_bindless_texture(
        textures,
        &default_textures.black,
        sampler,
        next_bindless_image_index,
    );
}

fn add_model(
    gpu_materials: &mut Vec<vs::GpuMaterial>,
    model: &mut Model,
    default_diffuse_map_index: u32,
    default_normal_map_index: u32,
    default_occlusion_map_index: u32,
    default_metallic_roughness_map_index: u32,
    default_black_map_index: u32,
    textures: &mut Vec<(Arc<ImageView>, Arc<Sampler>)>,
    sampler: &Arc<Sampler>,
    next_bindless_image_index: &mut u32,
) {
    // Add the images from the new model to the bindless descriptor set and
    // also update the mappings for each primitive to be indexes corresponding
    // to the ordering in the bindless descriptor set texture array.
    for mesh in &mut model.meshes {
        let diffuse_bindless_index = match mesh.material.base_color_map {
            DEFAULT_TEXTURE_MAP => default_diffuse_map_index,
            _ => add_bindless_texture(
                textures,
                &model.textures[mesh.material.base_color_map as usize],
                sampler,
                next_bindless_image_index,
            ),
        };

        let normal_bindless_index = match mesh.material.normal_map {
            DEFAULT_TEXTURE_MAP => default_normal_map_index,
            _ => add_bindless_texture(
                textures,
                &model.textures[mesh.material.normal_map as usize],
                sampler,
                next_bindless_image_index,
            ),
        };

        let metallic_roughness_bindless_index = match mesh.material.metallic_roughness_map {
            DEFAULT_TEXTURE_MAP => default_metallic_roughness_map_index,
            _ => add_bindless_texture(
                textures,
                &model.textures[mesh.material.metallic_roughness_map as usize],
                sampler,
                next_bindless_image_index,
            ),
        };

        let occlusion_bindless_index = match mesh.material.occlusion_map {
            DEFAULT_TEXTURE_MAP => default_occlusion_map_index,
            _ => add_bindless_texture(
                textures,
                &model.textures[mesh.material.occlusion_map as usize],
                sampler,
                next_bindless_image_index,
            ),
        };

        let emissive_bindless_index = match mesh.material.emissive_map {
            DEFAULT_TEXTURE_MAP => default_black_map_index,
            _ => add_bindless_texture(
                textures,
                &model.textures[mesh.material.emissive_map as usize],
                sampler,
                next_bindless_image_index,
            ),
        };

        let material_index = add_material(
            gpu_materials,
            vs::GpuMaterial {
                base_color_map: diffuse_bindless_index,
                normal_map: normal_bindless_index,
                metallic_roughness_map: metallic_roughness_bindless_index,
                occlusion_map: occlusion_bindless_index,
                emissive_map: emissive_bindless_index,
                // UV sets
                base_color_uv_set: mesh.material.base_color_uv_set,
                normal_uv_set: mesh.material.normal_uv_set,
                metallic_roughness_uv_set: mesh.material.metallic_roughness_uv_set,
                occlusion_uv_set: mesh.material.occlusion_uv_set,
                emissive_uv_set: mesh.material.emissive_uv_set,
                padding: [0.0; 2],
                // Factors
                base_color_factor: mesh.material.base_color_factor.into(),
                emissive_factor: [
                    mesh.material.emissive_factor.x,
                    mesh.material.emissive_factor.y,
                    mesh.material.emissive_factor.z,
                    1.0,
                ],
                metallic_factor: mesh.material.metallic_factor,
                roughness_factor: mesh.material.roughness_factor,
                // alpha
                alpha_mode: mesh.material.alpha_mode as u32,
                alpha_cutoff: mesh.material.alpha_cutoff,
                raytrace_properties: [
                    mesh.material.material_type as f32,
                    mesh.material.material_property,
                    0.0,
                    0.0,
                ],
            },
        );

        mesh.gpu_mat_index = material_index;
    }
}

fn add_bindless_texture(
    textures: &mut Vec<(Arc<ImageView>, Arc<Sampler>)>,
    texture: &Texture,
    sampler: &Arc<Sampler>,
    next_bindless_image_index: &mut u32,
) -> u32 {
    let index = *next_bindless_image_index;
    textures.push((texture.image_view.clone(), sampler.clone()));

    *next_bindless_image_index += 1;

    index
}

fn add_material(gpu_materials: &mut Vec<vs::GpuMaterial>, gpu_material: vs::GpuMaterial) -> u32 {
    let material_index = gpu_materials.len() as u32;
    gpu_materials.push(gpu_material);

    material_index
}
