pub use crate::api::camera::Camera;
use crate::api::BufferId;

pub trait ICamera {
    fn build_view_projection_matrix(&self) -> na::Matrix4<f32>;
    fn get_buffer(&self) -> BufferId;
    fn get_bind_group(&self) -> usize;
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: na::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj<T: ICamera>(&mut self, camera: &T) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

pub trait UpdateCamera<'a> {
    fn update_camera(&mut self, uniform: &'a [CameraUniform]);
}
