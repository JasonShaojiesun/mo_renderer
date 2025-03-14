use crate::application::plugin::{PluginState, PluginTrait};
use anyhow::Result;
use bevy_ecs::prelude::*;
use mo_ecs::resource::{Camera, Input};
use mo_renderpass::{RenderPassTrait, GBufferPass, PresentPass, SSAOPass, ShadingPass, ShadowPass};
use mo_vk::{VulkanoWindows, WindowDescriptor, VULKAN};
use std::{cell::RefCell, sync::Arc, time::Duration};
use thiserror::Error;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage},
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    sync::GpuFuture,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

pub mod plugin;

pub struct App {
    pub world: RefCell<World>,
    startup_schedule: Schedule,
    runtime_schedule: Schedule,
    plugin_state: PluginState,
    plugins: Vec<Box<dyn PluginTrait>>,

    // vulkano related
    windows: VulkanoWindows,
    window_descriptor: WindowDescriptor,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,

    // Render pass
    render_passes: RefCell<Vec<Box<dyn RenderPassTrait>>>,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("duplicate plugin {plugin_name:?}")]
    DuplicatePlugin { plugin_name: String },
}

impl App {
    pub fn new(_event_loop: &EventLoop<()>, window_descriptor: WindowDescriptor) -> Self {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();

        // Manages any windows and their rendering.
        let windows = VulkanoWindows::default();

        // Some little debug infos.
        println!(
            "Using device: {} (type: {:?})",
            VULKAN.device().physical_device().properties().device_name,
            VULKAN.device().physical_device().properties().device_type,
        );

        tracing::info!("Context - Render Context and Window successfully created");

        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
            VULKAN.device().clone(),
            Default::default(),
        ));

        Self {
            world: RefCell::new(World::new()),
            startup_schedule: Schedule::default(),
            runtime_schedule: Schedule::default(),
            plugin_state: PluginState::Adding,
            plugins: Vec::new(),
            windows,
            window_descriptor,
            descriptor_set_allocator,
            render_passes: RefCell::new(Vec::new()),
        }
    }

    pub fn add_plugin(&mut self, plugin: Box<dyn PluginTrait>) -> Result<(), AppError> {
        tracing::info!("Plugin - Plugin: {} added to main app.", plugin.name());

        self.plugins.push(plugin);

        Ok(())
    }

    pub fn init_resource<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.world.borrow_mut().init_resource::<R>();
        self
    }

    pub fn insert_resource<R: Resource + FromWorld>(&mut self, resource: R) -> &mut Self {
        self.world.borrow_mut().insert_resource(resource);
        self
    }

    pub fn add_startup_system<T>(&mut self, system: impl IntoSystemConfigs<T>) {
        self.startup_schedule.add_systems(system);
    }

    pub fn add_runtime_system<T>(&mut self, system: impl IntoSystemConfigs<T>) {
        self.runtime_schedule.add_systems(system);
    }

    pub fn add_render_pass(&self, render_pass: Box<dyn RenderPassTrait>) {
        self.render_passes.borrow_mut().push(render_pass);
    }

    pub fn run_startup_systems(&mut self) {
        self.startup_schedule.run(&mut self.world.borrow_mut());
        tracing::info!("Runtime - Startup systems finished running.");
    }

    pub fn run_runtime_systems(&mut self) {
        self.runtime_schedule.run(&mut self.world.borrow_mut());
    }

    pub fn add_entity<B: Bundle>(&mut self, entity: B) -> Entity {
        self.world.borrow_mut().spawn(entity).id()
    }

    pub fn window_descriptor(&self) -> &WindowDescriptor {
        &self.window_descriptor
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(primary_window_id) = self.windows.primary_window_id() {
            self.windows.remove_renderer(primary_window_id);
        }

        self.windows
            .create_window(event_loop, &VULKAN, &Default::default(), |_| {});

        let shadow_pass = ShadowPass::new();

        let gbuffer_pass = GBufferPass::new(
            &self.world,
            self.windows.get_primary_renderer().unwrap(),
            self.descriptor_set_allocator.clone(),
        );

        let ssao_pass = SSAOPass::new(
            gbuffer_pass.gbuffer_textures.depth.clone(),
            self.windows.get_primary_renderer().unwrap(),
        );

        let shading_pass = ShadingPass::new(
            &self.world,
            &gbuffer_pass.gbuffer_textures,
            &shadow_pass.shadow_map,
            &ssao_pass.ssao_texture,
            self.windows.get_primary_renderer().unwrap(),
            self.descriptor_set_allocator.clone(),
        );

        let present_pass = PresentPass::new(
            &self.world,
            self.windows.get_primary_renderer().unwrap(),
            self.descriptor_set_allocator.clone(),
            shading_pass.output_image.clone(),
        );

        self.add_render_pass(Box::new(shadow_pass));
        self.add_render_pass(Box::new(gbuffer_pass));
        self.add_render_pass(Box::new(ssao_pass));
        self.add_render_pass(Box::new(shading_pass));
        self.add_render_pass(Box::new(present_pass));

        self.run_startup_systems();

        tracing::info!("Runtime - Starting render loop.")
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // We first handle the window event.
        self.world
            .borrow_mut()
            .resource_mut::<Input>()
            .on_window_event(&event);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                self.windows.get_primary_renderer_mut().unwrap().resize();
            }
            WindowEvent::RedrawRequested => {
                self.run_runtime_systems();
                let window_renderer = self.windows.get_primary_renderer_mut().unwrap();

                let window_size = window_renderer.window().inner_size();

                // Do not draw the frame when the screen size is zero. On Windows, this can
                // occur when minimizing the application.
                if window_size.width == 0 || window_size.height == 0 {
                    return;
                }

                // Begin rendering by acquiring the gpu future from the window renderer.
                let previous_frame_end = window_renderer
                    .acquire(Some(Duration::from_millis(1000)), |swapchain_images| {
                        // When window size changes, we need to resize the camera.
                        self.world
                            .borrow_mut()
                            .resource_mut::<Camera>()
                            .resize(window_size.into());

                        // Whenever the window resizes we need to recreate everything dependent on the window size.
                        // In this example that includes the swapchain, the framebuffers and the dynamic state viewport.
                        for render_pass in self.render_passes.borrow_mut().iter_mut() {
                            render_pass.on_swapchain_recreate(swapchain_images, window_size);
                        }
                    })
                    .unwrap();

                // In order to draw, we have to record a *command buffer*. The command buffer
                // object holds the list of commands that are going to be executed.
                //
                // Recording a command buffer is an expensive operation (usually a few hundred
                // microseconds), but it is known to be a hot path in the driver and is expected to
                // be optimized.
                //
                // Note that we have to pass a queue family when we create the command buffer. The
                // command buffer will only be executable on that given queue family.
                let mut builder = AutoCommandBufferBuilder::primary(
                    VULKAN.command_buffer_allocator().clone(),
                    VULKAN.graphics_queue().queue_family_index(),
                    CommandBufferUsage::OneTimeSubmit,
                )
                .unwrap();

                for render_pass in self.render_passes.borrow_mut().iter_mut() {
                    render_pass.render(
                        window_renderer.image_index(),
                        &self.world.borrow(),
                        self.descriptor_set_allocator.clone(),
                        &mut builder,
                    );
                }

                // Finish recording the command buffer by calling `end`.
                let command_buffer = builder.build().unwrap();

                let future = previous_frame_end
                    .then_execute(VULKAN.graphics_queue().clone(), command_buffer)
                    .unwrap()
                    .boxed();

                // The color output is now expected to contain our triangle. But in order to show
                // it on the screen, we have to *present* the image by calling `present` on the
                // window renderer.
                //
                // This function does not actually present the image immediately. Instead, it
                // submits a present command at the end of the queue. This means that it will only
                // be presented once the GPU has finished executing the command buffer that draws
                // the triangle.
                window_renderer.present(future, false);
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.world
            .borrow_mut()
            .resource_mut::<Input>()
            .on_device_event(&event);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let window_renderer = self.windows.get_primary_renderer_mut().unwrap();
        window_renderer.window().request_redraw();
    }
}
