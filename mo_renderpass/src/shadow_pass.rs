use crate::RenderPassTrait;
use bevy_ecs::prelude::*;
use bevy_math::Mat4;
use mo_ecs::component::{DirectionalLight, Transform};
use mo_ecs::model::{Model, StaticVertex};
use mo_vk::{Texture, TextureCreateInfo, VULKAN};
use std::sync::Arc;
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::buffer::BufferUsage;
use vulkano::descriptor_set::layout::{
    DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateInfo, DescriptorType,
};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::format::Format;
use vulkano::memory::allocator::MemoryTypeFilter;
use vulkano::pipeline::graphics::depth_stencil::CompareOp;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::rasterization::{CullMode, DepthBiasState};
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::layout::PushConstantRange;
use vulkano::pipeline::{Pipeline, PipelineBindPoint};
use vulkano::render_pass::{FramebufferCreateInfo, Subpass};
use vulkano::shader::ShaderStages;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderPassBeginInfo},
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    format::ClearValue,
    image::{view::ImageView, ImageUsage},
    pipeline::{
        graphics::{
            depth_stencil::{DepthState, DepthStencilState},
            rasterization::RasterizationState,
            vertex_input::{Vertex, VertexDefinition},
            viewport::Viewport,
            GraphicsPipelineCreateInfo,
        }, layout::PipelineLayoutCreateInfo, GraphicsPipeline,
        PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    render_pass::{Framebuffer, RenderPass},
};
use winit::dpi::PhysicalSize;

pub struct ShadowPass {
    renderpass: Arc<RenderPass>,
    framebuffer: Arc<Framebuffer>,
    pipeline: Arc<GraphicsPipeline>,
    pub shadow_map: Arc<Texture>,
    uniform_buffer_allocator: SubbufferAllocator,
}

impl ShadowPass {
    pub fn new() -> Self {
        let shadow_map_info = TextureCreateInfo {
            format: Format::D32_SFLOAT,
            extent: [2048, 2048, 1],
            usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
            ..Default::default()
        };

        let shadow_map = Arc::new(Texture::new(shadow_map_info));

        let renderpass = vulkano::single_pass_renderpass!(
            VULKAN.device().clone(),
            attachments: {
                depth: {
                    format: shadow_map.image_view.format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                }
            },
            pass: {
                color: [],
                depth_stencil: {depth},
            }
        )
        .unwrap();

        let pipeline = {
            let vs = vs::load(VULKAN.device().clone())
                .unwrap()
                .entry_point("main")
                .unwrap();

            let vertex_input_state = [StaticVertex::per_vertex()].definition(&vs).unwrap();
            let stages = [PipelineShaderStageCreateInfo::new(vs.clone())];

            let pipeline_layout = PipelineLayout::new(
                VULKAN.device().clone(),
                PipelineLayoutCreateInfo {
                    set_layouts: vec![
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
                        stages: ShaderStages::VERTEX,
                        offset: 0,
                        size: size_of::<vs::PushConsts>() as u32,
                    }],
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
                    vertex_input_state: Some(vertex_input_state),
                    input_assembly_state: Some(InputAssemblyState::default()),
                    rasterization_state: Some(RasterizationState {
                        cull_mode: CullMode::None,
                        depth_bias: Some(DepthBiasState {
                            constant_factor: 2.0, // 根据情况调整
                            clamp: 0.0,
                            slope_factor: 2.0,
                        }),
                        ..Default::default()
                    }),
                    color_blend_state: None,
                    depth_stencil_state: Some(DepthStencilState {
                        depth: Some(DepthState {
                            compare_op: CompareOp::LessOrEqual,
                            write_enable: true,
                        }),
                        ..Default::default()
                    }),
                    viewport_state: Some(ViewportState {
                        viewports: [Viewport {
                            offset: [0.0, 0.0],
                            extent: [2048.0, 2048.0],
                            depth_range: 0.0..=1.0,
                        }]
                        .into(),
                        ..Default::default()
                    }),
                    multisample_state: Some(Default::default()),
                    subpass: Some(subpass.into()),
                    ..GraphicsPipelineCreateInfo::layout(pipeline_layout)
                },
            )
            .unwrap()
        };

        let framebuffer = recreate_shadow_framebuffer(&shadow_map, &renderpass);

        let uniform_buffer_allocator = SubbufferAllocator::new(
            VULKAN.memory_allocator().clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        ShadowPass {
            renderpass,
            framebuffer,
            pipeline,
            shadow_map,
            uniform_buffer_allocator,
        }
    }
}

impl RenderPassTrait for ShadowPass {
    fn render(
        &mut self,
        _image_idx: u32,
        world: &World,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        let mut proj_view: Mat4 = Mat4::IDENTITY;
        for entity in world.iter_entities().into_iter() {
            if entity
                .get::<DirectionalLight>()
                .is_some_and(|light| light.is_shadow_caster)
            {
                let light = entity.get::<DirectionalLight>().unwrap();
                proj_view = light.proj_view();
                break;
            }
        }

        let uniform_buffer_subbuffer = {
            let uniform_data = vs::UBO_projview {
                proj_view: proj_view.to_cols_array_2d(),
            };

            let subbuffer = self.uniform_buffer_allocator.allocate_sized().unwrap();
            *subbuffer.write().unwrap() = uniform_data;

            subbuffer
        };

        let layout = &self.pipeline.layout().set_layouts()[0];
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
                    clear_values: vec![Some(ClearValue::Depth(1.0))],
                    ..RenderPassBeginInfo::framebuffer(self.framebuffer.clone())
                },
                Default::default(),
            )
            .unwrap()
            .bind_pipeline_graphics(self.pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                vec![descriptor_set],
            )
            .unwrap();

        // 绘制逻辑...
        for entity in world.iter_entities().into_iter() {
            if entity.get::<Model>().is_some() {
                let transform = entity.get::<Transform>().unwrap();
                let model = entity.get::<Model>().unwrap();

                for mesh in &model.meshes {
                    builder
                        .push_constants(
                            self.pipeline.layout().clone(),
                            0,
                            vs::PushConsts {
                                model: (transform.model_matrix() * mesh.world).to_cols_array_2d(),
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
        _window_size: PhysicalSize<u32>,
    ) {
        self.framebuffer = recreate_shadow_framebuffer(&self.shadow_map, &self.renderpass);
    }
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
        #version 460
        layout (location = 0) in vec4 position;
        layout (location = 1) in vec4 normal;
        layout (location = 2) in vec4 color;
        layout (location = 3) in vec2 uv0;
        layout (location = 4) in vec2 uv1;
        layout (location = 5) in vec4 tangent;

        layout (std140, set = 0, binding = 0) uniform UBO_projview
        {
            mat4 proj_view;
        } shadowProjview;

        layout (push_constant) uniform PushConsts {
            mat4 model;
        } pushConsts;

        void main() {
            gl_Position = shadowProjview.proj_view * pushConsts.model * vec4(position.xyz, 1.0);
        }
        "
    }
}

fn recreate_shadow_framebuffer(
    shadow_map: &Arc<Texture>,
    render_pass: &Arc<RenderPass>,
) -> Arc<Framebuffer> {
    Framebuffer::new(
        render_pass.clone(),
        FramebufferCreateInfo {
            // Attach the offscreen image to the framebuffer.
            attachments: vec![shadow_map.image_view.clone()],
            ..Default::default()
        },
    )
    .unwrap()
}
