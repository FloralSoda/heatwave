use std::sync::Arc;

use winit::window::Window;

use crate::HeatwaveConfig;

///Holds all relevant CPU objects for communication with the GPU
/// 
///The OS window is stored here, as well, as the GPU connections are bound to it.
pub struct GpuConnection<'window> {
	instance: wgpu::Instance,
	surface: wgpu::Surface<'window>,
	surface_config: wgpu::SurfaceConfiguration,
	adapter: wgpu::Adapter,
	device: wgpu::Device,
	queue: wgpu::Queue,
	texture_size: winit::dpi::PhysicalSize<u32>,

	///Window is stored here due to unsafe references with the surface
	///It must be after `surface` due to Rust's drop order guarantee (Objects are dropped in reverse order)
	window: Arc<Window>
}

impl<'window> GpuConnection<'window> {
	///Creates a new GPU connection bound to the window provided.
	/// 
	///Takes ownership of the window.
	pub async fn new(window: Window, config: &HeatwaveConfig) -> Self 
	{
		let size = window.inner_size();

		let window_ref = Arc::new(window);

		assert!(size.width > 0 && size.height > 0, "Window size has to be above 0!");

		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends: wgpu::Backends::PRIMARY,
			..Default::default() //Allowing non-compliant adapters might be supported at a later date
		});

		//This is safe as window is owned by this struct and is ensured to deconstruct before surface does by Rust's drop order
		//This is safe in Rust 1.75.0
		let surface = instance.create_surface(window_ref.clone())
									.expect("Expected a window that a gpu surface could build from");

		let adapter = instance.request_adapter(
			&wgpu::RequestAdapterOptions {
				power_preference: config.power_preference(),
				compatible_surface: Some(&surface),
				force_fallback_adapter: false
			}
		).await.expect("No adapter was found that met the requirements");

		let (device, queue) = adapter.request_device(
			&wgpu::DeviceDescriptor {
				required_features: config.gpu_features(),
				required_limits: wgpu::Limits::default(),
				label: Some("Heatwave Adapter")
			}, None
		).await.expect("Graphical features requested are not supported");

		let surface_capabilities = surface.get_capabilities(&adapter);
		let surface_format = surface_capabilities.formats.iter()
			.copied()
			.find(|f| f.is_srgb()) 
			.unwrap_or(surface_capabilities.formats[0]);

		let surface_config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT, //This isn't configurable, since this is a rendering engine
			format: surface_format,
			width: size.width,
			height: size.height,
			present_mode: wgpu::PresentMode::AutoVsync, //VSync is always enabled for best visuals if supported.
			alpha_mode: surface_capabilities.alpha_modes[0], //Take the default alpha type.
			view_formats: vec![], //Todo: I don't really understand this
			desired_maximum_frame_latency: 2
		};

		surface.configure(&device, &surface_config);


		GpuConnection {
			instance,
			surface,
			surface_config,
			adapter,
			device,
			queue,
			texture_size: size,
			window: window_ref
		}
	}
}
