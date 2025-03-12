use crate::VULKAN;
use std::sync::Arc;
use vulkano::buffer::BufferContents;
use vulkano::command_buffer::{BlitImageInfo, ImageBlit};
use vulkano::image::sampler::Filter;
use vulkano::image::{ImageLayout, ImageSubresourceLayers};
use vulkano::sync::GpuFuture;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
        PrimaryCommandBufferAbstract,
    },
    format::Format,
    image::{
        Image, ImageAspects, ImageCreateFlags, ImageCreateInfo, ImageSubresourceRange, ImageType,
        ImageUsage,
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    sync,
};

#[derive(Clone)]
pub struct TextureCreateInfo {
    pub image_type: ImageType,
    pub format: Format,
    pub extent: [u32; 3],
    pub usage: ImageUsage,
    pub mip_levels: u32,
    pub hdr: bool,
}

impl TextureCreateInfo {
    pub fn default_hdr() -> Self {
        Self {
            image_type: ImageType::Dim2d,
            format: Format::R32G32B32A32_SFLOAT,
            extent: [1, 1, 1],
            usage: ImageUsage::SAMPLED
                | ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::STORAGE
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            mip_levels: 1,
            hdr: true,
        }
    }
}

impl Default for TextureCreateInfo {
    fn default() -> Self {
        Self {
            image_type: ImageType::Dim2d,
            format: Format::R8G8B8A8_UNORM,
            extent: [1, 1, 1],
            usage: ImageUsage::SAMPLED
                | ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::STORAGE
                | ImageUsage::TRANSFER_DST
                | ImageUsage::TRANSFER_SRC,
            mip_levels: 1,
            hdr: false,
        }
    }
}

pub struct Texture {
    pub image_view: Arc<ImageView>,
    pub info: TextureCreateInfo,
}

impl Texture {
    pub fn load_from_file(path: &str, create_info: &TextureCreateInfo) -> Texture {
        let root = std::env::current_dir().expect("Current working directory must be accessible");
        let path_buf = root.join("resources").join("tex").join(path);

        let image = match image::open(path_buf) {
            Ok(image) => image,
            Err(_err) => panic!("Unable to load \"{}\"", path),
        };

        if !create_info.hdr {
            let image = image.to_rgba8();
            let (width, height) = (image.width(), image.height());
            let image_data = image.into_raw();

            let mut updated_create_info = create_info.clone();
            updated_create_info.extent = [width, height, 1];

            Self::create::<u8>(image_data, updated_create_info)
        } else {
            let image = image.to_rgba32f();
            let (width, height) = (image.width(), image.height());
            let image_data = image.into_raw();

            let mut updated_create_info = create_info.clone();
            updated_create_info.extent = [width, height, 1];

            Self::create::<f32>(image_data, updated_create_info)
        }
    }

    pub fn new(create_info: TextureCreateInfo) -> Texture {
        let memory_allocator = VULKAN.memory_allocator().clone();

        let texture = {
            let extent = [create_info.extent[0], create_info.extent[1], 1];

            let image = Image::new(
                memory_allocator,
                ImageCreateInfo {
                    image_type: create_info.image_type,
                    format: create_info.format,
                    extent,
                    usage: create_info.usage,
                    mip_levels: create_info.mip_levels,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap();

            ImageView::new_default(image).unwrap()
        };

        Self {
            image_view: texture,
            info: create_info,
        }
    }

    pub fn new_cubemap(create_info: TextureCreateInfo) -> Texture {
        let memory_allocator = VULKAN.memory_allocator().clone();

        let texture = {
            let extent = [create_info.extent[0], create_info.extent[1], 1];

            let image = Image::new(
                memory_allocator,
                ImageCreateInfo {
                    flags: ImageCreateFlags::CUBE_COMPATIBLE,
                    image_type: ImageType::Dim2d,
                    format: Format::R32G32B32A32_SFLOAT,
                    extent,
                    usage: create_info.usage,
                    array_layers: 6,
                    mip_levels: create_info.mip_levels,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap();

            let image_view_ci = ImageViewCreateInfo {
                view_type: ImageViewType::Cube,
                format: Format::R32G32B32A32_SFLOAT,
                usage: ImageUsage::SAMPLED
                    | ImageUsage::COLOR_ATTACHMENT
                    | ImageUsage::STORAGE
                    | ImageUsage::TRANSFER_DST
                    | ImageUsage::TRANSFER_SRC,
                subresource_range: ImageSubresourceRange {
                    aspects: ImageAspects::COLOR,
                    mip_levels: 0..create_info.mip_levels,
                    array_layers: 0..6,
                },
                ..Default::default()
            };

            ImageView::new(image, image_view_ci).unwrap()
        };

        Self {
            image_view: texture,
            info: create_info,
        }
    }

    pub fn create<T: BufferContents>(
        image_data: Vec<T>,
        create_info: TextureCreateInfo,
    ) -> Texture {
        let memory_allocator = VULKAN.memory_allocator().clone();

        let mut uploads = AutoCommandBufferBuilder::primary(
            VULKAN.command_buffer_allocator().clone(),
            VULKAN.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let texture = {
            let extent = [create_info.extent[0], create_info.extent[1], 1];

            let upload_buffer = Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                image_data,
            )
            .unwrap();

            let image = Image::new(
                memory_allocator,
                ImageCreateInfo {
                    image_type: create_info.image_type,
                    format: create_info.format,
                    extent,
                    usage: create_info.usage,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap();

            uploads
                .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                    upload_buffer,
                    image.clone(),
                ))
                .unwrap();

            ImageView::new_default(image).unwrap()
        };

        let _ = uploads
            .build()
            .unwrap()
            .execute(VULKAN.graphics_queue().clone())
            .unwrap();

        Self {
            image_view: texture,
            info: create_info,
        }
    }

    /// Generate mipmaps for the texture. Note that this function should be called only when
    /// the image is created with mipmap level info, and the first mipmap level is filled.
    pub fn generate_mipmaps(&self) {
        let mip_levels = self.image_view.image().mip_levels();
        let mut mip_width = self.info.extent[0];
        let mut mip_height = self.info.extent[1];

        let mut builder = AutoCommandBufferBuilder::primary(
            VULKAN.command_buffer_allocator().clone(),
            VULKAN.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // 循环生成每个mip层级
        for i in 1..mip_levels {
            let src_level = i - 1;
            let dst_level = i;

            // 计算下一级mip尺寸
            let (next_width, next_height) = ((mip_width >> 1).max(1), (mip_height >> 1).max(1));

            // 执行blit操作
            builder
                .blit_image(BlitImageInfo {
                    // Same as above applies for blitting.
                    src_image_layout: ImageLayout::General,
                    dst_image_layout: ImageLayout::General,
                    regions: [ImageBlit {
                        src_subresource: ImageSubresourceLayers {
                            aspects: ImageAspects::COLOR,
                            mip_level: src_level,
                            array_layers: 0..1,
                        },
                        src_offsets: [[0, 0, 0], [mip_width, mip_width, 1]],
                        dst_subresource: ImageSubresourceLayers {
                            aspects: ImageAspects::COLOR,
                            mip_level: dst_level,
                            array_layers: 0..1,
                        },
                        // Swapping the two corners results in flipped image.
                        dst_offsets: [[0, 0, 0], [next_width, next_height, 1]],
                        ..Default::default()
                    }]
                    .into(),
                    filter: Filter::Linear,
                    ..BlitImageInfo::images(
                        self.image_view.image().clone(),
                        self.image_view.image().clone(),
                    )
                })
                .unwrap();

            mip_width = next_width;
            mip_height = next_height;
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

    pub fn copy_to_mip_level(&self, target_mip_level: u32) {
        assert!(
            target_mip_level > 1 && target_mip_level <= self.image_view.image().mip_levels(),
            "The passed in mip level is invalid"
        );

        let mut mip_width = self.info.extent[0];
        let mut mip_height = self.info.extent[1];

        let mut builder = AutoCommandBufferBuilder::primary(
            VULKAN.command_buffer_allocator().clone(),
            VULKAN.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        // 循环生成每个mip层级
        for _ in 1..target_mip_level {
            // 计算下一级mip尺寸
            let (next_width, next_height) = ((mip_width >> 1).max(1), (mip_height >> 1).max(1));

            mip_width = next_width;
            mip_height = next_height;
        }

        // 执行blit操作
        builder
            .blit_image(BlitImageInfo {
                // Same as above applies for blitting.
                src_image_layout: ImageLayout::General,
                dst_image_layout: ImageLayout::General,
                regions: [ImageBlit {
                    src_subresource: ImageSubresourceLayers {
                        aspects: ImageAspects::COLOR,
                        mip_level: 0,
                        array_layers: 0..1,
                    },
                    src_offsets: [[0, 0, 0], [self.info.extent[0], self.info.extent[1], 1]],
                    dst_subresource: ImageSubresourceLayers {
                        aspects: ImageAspects::COLOR,
                        mip_level: target_mip_level - 1,
                        array_layers: 0..1,
                    },
                    // Swapping the two corners results in flipped image.
                    dst_offsets: [[0, 0, 0], [mip_width, mip_height, 1]],
                    ..Default::default()
                }]
                .into(),
                filter: Filter::Linear,
                ..BlitImageInfo::images(
                    self.image_view.image().clone(),
                    self.image_view.image().clone(),
                )
            })
            .unwrap();

        // Finish recording the command buffer by calling `end`.
        let command_buffer = builder.build().unwrap();

        let future = sync::now(VULKAN.device().clone())
            .then_execute(VULKAN.graphics_queue().clone(), command_buffer)
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap();

        future.wait(None).unwrap();
    }
}
