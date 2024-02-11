use std::sync::Arc;

use wgpu::{CreateSurfaceError, RequestDeviceError};
use winit::window::Window;

use crate::HeatwaveConfig;

///Holds all relevant CPU objects for communication with the GPU
pub struct GpuConnection<'window> {
	//This is commented out as instance isn't currently needed outside of init. Comment this back in if instance is needed later down the line
	//instance: wgpu::Instance,
	//This is commented out as adapter isn't currently needed outside of init. Comment this back in if adapter is needed later down the line
	//adapter: wgpu::Adapter,

	///Reference to the presentable surface (Where the GPU draws to)
	surface: wgpu::Surface<'window>,
	///The configuration for the surface. Needed for future changes to the surface
	surface_config: wgpu::SurfaceConfiguration,
	///The connection to the physical graphics device.
	device: wgpu::Device,
	///Sends and executes command buffers on the GPU
	queue: wgpu::Queue,
	///The size of the current draw texture.
	texture_size: winit::dpi::PhysicalSize<u32>,
}

impl<'window> GpuConnection<'window> {
	///Creates a new GPU connection bound to the window provided.
	/// 
	///# Errors
	/// May error if an adapter that meets the requirements of the app is not found
	/// May error during requesting of a physical device
	/// May error during creation of the render surface. 
	///# Panics
	/// On iOS, this will panic if not called on the main thread.
	pub async fn new(window: Arc<Window>, config: &HeatwaveConfig<'_>) -> Result<Self, GpuConnectionError> 
	{
		let size = window.inner_size();

		assert!(size.width > 0 && size.height > 0, "Window size has to be above 0!");

		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends: wgpu::Backends::PRIMARY,
			..Default::default() //Allowing non-compliant adapters might be supported at a later date
		});
 
		let surface = match instance.create_surface(window.clone()) {
			Ok(surface) => surface,
			Err(error) => {
				return Err(GpuConnectionError { inner: GpuConnectionErrorKind::SurfaceCreation(error) } );
			}
		};

		let adapter = match instance.request_adapter(
			&wgpu::RequestAdapterOptions {
				power_preference: config.power_preference,
				compatible_surface: Some(&surface),
				force_fallback_adapter: false
			}
		).await {
			Some(adapter) => adapter,
			None => return Err(GpuConnectionError { inner: GpuConnectionErrorKind::CompatibleAdapterNotFound })
		};

		let future_device = adapter.request_device(
			&wgpu::DeviceDescriptor {
				required_features: config.features,
				required_limits: wgpu::Limits::default(),
				label: Some("Heatwave Adapter")
			}, None
		);

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

		let (device, queue) = match future_device.await {
			Ok(tuple) => tuple,
			Err(error) => { return Err(GpuConnectionError { inner: GpuConnectionErrorKind::DeviceRequest(error)}); }
		};

		surface.configure(&device, &surface_config);


		Ok(GpuConnection {
			//instance,
			//adapter,
			surface,
			surface_config,
			device,
			queue,
			texture_size: size
		})
	}
	pub fn device(&self) -> &wgpu::Device {
		&self.device
	}
	pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
		&self.surface_config
	}
}

pub struct GpuConnectionError {
	inner: GpuConnectionErrorKind
}
impl GpuConnectionError {
	pub fn kind(&self) -> &GpuConnectionErrorKind {
		&self.inner
	}
}
///Describes errors thrown by a [`GpuConnection`]
pub enum GpuConnectionErrorKind {
	SurfaceCreation(CreateSurfaceError),
	DeviceRequest(RequestDeviceError),
	CompatibleAdapterNotFound
}
