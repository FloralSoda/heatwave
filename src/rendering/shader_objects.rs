use super::ShaderObject;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ShaderVertex {
	position: [f32; 3]
}
impl ShaderVertex {
	const ATTRIBUTES: [wgpu::VertexAttribute; 1] =
		wgpu::vertex_attr_array![0 => Float32x3];
}
impl ShaderObject for ShaderVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &Self::ATTRIBUTES
		}
    }
}
pub mod primitives {
	
}
