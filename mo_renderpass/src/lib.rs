pub mod present_pass;
pub use present_pass::PresentPass;

pub mod gbuffer_pass;
pub use gbuffer_pass::{GBufferPass, GBufferTextures};

pub mod shading_pass;
pub use shading_pass::{ShadingPass};

pub mod shadow_pass;
pub use shadow_pass::ShadowPass;

pub mod ssao_pass;
pub use ssao_pass::SSAOPass;

pub mod utils;

use bevy_ecs::prelude::*;
use std::sync::Arc;
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::image::view::ImageView;
use winit::dpi::PhysicalSize;

pub trait RenderPassTrait {
    /// Renders the current frame using the provided resources and world state.
    ///
    /// # Arguments
    ///
    /// * `image_idx` - The index of the swapchain image to render to.
    /// * `world` - The ECS world containing the current game state.
    /// * `descriptor_set_allocator` - Allocator for descriptor sets.
    /// * `command_buffer_allocator` - Builder for the command buffer.
    fn render(
        &mut self,
        image_idx: u32,
        world: &World,
        descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    );

    /// Framebuffers and pipeline need to be recreated when swapchain changes, and viewport information also needs to be set again.
    fn on_swapchain_recreate(
        &mut self,
        swapchain_images: &[Arc<ImageView>],
        window_size: PhysicalSize<u32>,
    );
}
