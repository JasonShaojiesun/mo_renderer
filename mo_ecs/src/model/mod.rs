use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Vec2, Vec3, Vec4};
use mo_vk::{Texture, TextureCreateInfo};
use vulkano::format::Format;

pub mod material;
pub mod primitives;

pub use material::*;
pub use primitives::*;

pub const DEFAULT_TEXTURE_MAP: u32 = u32::MAX;

#[derive(Component)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub textures: Vec<Texture>,
}

impl Model {
    pub fn load_node(
        gltf: &gltf::Document,
        node: &gltf::Node,
        model: &mut Model,
        buffers: &[gltf::buffer::Data],
        parent_transform: Mat4,
        path: std::path::PathBuf,
    ) {
        let node_transform =
            parent_transform * Mat4::from_cols_array_2d(&node.transform().matrix());

        for child in node.children() {
            Model::load_node(gltf, &child, model, buffers, node_transform, path.clone());
        }

        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|i| Some(&buffers[i.index()]));

                let indices: Vec<_> = reader.read_indices().unwrap().into_u32().collect();
                let positions: Vec<_> = reader.read_positions().unwrap().map(Vec3::from).collect();
                let normals: Vec<_> = reader.read_normals().unwrap().map(Vec3::from).collect();
                let tex_coords0 = if let Some(tex_coords) = reader.read_tex_coords(0) {
                    tex_coords.into_f32().map(Vec2::from).collect()
                } else {
                    vec![Vec2::new(0.0, 0.0); positions.len()]
                };

                let tex_coords1 = if let Some(tex_coords1) = reader.read_tex_coords(1) {
                    tex_coords1.into_f32().map(Vec2::from).collect()
                } else {
                    vec![Vec2::new(0.0, 0.0); positions.len()]
                };

                let tangents = if let Some(tangents) = reader.read_tangents() {
                    tangents.map(Vec4::from).collect()
                } else {
                    vec![Vec4::new(0.0, 0.0, 0.0, 0.0); positions.len()]
                };

                let colors: Vec<_> = if let Some(colors) = reader.read_colors(0) {
                    colors.into_rgba_f32().map(Vec4::from).collect()
                } else {
                    vec![Vec4::new(1.0, 1.0, 1.0, 1.0); positions.len()]
                };

                let mut vertices: Vec<StaticVertex> = vec![];

                for (i, _) in positions.iter().enumerate() {
                    vertices.push(StaticVertex {
                        position: positions[i].extend(0.0).into(),
                        normal: normals[i].extend(0.0).into(),
                        uv0: tex_coords0[i].into(),
                        uv1: tex_coords1[i].into(),
                        tangent: tangents[i].into(),
                        color: colors[i].into(),
                    });
                }

                let material = primitive.material();
                let pbr = material.pbr_metallic_roughness();

                let get_texture_index = |texture_info: Option<gltf::texture::Info>| {
                    texture_info
                        .and_then(|tex| {
                            // 先获取texture索引
                            let texture_idx = tex.texture().index();
                            // 再通过texture获取对应的image索引
                            gltf.textures()
                                .nth(texture_idx)
                                .and_then(|t| Some(t.source().index()))
                        })
                        .map(|image_idx| image_idx as u32)
                        .unwrap_or(DEFAULT_TEXTURE_MAP)
                };

                let diffuse_index = get_texture_index(pbr.base_color_texture());
                let metallic_roughness_index = get_texture_index(pbr.metallic_roughness_texture());
                let emissive_index = get_texture_index(material.emissive_texture());

                let normal_index = material
                    .normal_texture()
                    .and_then(|tex| {
                        // 先获取texture索引
                        let texture_idx = tex.texture().index();
                        // 再通过texture获取对应的image索引
                        gltf.textures()
                            .nth(texture_idx)
                            .and_then(|t| Some(t.source().index()))
                    })
                    .map(|image_idx| image_idx as u32)
                    .unwrap_or(DEFAULT_TEXTURE_MAP);
                let occlusion_index = material
                    .occlusion_texture()
                    .and_then(|tex| {
                        // 先获取texture索引
                        let texture_idx = tex.texture().index();
                        // 再通过texture获取对应的image索引
                        gltf.textures()
                            .nth(texture_idx)
                            .and_then(|t| Some(t.source().index()))
                    })
                    .map(|image_idx| image_idx as u32)
                    .unwrap_or(DEFAULT_TEXTURE_MAP);

                let base_color_factor = material.pbr_metallic_roughness().base_color_factor();
                let metallic_factor = material.pbr_metallic_roughness().metallic_factor();
                let roughness_factor = material.pbr_metallic_roughness().roughness_factor();
                let emissive_factor = material.emissive_factor();

                let mut alpha_cutoff = 0.0f32;
                // 在primitive处理部分添加：
                let alpha_mode = match material.alpha_mode() {
                    gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
                    gltf::material::AlphaMode::Mask => {
                        alpha_cutoff = material.alpha_cutoff().unwrap_or(0.5);
                        AlphaMode::Mask
                    }
                    gltf::material::AlphaMode::Blend => AlphaMode::Blend,
                };

                // 在load_node的primitive处理部分补充：
                let base_color_tex_info = pbr.base_color_texture();
                let base_color_uv_set = base_color_tex_info.map(|t| t.tex_coord()).unwrap_or(0);

                let normal_tex_info = material.normal_texture();
                let normal_uv_set = normal_tex_info.map(|t| t.tex_coord()).unwrap_or(0);

                let metallic_roughness_tex_info = pbr.metallic_roughness_texture();
                let metallic_roughness_uv_set = metallic_roughness_tex_info
                    .map(|t| t.tex_coord())
                    .unwrap_or(0);

                let occlusion_tex_info = material.occlusion_texture();
                let occlusion_uv_set = occlusion_tex_info.map(|t| t.tex_coord()).unwrap_or(0);

                let emissive_tex_info = material.emissive_texture();
                let emissive_uv_set = emissive_tex_info.map(|t| t.tex_coord()).unwrap_or(0);

                model.meshes.push(Mesh {
                    primitive: MeshPrimitive::new(indices, vertices),
                    material: Material {
                        // Texture IDs
                        base_color_map: diffuse_index,
                        normal_map: normal_index,
                        metallic_roughness_map: metallic_roughness_index,
                        occlusion_map: occlusion_index,
                        emissive_map: emissive_index,
                        // UV Sets
                        base_color_uv_set,
                        normal_uv_set,
                        metallic_roughness_uv_set,
                        occlusion_uv_set,
                        emissive_uv_set,
                        // Alpha Mode
                        alpha_mode,
                        alpha_cutoff,
                        // Color Factors
                        base_color_factor: Vec4::from(base_color_factor),
                        metallic_factor,
                        roughness_factor,
                        emissive_factor: emissive_factor.into(),
                        // Raytracing properties
                        material_type: 0,
                        material_property: 0.0,
                    },
                    gpu_mat_index: 0,
                    world: node_transform,
                });
            }
        }
    }
    pub fn load_gltf(path: &str) -> Model {
        let root = std::env::current_dir().expect("Current working directory must be accessible");
        let path_buf = root.join("resources").join("gltf").join(path);

        let (gltf, buffers, mut images) = match gltf::import(path_buf.clone()) {
            Ok(result) => result,
            Err(err) => panic!("Loading model {} failed with error: {}", path, err),
        };

        let mut model = Model {
            meshes: vec![],
            textures: vec![],
        };

        for image in &mut images {
            // Convert images from rgb8 to rgba8
            if image.format == gltf::image::Format::R8G8B8 {
                let dynamic_image = image::DynamicImage::ImageRgb8(
                    image::RgbImage::from_raw(
                        image.width,
                        image.height,
                        std::mem::take(&mut image.pixels),
                    )
                    .unwrap(),
                );

                let rgba8_image = dynamic_image.to_rgba8();
                image.format = gltf::image::Format::R8G8B8A8;
                image.pixels = rgba8_image.into_raw();
            }

            let image_format: Format;

            match image.format {
                gltf::image::Format::R8 => image_format = Format::R8_UNORM,
                gltf::image::Format::R8G8 => image_format = Format::R8G8_UNORM,
                gltf::image::Format::R8G8B8 => image_format = Format::R8G8B8_UNORM,
                gltf::image::Format::R8G8B8A8 => image_format = Format::R8G8B8A8_UNORM,
                gltf::image::Format::R16 => image_format = Format::R16_UNORM,
                gltf::image::Format::R16G16 => image_format = Format::R16G16_UNORM,
                gltf::image::Format::R16G16B16 => image_format = Format::R16G16B16_UNORM,
                gltf::image::Format::R16G16B16A16 => image_format = Format::R16G16B16A16_UNORM,
                gltf::image::Format::R32G32B32FLOAT => image_format = Format::R32G32B32_SFLOAT,
                gltf::image::Format::R32G32B32A32FLOAT => {
                    image_format = Format::R32G32B32A32_SFLOAT
                }
            }

            let create_info = TextureCreateInfo {
                format: image_format,
                extent: [image.width, image.height, 1],
                ..Default::default()
            };

            let texture = Texture::create(image.pixels.clone(), create_info);

            model.textures.push(texture);
        }

        for scene in gltf.scenes() {
            for node in scene.nodes() {
                Model::load_node(
                    &gltf,
                    &node,
                    &mut model,
                    &buffers,
                    Mat4::IDENTITY,
                    path_buf.clone(),
                );
            }
        }

        model
    }
}
