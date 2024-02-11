use wgpu::{RenderPipelineDescriptor, ShaderModule, VertexBufferLayout};

pub trait Presenter {
	///Asks if any new rendering should occur this frame.
	/// 
	///Rendering is currently readonly\
	///In the future there will be a version of rendering that lets you mutate the Presenter, but for now you'll have to do all mutations on user events or write a custom event loop handler.
	fn should_render(&self) -> bool {
		true
	}

	///Requests the presenter to make draw calls to the GPU as to provide the next frame of the app
	/// 
	///Rendering is currently readonly.\
	///In the future there will be a version of rendering that lets you mutate the Presenter, but for now you'll have to do all mutations on user events or write a custom event loop handler.
	fn render(&self);
}

pub struct RenderHelper {
	
}

pub trait ShaderObject {
	fn layout() -> wgpu::VertexBufferLayout<'static>;
}


///Describes the properties for a new [`wgpu::RenderPipeline`] with some abstractions made for an easier user experience
/// 
/// 
///# Remarks
/// ## Conversion
/// When converting to [`wgpu::RenderPipelineDescriptor`], optional parameters are defaulted to None.\ The [`HeatwaveWindow`] will fill some of these in with more reasonable values, but you will need to handle it yourself if you need more granular control.
///# Assumptions
/// This struct makes quite a few assumptions to give an easier, more generalised descriptor for common usage\
/// The assumptions listed below are assumptions made in your typical game engine, which are replicated for Heatwave.
/// - The fragment shader overwrites old pixels
/// - The fragment shader accesses all colour channels
/// - The topology is a triangle list
/// - The front face is the counter clockwise side
/// - Culls the back faces
/// - Depth clipping is enabled, though will be disabled by default if the [`wgpu_types::Features::DEPTH_CLIP_CONTROL`] feature is enabled
/// - Polygons will rasterise in fill mode, by default.
/// - Conservative rasterisation is disabled by default, but enabled if [`wgpu_types::Features::CONSERVATIVE_RASTERIZATION`] is enabled (Any pixel touched by the polygon is filled, rather than only if most of the pixel is touched)
/// - Assumes only 1 view is needed 
///# Usage
///```rs
/// let descriptor = SimpleRenderPipelineDescriptor {
///     name: "My Render Pipeline",
///     vertex: &my_vertex_shader_module,
///     fragment: Some(&my_fragment_shader_module), //This can be the same as the vertex shader
///     vertex_entry_point: "vs_main",
///     fragment_entry_point: "fs_main",
///     vertex_buffer_format: config.format
/// }
///                          
/// let my_pipeline_id = my_heatwave_window.add_render_pipeline(descriptor);
///```
pub struct SimpleRenderPipelineDescriptor<'a> {
	///The name of the pipeline. Used for debugging
	pub name: &'a str,
	///The module for the vertex shader
	pub vertex: &'a ShaderModule,
	///The module for the fragment shader. Technically optional but it's recommended to have one
	pub fragment: Option<&'a ShaderModule>,
	///The name of the entry point function for the vertex shader within the shader module
	pub vertex_entry_point: &'a str,
	///The name of the entry point function for the fragment shader within the shader module
	pub fragment_entry_point: Option<&'a str>,
	///The format of any vertex buffers used by the pipeline
	pub vertex_buffer_format: &'a [VertexBufferLayout<'a>],
}
impl<'a> From<SimpleRenderPipelineDescriptor<'a>> for RenderPipelineDescriptor<'a> {
	///Converts a SimpleRenderPipelineDescriptor into a RenderPipelineDescriptor.
	/// 
	/// It is missing the following information:\
	/// * layout (Needs pipeline layout from the heatwave window)
	/// * color states of the fragment shader's render targets (Needs formats from the surfaces)
	/// 
	/// This information is automatically filled in when adding it to a HeatwaveWindow
    fn from(val: SimpleRenderPipelineDescriptor<'a>) -> Self {
        RenderPipelineDescriptor {
			label: Some(val.name),
			layout: None,
			vertex: wgpu::VertexState {
				module: val.vertex,
				entry_point: val.vertex_entry_point,
				buffers: val.vertex_buffer_format
			},
			fragment: val.fragment.map(|fragment| wgpu::FragmentState {
				module: fragment,
				entry_point: val.fragment_entry_point.expect("Expected fragment entry point name for the fragment shader"),
				targets: &[],
			}),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: Some(wgpu::Face::Back),
				unclipped_depth: false,
				polygon_mode: wgpu::PolygonMode::Fill,
				conservative: false
			},
			 depth_stencil: Some(wgpu::DepthStencilState {
			 	format: Texture::DEPTH_FORMAT,
				depth_write_enabled: true,
				depth_compare: wgpu::CompareFunction::Less,
				stencil: wgpu::StencilState::default(),
				bias: wgpu::DepthBiasState::default()
			}),
			multisample: wgpu::MultisampleState {
				count: 8,
				mask: !0,
				alpha_to_coverage_enabled: false
			},
			multiview: None
		}
    }
}

pub struct Texture {
	pub texture: wgpu::Texture,
	pub view: wgpu::TextureView,
	pub sampler: wgpu::Sampler
}
impl Texture {
	pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

	//Todo: Create depth texture function
}
