use std::borrow::BorrowMut;
use std::{cell::RefCell, collections::BTreeMap, ops::Range, sync::Arc};
use thiserror::Error;
use void_core::db::IDb;
use void_core::IBuilder;
use wgpu::util::RenderEncoder;

use crate::{
    IBindGroup, IBuffer, IContext, IEncoder, IRenderContext, IRenderEncoder, TextureDesc,
    TextureError,
};

use crate::{
    CommandListIndex, Displayable, GpuResource, IGpu, IGpuCommandBuffer, IPipeline, Texture,
};

impl IBuffer for wgpu::CommandBuffer {}
impl IPipeline for wgpu::RenderPipeline {}
impl IPipeline for wgpu::ComputePipeline {}
impl IBuffer for wgpu::Buffer {}
impl IBindGroup for wgpu::BindGroup {}
impl IBuffer for wgpu::RenderBundle {}

impl<'a> IEncoder for wgpu::RenderBundleEncoder<'a> {
    type Buffer = wgpu::Buffer;
    type Pipeline = wgpu::RenderPipeline;
    type BindGroup = wgpu::BindGroup;
}

impl<'a> IRenderEncoder<'a> for wgpu::RenderBundleEncoder<'a> {
    fn set_pipeline(&mut self, pipeline: &'a Self::Pipeline) {
        self.set_pipeline(&pipeline);
    }
    fn set_bind_group(&mut self, index: u32, group: &'a Self::BindGroup) {
        self.set_bind_group(index, group, &[]);
    }
    fn draw(&mut self, verts: Range<u32>, instances: Range<u32>) {
        self.draw(verts, instances);
    }
    fn set_index_buffer(&mut self, buffer: &'a Self::Buffer) {
        self.set_index_buffer(buffer.slice(..), wgpu::IndexFormat::Uint16);
    }
    fn set_vertex_buffer(&mut self, slot: u32, buffer: &'a Self::Buffer) {
        self.set_vertex_buffer(slot, buffer.slice(..))
    }
}

pub struct RenderContext<'a, T: Displayable<'a>> {
    gpu_resource: Arc<GpuResource<'a, T>>,
    render_bundles: RefCell<Vec<wgpu::RenderBundle>>,
    cmd_encoder: wgpu::CommandEncoder,
}

impl<'a, T: Displayable<'a>> IContext<'a, T> for RenderContext<'a, T> {
    type CmdBuffer = wgpu::CommandBuffer;
    type Encoder = wgpu::RenderBundleEncoder<'a>;

    fn new(gpu_resource: Arc<GpuResource<'a, T>>) -> Self {
        let cmd_encoder =
            gpu_resource
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Command Encoder"),
                });
        Self {
            gpu_resource,
            render_bundles: RefCell::new(Vec::new()),
            cmd_encoder,
        }
    }

    fn get_encoder<'b>(&'a self) -> Self::Encoder
    where
        'a: 'b,
    {
        self.gpu_resource.device.create_render_bundle_encoder(
            &wgpu::RenderBundleEncoderDescriptor {
                label: Some("Render Bundle Encoder"),
                color_formats: &[],
                depth_stencil: None,
                sample_count: 1,
                multiview: None,
            },
        )
    }

    fn submit_encoders(&self, encoders: impl Iterator<Item = Self::Encoder>) {
        let bundles = encoders.map(|encoder| {
            encoder.finish(&wgpu::RenderBundleDescriptor {
                label: Some("Bundle"),
            })
        });
        self.render_bundles.borrow_mut().extend(bundles);
    }

    fn end(&mut self) -> impl Iterator<Item = Self::CmdBuffer> {
        let mut cmd_encoder =
            self.gpu_resource
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Command Encoder"),
                });
        std::mem::swap(&mut cmd_encoder, &mut self.cmd_encoder);
        std::iter::once(cmd_encoder.finish())
    }
}

impl<'a, T: Displayable<'a>> IRenderContext<'a, T, wgpu::RenderBundleEncoder<'a>>
    for RenderContext<'a, T>
{
    type Gpu = Gpu<'a, T>;
    type Err = RenderError;
    fn render(&mut self, gpu: &'a mut Self::Gpu) -> Result<(), Self::Err> {
        let surface = self.gpu_resource.surface.get_current_texture()?;
        let texture_view = surface.texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: None,
            dimension: None,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        });
        let bundles = std::mem::take(self.render_bundles.get_mut());
        {
            let mut rpass = self
                .cmd_encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &texture_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    label: Some("egui main render pass"),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            rpass.execute_bundles(bundles.iter());
        }
        gpu.submit_cmds(self.end());
        gpu.present();
        surface.present();
        Ok(())
    }
    fn draw<'b>(
        &'a mut self,
        model_db: &'b crate::ModelDB,
        mesh_db: &'b crate::MeshDB,
        material_db: &'b crate::MaterialDB,
    ) where
        'b: 'a,
    {
        let models = model_db.iter().map(|dat| dat.1);
        let iter: Vec<_> = models
            .map(|model| {
                let meshes = &model.meshes;
                let mats = &model.materials;

                meshes.iter().zip(mats.iter()).map(|(mesh, mat)| {
                    let mesh = mesh_db.get_by_id(mesh).unwrap();

                    let mut encoder = self.get_encoder();
                    encoder.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    encoder
                        .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    encoder
                })
            })
            .flatten()
            .collect();
        self.submit_encoders(iter.into_iter());
    }
}

impl IGpuCommandBuffer for wgpu::CommandEncoder {}

pub struct Gpu<'a, T>
where
    T: Displayable<'a>,
{
    gpu_resource: Arc<GpuResource<'a, T>>,
    node_id: &'a [u8; 6],
    commands: BTreeMap<CommandListIndex, wgpu::CommandBuffer>,
}

impl<'a, T> IGpu<'a, T> for Gpu<'a, T>
where
    T: Displayable<'a>,
{
    type Texture = Texture;
    type CmdBuffer = wgpu::CommandBuffer;
    type RenderPipeline = wgpu::RenderPipeline;
    type ComputePipeline = wgpu::ComputePipeline;
    type Err = GpuError;

    fn present(&mut self) {
        let cmds_map = std::mem::take(&mut self.commands);
        let cmds = cmds_map.into_iter().map(|entry| entry.1);
        self.gpu_resource.queue.submit(cmds);
    }

    fn create_texture(&self, texture_desc: TextureDesc) -> Result<Self::Texture, Self::Err> {
        let device = &self.gpu_resource.device;
        let queue = &self.gpu_resource.queue;
        let TextureDesc { data, .. } = texture_desc;
        let tex = Texture::from_bytes(device, queue, data, "Texture")?;
        Ok(tex)
    }

    fn submit_cmds(&mut self, cmds: impl Iterator<Item = Self::CmdBuffer>) {
        self.commands
            .extend(cmds.map(|cmd| (CommandListIndex::new(&self.node_id), cmd)));
    }

    fn create_pipeline(
        &self,
        shader_src: &str,
        pipeline_builder: impl IBuilder<Output = Self::RenderPipeline>,
    ) -> Result<Self::RenderPipeline, Self::Err> {
        todo!()
    }
}

#[derive(Error, Debug)]
pub enum GpuError {
    #[error("Error creating texture {0}")]
    TextureError(#[from] TextureError),
}

#[derive(Error, Debug)]
pub enum RenderError {
    #[error("{0}")]
    SurfaceError(#[from] wgpu::SurfaceError),
}
