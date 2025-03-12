use crate::BufferPassTrait;
use bevy_ecs::prelude::World;
use mo_ecs::resource::GlobalSamplers;
use mo_vk::{Texture, VulkanoWindowRenderer, VULKAN};
use std::cell::RefCell;
use std::sync::Arc;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, RenderPassBeginInfo},
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, DescriptorSet, WriteDescriptorSet,
    },
    image::view::ImageView,
    pipeline::{
        graphics::{
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            multisample::MultisampleState,
            rasterization::RasterizationState,
            viewport::{Viewport, ViewportState},
            GraphicsPipelineCreateInfo,
        }, layout::PipelineDescriptorSetLayoutCreateInfo, DynamicState, GraphicsPipeline, Pipeline,
        PipelineBindPoint,
        PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
};
use winit::dpi::PhysicalSize;

pub struct PresentPass {
    render_pass: Arc<RenderPass>,
    frame_buffers: Vec<Arc<Framebuffer>>,
    pipeline: Arc<GraphicsPipeline>,
    viewport: Viewport,

    present_descriptor_set: Arc<DescriptorSet>,
}

impl PresentPass {
    pub fn new(
        world: &RefCell<World>,
        vulkano_window_renderer: &VulkanoWindowRenderer,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        image_to_present: Arc<Texture>,
    ) -> Self {
        let device = VULKAN.device().clone();
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    format: vulkano_window_renderer.swapchain_format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {},
            },
        )
        .unwrap();

        let vs = vs::load(VULKAN.device().clone())
            .unwrap()
            .entry_point("main")
            .unwrap();
        let fs = fs::load(VULKAN.device().clone())
            .unwrap()
            .entry_point("main")
            .unwrap();

        let pipeline = {
            let stages = [
                PipelineShaderStageCreateInfo::new(vs.clone()),
                PipelineShaderStageCreateInfo::new(fs.clone()),
            ];
            let layout = PipelineLayout::new(
                device.clone(),
                PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                    .into_pipeline_layout_create_info(device.clone())
                    .unwrap(),
            )
            .unwrap();
            let subpass = Subpass::from(render_pass.clone(), 0).unwrap();

            GraphicsPipeline::new(
                device.clone(),
                None,
                GraphicsPipelineCreateInfo {
                    vertex_input_state: Some(Default::default()),
                    stages: stages.into_iter().collect(),
                    input_assembly_state: Some(InputAssemblyState {
                        topology: PrimitiveTopology::TriangleStrip,
                        ..Default::default()
                    }),
                    viewport_state: Some(ViewportState::default()),
                    rasterization_state: Some(RasterizationState::default()),
                    multisample_state: Some(MultisampleState::default()),
                    color_blend_state: Some(ColorBlendState::with_attachment_states(
                        subpass.num_color_attachments(),
                        ColorBlendAttachmentState::default(),
                    )),
                    dynamic_state: [DynamicState::Viewport].into_iter().collect(),
                    subpass: Some(subpass.into()),
                    ..GraphicsPipelineCreateInfo::layout(layout)
                },
            )
            .unwrap()
        };

        let frame_buffers = recreate_framebuffers(
            vulkano_window_renderer.swapchain_image_views(),
            &render_pass,
        );

        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: vulkano_window_renderer.window_size().into(),
            depth_range: 0.0..=1.0,
        };

        let sampler = world.borrow().resource::<GlobalSamplers>().clamp.clone();

        let layout = &pipeline.layout().set_layouts()[0];
        let descriptor_set = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            layout.clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                image_to_present.image_view.clone(),
                sampler.clone(),
            )],
            [],
        )
        .unwrap();

        // We are now ready to draw!
        tracing::info!("Render - Render pass Present Pass successfully created.");

        Self {
            render_pass,
            frame_buffers,
            pipeline,
            viewport,
            present_descriptor_set: descriptor_set,
        }
    }
}

impl BufferPassTrait for PresentPass {
    fn render(
        &mut self,
        image_idx: u32,
        _world: &World,
        _descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    ) {
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some([0.9, 0.9, 0.9, 1.0].into())],
                    ..RenderPassBeginInfo::framebuffer(
                        self.frame_buffers[image_idx as usize].clone(),
                    )
                },
                Default::default(),
            )
            .unwrap()
            .set_viewport(0, [self.viewport.clone()].into_iter().collect())
            .unwrap()
            .bind_pipeline_graphics(self.pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                self.present_descriptor_set.clone(),
            )
            .unwrap();
        unsafe { builder.draw(4, 1, 0, 0) }.unwrap();

        builder.end_render_pass(Default::default()).unwrap();
    }

    fn on_swapchain_recreate(
        &mut self,
        swapchain_images: &[Arc<ImageView>],
        window_size: PhysicalSize<u32>,
    ) {
        self.frame_buffers = recreate_framebuffers(swapchain_images, &self.render_pass);

        self.viewport.extent = [window_size.width as f32, window_size.height as f32];
    }
}

fn recreate_framebuffers(
    images: &[Arc<ImageView>],
    render_pass: &Arc<RenderPass>,
) -> Vec<Arc<Framebuffer>> {
    let framebuffers = images
        .iter()
        .map(|view| {
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view.clone()],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();

    framebuffers
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../resources/shaders/fullscreen.vert"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "\
        #version 460
        #extension GL_EXT_nonuniform_qualifier : enable

        layout (set = 0, binding = 0) uniform sampler2D Image;

        layout (location = 0) in vec2 fragTexCoord;
        layout (location = 0) out vec4 outColor;

        void main() {
            outColor = texture(nonuniformEXT(Image), fragTexCoord);
        }
        ",
    }
}
