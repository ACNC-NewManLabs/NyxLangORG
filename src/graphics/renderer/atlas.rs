use std::collections::BTreeMap;
use wgpu::*;

#[derive(Debug, Clone, Default)]
pub struct Atlas<T> {
    pub entries: BTreeMap<String, T>,
}

impl<T> Atlas<T> {
    pub fn insert(&mut self, key: impl Into<String>, value: T) {
        self.entries.insert(key.into(), value);
    }
}

pub struct TextureAtlas {
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: Sampler,
    pub width: u32,
    pub height: u32,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl TextureAtlas {
    pub fn new(device: &Device, queue: &Queue, width: u32, height: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Texture Atlas"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor::default());

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Texture Atlas Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        // Initialize with white texture
        let white_data = vec![255u8; (width * height * 4) as usize];
        queue.write_texture(
            ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &white_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(Self {
            texture,
            view,
            sampler,
            width,
            height,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        })
    }

    pub fn add_texture(&mut self, _device: &Device, queue: &Queue, data: &[u8], width: u32, height: u32) -> Result<AtlasRegion, Box<dyn std::error::Error>> {
        // Check if texture fits in current row
        if self.cursor_x + width > self.width {
            // Move to next row
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }

        // Check if we need to move to next page (simplified - just return error for now)
        if self.cursor_y + height > self.height {
            return Err("Atlas full".into());
        }

        // Update row height if needed
        if height > self.row_height {
            self.row_height = height;
        }

        // Write texture data
        queue.write_texture(
            ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: self.cursor_x,
                    y: self.cursor_y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let region = AtlasRegion {
            x: self.cursor_x,
            y: self.cursor_y,
            width,
            height,
            u0: self.cursor_x as f32 / self.width as f32,
            v0: self.cursor_y as f32 / self.height as f32,
            u1: (self.cursor_x + width) as f32 / self.width as f32,
            v1: (self.cursor_y + height) as f32 / self.height as f32,
        };

        // Advance cursor
        self.cursor_x += width;

        Ok(region)
    }
}

#[derive(Debug, Clone)]
pub struct AtlasRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}
