// Spec: specs/client/render-pipeline.md (A9)
//! The wgpu side: pipelines, upload, dispatch, readback. Nothing here
//! decides a pixel; it moves the [`Packed`] bytes and the atlas pages to
//! the GPU unchanged and reads the framebuffer back.

use std::sync::mpsc;

use d2_formats::palette::Palette;
use wgpu::util::DeviceExt;

use super::pack::Packed;
use super::{GpuError, SHADER, WORKGROUP};
use crate::frames::atlas::AtlasPage;
use crate::frames::PAGE_SIZE;

/// A device and queue with the compositor's two pipelines.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    compose: wgpu::ComputePipeline,
    to_rgba: wgpu::ComputePipeline,
}

impl Gpu {
    /// A headless device on the default adapter (no window, no surface):
    /// what verify uses. Also returns the adapter's description.
    pub fn headless() -> Result<(Self, wgpu::AdapterInfo), GpuError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = bevy::tasks::block_on(
            instance.request_adapter(&wgpu::RequestAdapterOptions::default()),
        )
        .map_err(|e| GpuError::Adapter(e.to_string()))?;
        let info = adapter.get_info();
        let (device, queue) =
            bevy::tasks::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("d2 compositor"),
                ..Default::default()
            }))
            .map_err(|e| GpuError::Device(e.to_string()))?;
        Ok((Self::from_device(device, queue), info))
    }

    /// On an existing device, e.g. Bevy's (`RenderDevice::wgpu_device()`
    /// and the `RenderQueue`), for the in-app render node.
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("d2 compositor"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let compose = pipeline("compose");
        let to_rgba = pipeline("to_rgba");
        Gpu {
            device,
            queue,
            compose,
            to_rgba,
        }
    }

    /// Composes `packed` over the atlas `pages`: the index framebuffer,
    /// `view.width × view.height` bytes, equal to `scene::compose`.
    pub fn compose(&self, packed: &Packed, pages: &[AtlasPage]) -> Result<Vec<u8>, GpuError> {
        Ok(self.run(packed, pages, None)?.0)
    }

    /// [`Gpu::compose`] plus the RGBA8 image through `palette` (equal to
    /// `scene::compose_rgba`). Returns `(indices, rgba)`.
    pub fn compose_rgba(
        &self,
        packed: &Packed,
        pages: &[AtlasPage],
        palette: &Palette,
    ) -> Result<(Vec<u8>, Vec<u8>), GpuError> {
        let (indices, rgba) = self.run(packed, pages, Some(palette))?;
        Ok((indices, rgba.unwrap_or_default()))
    }

    /// The atlas pages as the compositor's texture array (layer = page,
    /// bytes unchanged), for a caller that keeps it across frames (the
    /// in-app render node, `app::compose_node`).
    pub fn atlas_texture(&self, pages: &[AtlasPage]) -> wgpu::Texture {
        self.upload_pages(pages)
    }

    /// Records both dispatches of `packed` into `encoder` over a resident
    /// atlas (from [`Gpu::atlas_texture`] of `pages` pages) and returns
    /// the RGBA8 buffer: one RGBA8 word per pixel, rows of `width × 4`
    /// bytes, usable as a copy source. Nothing is submitted or read back;
    /// `None` for an empty view.
    pub fn encode_rgba(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        packed: &Packed,
        atlas: &wgpu::Texture,
        pages: u32,
        palette: &Palette,
    ) -> Result<Option<wgpu::Buffer>, GpuError> {
        if pages != packed.params.pages {
            return Err(GpuError::PageCount {
                pages: pages as usize,
                packed: packed.params.pages,
            });
        }
        if packed.pixel_count() == 0 {
            return Ok(None);
        }
        self.check_limits(packed, u64::from(pages))?;
        Ok(self.encode(encoder, packed, atlas, Some(palette)).1)
    }

    fn run(
        &self,
        packed: &Packed,
        pages: &[AtlasPage],
        palette: Option<&Palette>,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), GpuError> {
        if pages.len() != packed.params.pages as usize {
            return Err(GpuError::PageCount {
                pages: pages.len(),
                packed: packed.params.pages,
            });
        }
        let pixels = packed.pixel_count() as u64;
        if pixels == 0 {
            return Ok((Vec::new(), palette.map(|_| Vec::new())));
        }
        self.check_limits(packed, pages.len() as u64)?;
        let out_size = pixels * 4;
        let atlas = self.upload_pages(pages);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let (indices, rgba) = self.encode(&mut encoder, packed, &atlas, palette);
        let read_index = self.staging(&mut encoder, &indices, out_size);
        let read_rgba = rgba
            .as_ref()
            .map(|b| self.staging(&mut encoder, b, out_size));
        self.queue.submit([encoder.finish()]);

        // One u32 per pixel holding 0..=255: keep the low byte.
        let index_bytes = self.read(&read_index)?;
        let out: Vec<u8> = index_bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| w[0])
            .collect();
        let rgba = match read_rgba {
            Some(b) => Some(self.read(&b)?),
            None => None,
        };
        Ok((out, rgba))
    }

    /// Records the compose dispatch (and the RGBA one with a palette);
    /// returns the index and RGBA output buffers. Limits are checked by
    /// the caller.
    fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        packed: &Packed,
        atlas: &wgpu::Texture,
        palette: Option<&Palette>,
    ) -> (wgpu::Buffer, Option<wgpu::Buffer>) {
        let d = &self.device;
        let init = |label, contents: &[u8], usage| {
            d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE;
        let params = init(
            "params",
            &packed.params_bytes(),
            wgpu::BufferUsages::UNIFORM,
        );
        let items = init("items", &packed.items_bytes(), storage);
        let ranges = init("bin ranges", &packed.bin_ranges_bytes(), storage);
        let bin_items = init("bin items", &packed.bin_items_bytes(), storage);
        let maps = init("maps", &packed.maps_bytes(), storage);
        let base = init("base", &packed.base_bytes(), storage);
        let out_size = packed.pixel_count() as u64 * 4;
        let output = |label| {
            d.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: out_size,
                usage: storage | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        };
        let indices = output("indices");
        let atlas_view = atlas.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        fn entry(binding: u32, resource: wgpu::BindingResource<'_>) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource }
        }
        let compose_group = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compose"),
            layout: &self.compose.get_bind_group_layout(0),
            entries: &[
                entry(0, params.as_entire_binding()),
                entry(1, items.as_entire_binding()),
                entry(2, ranges.as_entire_binding()),
                entry(3, bin_items.as_entire_binding()),
                entry(4, maps.as_entire_binding()),
                entry(5, wgpu::BindingResource::TextureView(&atlas_view)),
                entry(6, indices.as_entire_binding()),
                entry(9, base.as_entire_binding()),
            ],
        });
        let groups = (
            packed.params.width.div_ceil(WORKGROUP),
            packed.params.height.div_ceil(WORKGROUP),
        );
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.compose);
            pass.set_bind_group(0, &compose_group, &[]);
            pass.dispatch_workgroups(groups.0, groups.1, 1);
        }
        let rgba = palette.map(|palette| {
            let words: Vec<u8> = palette
                .colors
                .iter()
                .flat_map(|c| [c.r, c.g, c.b, 255])
                .collect();
            let palette = init("palette", &words, storage);
            let rgba = output("rgba");
            let group = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("to_rgba"),
                layout: &self.to_rgba.get_bind_group_layout(0),
                entries: &[
                    entry(0, params.as_entire_binding()),
                    entry(6, indices.as_entire_binding()),
                    entry(7, palette.as_entire_binding()),
                    entry(8, rgba.as_entire_binding()),
                ],
            });
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.to_rgba);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(groups.0, groups.1, 1);
            drop(pass);
            rgba
        });
        (indices, rgba)
    }

    fn check_limits(&self, packed: &Packed, pages: u64) -> Result<(), GpuError> {
        let limits = self.device.limits();
        let storage = limits.max_storage_buffer_binding_size;
        let sizes = [
            ("item buffer", packed.items_bytes().len() as u64),
            ("bin item buffer", packed.bin_items.len() as u64 * 4),
            ("map table buffer", packed.maps.len() as u64),
            ("framebuffer", packed.pixel_count() as u64 * 4),
            ("base buffer", packed.base_bytes().len() as u64),
        ];
        for (what, needed) in sizes {
            if needed > storage {
                return Err(GpuError::Limit {
                    what,
                    needed,
                    limit: storage,
                });
            }
        }
        let layers = u64::from(limits.max_texture_array_layers);
        if pages > layers {
            return Err(GpuError::Limit {
                what: "atlas pages",
                needed: pages,
                limit: layers,
            });
        }
        let groups = u64::from(limits.max_compute_workgroups_per_dimension);
        let needed = u64::from(
            packed
                .params
                .width
                .max(packed.params.height)
                .div_ceil(WORKGROUP),
        );
        if needed > groups {
            return Err(GpuError::Limit {
                what: "workgroups per dimension",
                needed,
                limit: groups,
            });
        }
        Ok(())
    }

    /// The pages as one R8Uint texture array (layer = page), bytes
    /// unchanged. No pages: one 1×1 zero layer that nothing reads.
    fn upload_pages(&self, pages: &[AtlasPage]) -> wgpu::Texture {
        let side = if pages.is_empty() { 1 } else { PAGE_SIZE };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: pages.len().max(1) as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, page) in pages.iter().enumerate() {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &page.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(PAGE_SIZE),
                    rows_per_image: Some(PAGE_SIZE),
                },
                wgpu::Extent3d {
                    width: PAGE_SIZE,
                    height: PAGE_SIZE,
                    depth_or_array_layers: 1,
                },
            );
        }
        texture
    }

    fn staging(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Buffer,
        size: u64,
    ) -> wgpu::Buffer {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(source, 0, &buffer, 0, size);
        buffer
    }

    fn read(&self, buffer: &wgpu::Buffer) -> Result<Vec<u8>, GpuError> {
        let (tx, rx) = mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |r| {
            // The receiver waits below; a send error cannot happen.
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| GpuError::Readback(e.to_string()))?
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        let bytes = buffer.get_mapped_range(..).to_vec();
        buffer.unmap();
        Ok(bytes)
    }
}
