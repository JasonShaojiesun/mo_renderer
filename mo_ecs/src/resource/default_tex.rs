use bevy_ecs::prelude::Resource;
use mo_vk::{Texture, TextureCreateInfo};
use std::sync::Arc;

#[derive(Resource)]
pub struct DefaultTextures {
    pub diffuse: Arc<Texture>,
    pub normal: Arc<Texture>,
    pub metallic_roughness: Arc<Texture>,
    pub occlusion: Arc<Texture>,
    pub grid: Arc<Texture>,
    pub black: Arc<Texture>,
}

impl DefaultTextures {
    pub fn new() -> Self {
        let info = TextureCreateInfo::default();

        let diffuse = Arc::new(Texture::load_from_file("white_texture.png", &info));
        let normal = Arc::new(Texture::load_from_file("flat_normal_map.png", &info));
        let metallic_roughness = Arc::new(Texture::load_from_file(
            "default_metallic_roughness.png",
            &info,
        ));
        let occlusion = Arc::new(Texture::load_from_file("white_texture.png", &info));
        let grid = Arc::new(Texture::load_from_file("checker.jpg", &info));
        let black = Arc::new(Texture::load_from_file("default_black.png", &info));

        tracing::info!("ECS - Default Textures resources successfully loaded.");

        Self {
            diffuse,
            normal,
            metallic_roughness,
            occlusion,
            grid,
            black,
        }
    }
}

impl Default for DefaultTextures {
    fn default() -> Self {
        Self::new()
    }
}
