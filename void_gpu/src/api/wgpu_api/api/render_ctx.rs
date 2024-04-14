use wgpu::DynamicOffset;

use crate::api::wgpu_api::camera::Camera;
use crate::api::{DrawModel, IBindGroup, IContext, IRenderContext};
use crate::model;

use super::CtxOut;
use std::marker::PhantomData;
use std::ops::Range;

impl IBindGroup for wgpu::BindGroup {}

impl<'a> IContext for RenderCtx<'a>
where
{
    type Out = CtxOut<'a>;

    fn new() -> Self {
        Self::default()
    }

    fn finish(self) -> Self::Out {
        CtxOut::Render(self)
    }
}

impl<'a, 'b> DrawModel<'a> for RenderCtx<'b>
where
    'a:'b
{
    type Camera = Camera;

    fn draw_mesh(
        &mut self,
        mesh: &'a model::Mesh,
        material: &'a model::Material,
        camera_bind_group: &'a Self::Camera,
    ) {
        self.draw_mesh_instanced(mesh, material, 0..1, camera_bind_group)
    }
    fn draw_model(&mut self, model: &'a model::Model, camera_bind_group: &'a Self::Camera) {
        self.draw_model_instanced(model, 0..1, camera_bind_group)
    }
    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a model::Mesh,
        material: &'a model::Material,
        instances: Range<u32>,
        camera_bind_group: &'a Self::Camera,
    ) {
        self.set_vertex_buffer(0, &mesh.vertex_buffer);
        self.set_index_buffer(1, &mesh.index_buffer);
        self.set_bind_group(0, &material.bind_group);
        self.set_bind_group(1, &camera_bind_group);
        self.draw(0..mesh.num_elements, 0, instances);
    }
    fn draw_model_instanced(
        &mut self,
        model: &'a model::Model,
        instances: Range<u32>,
        camera_bind_group: &'a Self::Camera,
    ) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            self.draw_mesh_instanced(mesh, material, instances.clone(), camera_bind_group);
        }
    }
}

impl<'a> IRenderContext<'a> for RenderCtx<'a>
where
{
    type Pipeline = wgpu::RenderPipeline;
    type BindGroup = wgpu::BindGroup;
    type Buffer = wgpu::Buffer;

    fn set_pipeline(&mut self, pipeline: &'a Self::Pipeline) {
        self.pipeline = Some(pipeline);
    }

    fn set_bind_group(&mut self, slot: u32, group: &'a Self::BindGroup) {
        self.bind_groups.push((slot, group, &[]));
    }

    fn draw(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.draw_cmd = Some(DrawCmd {
            indices,
            instances,
            base_vertex,
        });
    }
    fn set_index_buffer(&mut self, _slot: u32, buffer: &'a Self::Buffer) {
        self.index_buffer = Some(buffer)
    }
    fn set_vertex_buffer(&mut self, slot: u32, buffer: &'a Self::Buffer) {
        self.vertex_buffer = Some((slot, buffer))
    }
}

#[derive(Default)]
pub struct RenderCtx<'a>
where
{
    pub(crate) bind_groups: Vec<(u32, &'a wgpu::BindGroup, &'a [DynamicOffset])>,
    pub(crate) vertex_buffer: Option<(u32, &'a wgpu::Buffer)>,
    pub(crate) index_buffer: Option<&'a wgpu::Buffer>,
    pub(crate) pipeline: Option<&'a wgpu::RenderPipeline>,
    pub(crate) draw_cmd: Option<DrawCmd>,
    _phantom: PhantomData<&'a ()>,
}

#[derive(Default)]
pub(crate) struct DrawCmd {
    pub(super) indices: Range<u32>,
    pub(super) base_vertex: i32,
    pub(super) instances: Range<u32>,
}