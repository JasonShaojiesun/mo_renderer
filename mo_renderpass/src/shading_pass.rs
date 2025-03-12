use crate::{BufferPassTrait, GBufferTextures};
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use mo_ecs::resource::{GlobalSamplers, IBLResource};
use mo_ecs::{component::DirectionalLight, resource::Camera};
use mo_vk::{Texture, TextureCreateInfo, VulkanoWindowRenderer, VULKAN};
use std::{cell::RefCell, sync::Arc};
use vulkano::pipeline::graphics::depth_stencil::{CompareOp, DepthState, DepthStencilState};
use vulkano::{
    buffer::{
        allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo}, Buffer, BufferCreateInfo,
        BufferUsage,
    },
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderPassBeginInfo},
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, layout::{
            DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo,
            DescriptorType,
        },
        DescriptorSet,
        WriteDescriptorSet,
    },
    format::{ClearValue, Format},
    image::{sampler::Sampler, view::ImageView, ImageUsage},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    pipeline::{
        graphics::{
            color_blend::{ColorBlendAttachmentState, ColorBlendState, ColorComponents},
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            rasterization::RasterizationState,
            viewport::Viewport,
            GraphicsPipelineCreateInfo,
        }, layout::PipelineLayoutCreateInfo, DynamicState, GraphicsPipeline, Pipeline,
        PipelineBindPoint,
        PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    shader::ShaderStages,
};
use winit::dpi::PhysicalSize;

pub struct ShadingPass {
    renderpass: Arc<RenderPass>,
    framebuffer: Arc<Framebuffer>,
    pub output_image: Arc<Texture>,
    shading_pipeline: Arc<GraphicsPipeline>,
    uniform_buffer_allocator: SubbufferAllocator,
    light_descriptor_set: Arc<DescriptorSet>,
    gbuffer_images_descriptor_set: Arc<DescriptorSet>,
    num_lights: u32,
    viewport: Viewport,

    // extra textures
    environment_map: Arc<Texture>,
    depth: Arc<Texture>,
    sampler: Arc<Sampler>,

    skybox_pipeline: Arc<GraphicsPipeline>,
}

impl ShadingPass {
    pub fn new(
        world: &RefCell<World>,
        gbuffer_textures: &Arc<GBufferTextures>,
        shadow_map: &Arc<Texture>,
        ssao_texture: &Arc<Texture>,
        renderer: &VulkanoWindowRenderer,
        descriptor_set_alloc: Arc<StandardDescriptorSetAllocator>,
    ) -> Self {
        let final_output_info = TextureCreateInfo {
            format: Format::R8G8B8A8_UNORM,
            extent: [
                renderer.window_size()[0] as u32,
                renderer.window_size()[1] as u32,
                1,
            ],
            usage: ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::SAMPLED
                | ImageUsage::INPUT_ATTACHMENT,
            ..Default::default()
        };
        let output_image = Arc::new(Texture::new(final_output_info));

        let environment_map = world
            .borrow()
            .resource::<IBLResource>()
            .environment_map
            .clone();

        let renderpass = vulkano::ordered_passes_renderpass!(
            VULKAN.device().clone(),
            attachments: {
                final_output: {
                    format: output_image.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                depth: {
                    format: gbuffer_textures.depth.image_view.format(),
                    samples: 1,
                    load_op: Load,
                    store_op: Store,
                }
            },
            passes:[
                {
                    color: [final_output],
                    depth_stencil: {depth},
                    input: []
                },
                {
                    color: [final_output],
                    depth_stencil: {depth},
                    input: []
                }
            ]
        )
        .unwrap();

        let shading_pipeline = create_shading_pipeline(&renderpass);
        let skybox_pipeline = create_skybox_pipeline(&renderpass);

        let (light_descriptor_set, num_lights) =
            create_light_descriptor_set(world, &descriptor_set_alloc, &shading_pipeline);

        let framebuffer =
            recreate_shading_framebuffer(&output_image, &gbuffer_textures.depth, &renderpass);

        let uniform_buffer_allocator = SubbufferAllocator::new(
            VULKAN.memory_allocator().clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: renderer.window_size(),
            depth_range: 0.0..=1.0,
        };

        let sampler = world.borrow().resource::<GlobalSamplers>().wrap.clone();

        let gbuffer_images_descriptor_set = create_textures_descriptor_set(
            world,
            &descriptor_set_alloc,
            &shading_pipeline,
            gbuffer_textures,
            shadow_map,
            ssao_texture,
            &sampler,
        );

        ShadingPass {
            renderpass,
            framebuffer,
            shading_pipeline,
            output_image,
            uniform_buffer_allocator,
            viewport,
            light_descriptor_set,
            gbuffer_images_descriptor_set,
            num_lights,

            environment_map,
            depth: gbuffer_textures.depth.clone(),
            sampler,

            skybox_pipeline,
        }
    }
}

impl BufferPassTrait for ShadingPass {
    fn render(
        &mut self,
        _image_idx: u32,
        world: &World,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        let mut light_proj_view: Mat4 = Mat4::IDENTITY;
        for entity in world.iter_entities().into_iter() {
            if entity
                .get::<DirectionalLight>()
                .is_some_and(|light| light.is_shadow_caster)
            {
                let light = entity.get::<DirectionalLight>().unwrap();
                light_proj_view = light.proj_view();
                break;
            }
        }

        let camera = world.resource::<Camera>();
        let uniform_buffer_subbuffer = {
            let uniform_data = shading_fs::UBO_view {
                proj_view: camera.projection_view().to_cols_array_2d(),
                eye_pos: camera.position().into(),
                inverse_projection: camera.inverse_projection().to_cols_array_2d(),
                inverse_view: camera.inverse_view().to_cols_array_2d(),
                light_proj_view: light_proj_view.to_cols_array_2d(),
                num_lights: self.num_lights,
            };

            let subbuffer = self.uniform_buffer_allocator.allocate_sized().unwrap();
            *subbuffer.write().unwrap() = uniform_data;

            subbuffer
        };

        let layout2 = &self.shading_pipeline.layout().set_layouts()[2];
        let descriptor_set_2 = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            layout2.clone(),
            [WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)],
            [],
        )
        .unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some(ClearValue::Float([0.0, 0.0, 0.0, 1.0])), None],
                    ..RenderPassBeginInfo::framebuffer(self.framebuffer.clone())
                },
                Default::default(),
            )
            .unwrap()
            .bind_pipeline_graphics(self.shading_pipeline.clone())
            .unwrap()
            .set_viewport(0, [self.viewport.clone()].into_iter().collect())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.shading_pipeline.layout().clone(),
                0,
                vec![
                    self.light_descriptor_set.clone(),
                    self.gbuffer_images_descriptor_set.clone(),
                    descriptor_set_2,
                ],
            )
            .unwrap();

        unsafe { builder.draw(4, 1, 0, 0) }.unwrap();

        builder
            .next_subpass(Default::default(), Default::default())
            .unwrap();

        let uniform_buffer_subbuffer2 = {
            let mut near_plane_width_height = Vec2::ZERO;

            // Remember that in our camera component, fov is in degrees
            near_plane_width_height.y = 2.0 * camera.near_p() * (camera.fov() / 2.0).tan();
            near_plane_width_height.x = camera.aspect() * near_plane_width_height.y;

            let uniform_data = skybox_fs::cameraDataUBO {
                direction: Vec4::from((camera.direction(), 1.0)).into(),
                right: Vec4::from((camera.right(), 1.0)).into(),
                up: Vec4::from((camera.up(), camera.near_p())).into(),
                nearWidthHeight: near_plane_width_height.into(),
                viewportWidthHeight: [self.viewport.extent[0], self.viewport.extent[1]],
            };

            let subbuffer = self.uniform_buffer_allocator.allocate_sized().unwrap();
            *subbuffer.write().unwrap() = uniform_data;

            subbuffer
        };

        let skybox_layout = &self.skybox_pipeline.layout().set_layouts()[0];
        let descriptor_set_skybox = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            skybox_layout.clone(),
            [
                WriteDescriptorSet::image_view_sampler(
                    0,
                    self.environment_map.image_view.clone(),
                    self.sampler.clone(),
                ),
                WriteDescriptorSet::buffer(1, uniform_buffer_subbuffer2),
            ],
            [],
        )
        .unwrap();

        builder
            .bind_pipeline_graphics(self.skybox_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.skybox_pipeline.layout().clone(),
                0,
                vec![descriptor_set_skybox],
            )
            .unwrap();

        unsafe { builder.draw(4, 1, 0, 0) }.unwrap();

        builder.end_render_pass(Default::default()).unwrap();
    }

    fn on_swapchain_recreate(
        &mut self,
        _swapchain_images: &[Arc<ImageView>],
        _window_size: PhysicalSize<u32>,
    ) {
        self.framebuffer =
            recreate_shading_framebuffer(&self.output_image, &self.depth, &self.renderpass);
        self.viewport = Viewport {
            offset: [0.0, 0.0],
            extent: _window_size.into(),
            depth_range: 0.0..=1.0,
        };
    }
}

mod shading_vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../resources/shaders/fullscreen.vert"
    }
}

mod shading_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../resources/shaders/shading_pass.frag",
        vulkan_version: "1.2",
        spirv_version: "1.5",
    }
}

mod skybox_vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
        #version 450

        layout (location = 0) out vec2 o_screenUv;

        vec2 positions[4] =
        vec2[](vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0), vec2(1.0, 1.0));

        // 修改纹理坐标，使其从左下角开始
        vec2 texCoords[4] =
        vec2[](vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(1.0, 1.0));

        void main() {
            gl_Position = vec4(positions[gl_VertexIndex], 1.0, 1.0);
            o_screenUv = texCoords[gl_VertexIndex];
        }
        "
    }
}

mod skybox_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
        #version 450

        layout (set = 0, binding = 0) uniform samplerCube cubeMapTexture;

        // Be careful to Vulkan's UBO's struct && vec3 alignment.
        layout (set = 0, binding = 1) uniform cameraDataUBO {
            vec4 direction;
            vec4 right;
            vec4 up; // The up vector. The w component is the distance to the near plane.
            vec2 nearWidthHeight; // The height, width and distance of the camera's near plane -- |z|.
            vec2 viewportWidthHeight; // The width and height of the viewport.
        } cameraData;

        layout (location = 0) in vec2 inScreenUV;
        layout (location = 0) out vec4 outColor;

        void main() {
            float x = inScreenUV.x * 2.0 - 1.0;
            float y = inScreenUV.y * 2.0 - 1.0;

            float nearWidth = cameraData.nearWidthHeight.x;
            float nearHeight = cameraData.nearWidthHeight.y;

            vec3 sampleDir =
            x * (nearWidth / 2.0) * cameraData.right.xyz
            + (y * (nearHeight / 2.0)) * cameraData.up.xyz
            + -cameraData.direction.xyz * cameraData.up.w;

            sampleDir = normalize(sampleDir);

            outColor = texture(cubeMapTexture, sampleDir);
        }
        ",
    }
}

fn recreate_shading_framebuffer(
    output_image: &Arc<Texture>,
    depth: &Arc<Texture>,
    render_pass: &Arc<RenderPass>,
) -> Arc<Framebuffer> {
    Framebuffer::new(
        render_pass.clone(),
        FramebufferCreateInfo {
            // Attach the offscreen image to the framebuffer.
            attachments: vec![output_image.image_view.clone(), depth.image_view.clone()],
            ..Default::default()
        },
    )
    .unwrap()
}

fn create_shading_pipeline(renderpass: &Arc<RenderPass>) -> Arc<GraphicsPipeline> {
    let vs = shading_vs::load(VULKAN.device().clone())
        .unwrap()
        .entry_point("main")
        .unwrap();

    let fs = shading_fs::load(VULKAN.device().clone())
        .unwrap()
        .entry_point("main")
        .unwrap();

    let stages = [
        PipelineShaderStageCreateInfo::new(vs.clone()),
        PipelineShaderStageCreateInfo::new(fs.clone()),
    ];

    let pipeline_layout = PipelineLayout::new(
        VULKAN.device().clone(),
        PipelineLayoutCreateInfo {
            set_layouts: vec![
                // Light descriptor set. Number 0.
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [(
                            0,
                            DescriptorSetLayoutBinding {
                                stages: ShaderStages::FRAGMENT,
                                descriptor_count: VULKAN.max_per_stage_descriptor_storage_buffers(),
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
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [
                            (
                                0,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                1,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                2,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                3,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                4,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                5,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                6,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                7,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                8,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                9,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                10,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                11,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
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
                // Uniform Descriptor set.  Number 2.
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [(
                            0,
                            DescriptorSetLayoutBinding {
                                stages: ShaderStages::FRAGMENT,
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
            ..Default::default()
        },
    )
    .unwrap();

    let subpass = Subpass::from(renderpass.clone(), 0).unwrap();

    GraphicsPipeline::new(
        VULKAN.device().clone(),
        None,
        GraphicsPipelineCreateInfo {
            stages: stages.into_iter().collect(),
            vertex_input_state: Some(Default::default()),
            input_assembly_state: Some(InputAssemblyState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            }),
            rasterization_state: Some(RasterizationState::default()),
            color_blend_state: Some(ColorBlendState::with_attachment_states(
                subpass.num_color_attachments(),
                ColorBlendAttachmentState {
                    color_write_mask: ColorComponents::all(),
                    ..Default::default()
                },
            )),
            depth_stencil_state: Some(DepthStencilState {
                depth: Some(DepthState {
                    write_enable: false,
                    compare_op: CompareOp::Greater,
                }),
                ..Default::default()
            }),
            viewport_state: Some(Default::default()),
            multisample_state: Some(Default::default()),
            dynamic_state: [DynamicState::Viewport].into_iter().collect(),
            subpass: Some(subpass.into()),
            ..GraphicsPipelineCreateInfo::layout(pipeline_layout)
        },
    )
    .unwrap()
}

fn create_light_descriptor_set(
    world: &RefCell<World>,
    descriptor_set_alloc: &Arc<StandardDescriptorSetAllocator>,
    pipeline: &Arc<GraphicsPipeline>,
) -> (Arc<DescriptorSet>, u32) {
    let mut gpu_lights: Vec<shading_fs::GpuLight> = Vec::new();

    let mut total_light_num = 0;

    let mut query = world.borrow_mut().query::<&DirectionalLight>();
    for light in query.iter(&world.borrow()) {
        gpu_lights.push(shading_fs::GpuLight {
            type_range_spot_id: [0.0, 0.0, 0.0, 0.0],
            position: [
                light.transform.translation.x,
                light.transform.translation.y,
                light.transform.translation.z,
                0.0,
            ],
            color: [light.color.x, light.color.y, light.color.z, 0.0],
            direction: [
                light.transform.direction().x,
                light.transform.direction().y,
                light.transform.direction().z,
                0.0,
            ],
            attenuation: [
                light.intensity,
                light.intensity,
                light.intensity,
                light.intensity,
            ],
        });

        total_light_num += 1;
    }

    let memory_allocator = VULKAN.memory_allocator().clone();

    let gpu_light_buffer = Buffer::from_iter(
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
        gpu_lights,
    )
    .unwrap();

    let gpu_light_writes = WriteDescriptorSet::buffer(0, gpu_light_buffer);
    let layout = &pipeline.layout().set_layouts()[0];

    let set = DescriptorSet::new(
        descriptor_set_alloc.clone(),
        layout.clone(),
        [gpu_light_writes],
        [],
    )
    .unwrap();

    (set, total_light_num)
}

fn create_textures_descriptor_set(
    world: &RefCell<World>,
    descriptor_set_alloc: &Arc<StandardDescriptorSetAllocator>,
    shading_pipeline: &Arc<GraphicsPipeline>,
    gbuffer_textures: &GBufferTextures,
    shadow_map: &Arc<Texture>,
    ssao_texture: &Arc<Texture>,
    sampler: &Arc<Sampler>,
) -> Arc<DescriptorSet> {
    let irradiance_map = world
        .borrow()
        .resource::<IBLResource>()
        .irradiance_map
        .clone();
    let prefiltered_map = world
        .borrow()
        .resource::<IBLResource>()
        .specular_map
        .clone();
    let brdf_lut = world.borrow().resource::<IBLResource>().brdf_lut.clone();

    let layout1 = &shading_pipeline.layout().set_layouts()[1];
    DescriptorSet::new(
        descriptor_set_alloc.clone(),
        layout1.clone(),
        [
            WriteDescriptorSet::image_view_sampler(
                0,
                gbuffer_textures.base_color.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                1,
                gbuffer_textures.normal.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                2,
                gbuffer_textures.emissive.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                3,
                gbuffer_textures.pbr.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                4,
                gbuffer_textures.position.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                5,
                gbuffer_textures.velocity.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                6,
                shadow_map.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                7,
                ssao_texture.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                8,
                irradiance_map.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                9,
                prefiltered_map.image_view.clone(),
                sampler.clone(),
            ),
            WriteDescriptorSet::image_view_sampler(
                10,
                brdf_lut.image_view.clone(),
                sampler.clone(),
            ),
        ],
        [],
    )
    .unwrap()
}

fn create_skybox_pipeline(renderpass: &Arc<RenderPass>) -> Arc<GraphicsPipeline> {
    let vs = skybox_vs::load(VULKAN.device().clone())
        .unwrap()
        .entry_point("main")
        .unwrap();

    let fs = skybox_fs::load(VULKAN.device().clone())
        .unwrap()
        .entry_point("main")
        .unwrap();

    let stages = [
        PipelineShaderStageCreateInfo::new(vs.clone()),
        PipelineShaderStageCreateInfo::new(fs.clone()),
    ];

    let pipeline_layout = PipelineLayout::new(
        VULKAN.device().clone(),
        PipelineLayoutCreateInfo {
            set_layouts: vec![
                // Light descriptor set. Number 0.
                DescriptorSetLayout::new(
                    VULKAN.device().clone(),
                    DescriptorSetLayoutCreateInfo {
                        bindings: [
                            (
                                0,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::CombinedImageSampler,
                                    )
                                },
                            ),
                            (
                                1,
                                DescriptorSetLayoutBinding {
                                    stages: ShaderStages::FRAGMENT,
                                    descriptor_count: 1,
                                    ..DescriptorSetLayoutBinding::descriptor_type(
                                        DescriptorType::UniformBuffer,
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

    let subpass = Subpass::from(renderpass.clone(), 1).unwrap();

    GraphicsPipeline::new(
        VULKAN.device().clone(),
        None,
        GraphicsPipelineCreateInfo {
            stages: stages.into_iter().collect(),
            vertex_input_state: Some(Default::default()),
            input_assembly_state: Some(InputAssemblyState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            }),
            rasterization_state: Some(RasterizationState::default()),
            color_blend_state: Some(ColorBlendState::with_attachment_states(
                subpass.num_color_attachments(),
                ColorBlendAttachmentState {
                    color_write_mask: ColorComponents::all(),
                    ..Default::default()
                },
            )),
            depth_stencil_state: Some(DepthStencilState {
                depth: Some(DepthState {
                    write_enable: false,
                    compare_op: CompareOp::Equal,
                }),
                ..Default::default()
            }),
            viewport_state: Some(Default::default()),
            multisample_state: Some(Default::default()),
            dynamic_state: [DynamicState::Viewport].into_iter().collect(),
            subpass: Some(subpass.into()),
            ..GraphicsPipelineCreateInfo::layout(pipeline_layout)
        },
    )
    .unwrap()
}
