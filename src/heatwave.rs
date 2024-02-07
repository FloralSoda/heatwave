//! A cross platform rendering engine designed for simplicity of use with maximum freedom of control.
//! Uses [wgpu](https://docs.rs/wgpu/latest/wgpu/) to communicate with the GPU. Add it as a dependency to use more advanced, or non rendering related features.
//!
//! To start using the engine, create a [`HeatwaveApp`] 
//! 
//! Though this project uses wgpu in its backend, it doesn't currently fully support wasm.

use std::{collections::HashMap, sync::Arc};

use gpu::{GpuConnection, GpuConnectionError};
use wgpu::Features;
use winit::{error::{EventLoopError, OsError}, event_loop::EventLoop, window::{Fullscreen, Window, WindowBuilder, WindowButtons, WindowLevel}};

///Contains any structs relating to GPU connections and GPU objects
pub mod gpu;
///Contains any structs relating to rendering and presentation 
pub mod rendering;

///Represents an app in Heatwave, with references to the presenter, window and GPU bindings/
///
pub struct HeatwaveApp<'a> {
	connection: GpuConnection<'a>,
	
	buffers: HashMap<usize, wgpu::Buffer>,
	next_buffer_id: usize,

	render_pipelines: HashMap<usize, wgpu::RenderPipeline>,
	compute_pipelines: HashMap<usize, wgpu::ComputePipeline>,
	next_render_id: usize,
	next_compute_id: usize, 

	event_loop: EventLoop<()>,
	window: Arc<Window>
}
impl<'a> HeatwaveApp<'a> {
	#[cfg(target_arch = "wasm32")]
	///Initialises env_logger, used by Heatwave and wgpu. If the library is compiled for wasm, the browser's console is used instead.
	/// 
	///When not compiling for wasm32, this function does not require any inputs
	pub fn init_logger(level: log::Level) {
		std::panic::set_hook(Box::new(console_error_panic_hook::hook));
		console_log::init_with_level(level).expect("Couldn't initialise logger");
	}
	#[cfg(not(target_arch = "wasm32"))]
	///Initialises env_logger, used by Heatwave and wgpu. If the library is compiled for wasm, the browser's console is used instead.
	/// 
	///When compiling for wasm32, this function requires a log level
	pub fn init_logger() {
		env_logger::init();
	}

	pub async fn new(config: HeatwaveConfig) -> Result<Self, HeatwaveInitialiseError> {
		let mut window = WindowBuilder::new()
			.with_title(&config.name)
			.with_inner_size(config.default_size)
			.with_transparent(config.transparent)
			.with_resizable(config.resizeable)
			.with_enabled_buttons(config.window_buttons)
			.with_fullscreen(config.fullscreen_mode.clone()) //This isn't accessed anywhere else, so there's no problems
			.with_maximized(config.maximised)
			.with_visible(config.visible)
			.with_blur(config.blurred_background)
			.with_decorations(config.decorations)
			.with_window_level(config.window_layer)
			.with_active(config.active_on_open);

		if let Some(size) = config.maximum_size {
			window = window.with_max_inner_size(size);
		}
		if let Some(size) = config.minimum_size {
			window = window.with_min_inner_size(size);
		}
		if let Some(position) = config.starting_position {
			window = window.with_position(position);
		}
		
		HeatwaveApp::new_with_window(window, config).await
	}
	///Attaches a new HeatwaveWindow to a pre-existing WindowBuilder. 
	/// 
	///**Window-related configuration in HeatwaveConfig is ignored**
	pub async fn new_with_window(builder: WindowBuilder, config: HeatwaveConfig) -> Result<Self, HeatwaveInitialiseError> {
		let event_loop = EventLoop::new()?;

		let window = Arc::new(builder.build(&event_loop)?);

		let connection_future = GpuConnection::new(window.clone(), &config);
		Ok(HeatwaveApp {
			window,
			event_loop,
			buffers: HashMap::new(),
			next_buffer_id: 0,
			render_pipelines: HashMap::new(),
			compute_pipelines: HashMap::new(),
			next_compute_id: 0,
			next_render_id: 0,
			connection: connection_future.await?,
		})
	}
}

/// Configuration for the heatwave window and graphics.
/// In most situations, using the defaults will work for your needs
/// 
/// # Usage
/// ```rs
/// let config = HeatwaveConfig {
///     name: String::from("My Heatwave Window"),
///     maximised: true,
///     ..Default::default()
/// }
/// 
/// let my_app = HeatwaveApp::new(config)?;
/// //voila, you have your window!
/// ```
#[derive(Clone, Debug)]
pub struct HeatwaveConfig {
	///Preference for the power of the physical device used for rendering.
	/// 
	///Defaults to HighPerformance
	pub power_preference: wgpu::PowerPreference,
	///The title of the window. Also used in diagnostics.
	/// 
	///Defaults to "Heatwave App"
	pub name: String,
	///Which additional GPU features you require (Features that aren't supported in all contexts)
	/// 
	///Defaults to empty (No additional features)
	pub features: wgpu::Features,
	/// The size the window opens with (excluding decorations)
	/// 
	/// Defaults to 200x200
	pub default_size: winit::dpi::PhysicalSize<u32>,
	/// The smallest size the window can be (excluding decorations)
	/// 
	/// Defaults to the running platform's minimum size
	pub minimum_size: Option<winit::dpi::PhysicalSize<u32>>,
	/// The largest size the window can be (excluding decorations)
	/// 
	/// Defaults to no maximum
	pub maximum_size: Option<winit::dpi::PhysicalSize<u32>>,
	/// Where on the screen the window should open, typically positioned at the top left of the window
	/// 
	/// Defaults to whatever the platform picks.
	/// 
	/// # Platform specific
	/// * **MacOS** does not consider the window decorations for positioning.
	/// * **Windows** may have gap between the decoration and the position.
	pub starting_position: Option<winit::dpi::PhysicalPosition<u32>>,
	/// Tells the OS that the window should be transparent\
	/// You may have the alpha buffer on some systems
	/// 
	/// Defaults to false
	pub transparent: bool,
	/// If `true`, the window can be resized by the user if the platform allows it.
	/// 
	/// Defaults to `true`
	pub resizeable: bool,
	/// Which title bar buttons the window should have.
	/// 
	/// Defaults to `WindowButtons:all`
	pub window_buttons: WindowButtons,
	/// Whether or not the app should be windowed or fullscreen. If fullscreen, exclusive or borderless fullscreen.
	/// 
	/// Defaults to `None`, meaning windowed mode. See [the Fullscreen enum](https://docs.rs/winit/latest/winit/window/enum.Fullscreen.html) from winit for more information.
	pub fullscreen_mode: Option<Fullscreen>,
	/// Whether the window should be maximised or not on open
	/// 
	/// Defaults to `false`
	pub maximised: bool,
	/// Whether the window should be visible when opened.
	/// 
	/// Defaults to showing (`true`)
	pub visible: bool,
	/// Whether the window's background should be blurred by the system.
	/// 
	/// Defaults to `false`
	/// 
	/// This feature is mostly unsupported, currently only working on MacOS and Wayland.\
	/// Wayland requires the "org_kde_kwin_blur_manager" protocol to work.
	pub blurred_background: bool,
	/// Whether the window should have decorations (border, title bar etc.) if the platform has them
	/// 
	/// Defaults to `true`
	pub decorations: bool,
	/// Sets if the window should render under or above all windows, or to just behave normally.
	/// 
	/// Default is `Normal`
	pub window_layer: WindowLevel,
	/// Sets if the window should automatically focus on open
	/// 
	/// Defaults to `false`, though this behaviour might vary on platform.
	pub active_on_open: bool,
}
impl Default for HeatwaveConfig {
    fn default() -> Self {
        Self { 
			power_preference: wgpu::PowerPreference::HighPerformance, //Default to the best GPU it can find, as this is the most common one to ask for
			name: String::from("Heatwave App"),
			features: Features::empty(),
			default_size: winit::dpi::PhysicalSize::new(200,200),
			minimum_size: None,
			maximum_size: None,
			transparent: false,
            starting_position: None,
            resizeable: true,
            window_buttons: WindowButtons::all(),
            fullscreen_mode: None,
            maximised: false,
			visible: true,
            blurred_background: false,
            decorations: true,
            window_layer: WindowLevel::Normal,
			active_on_open: true,
		}
    }
}

 pub enum HeatwaveInitialiseError {
	GpuConnection(GpuConnectionError),
	EventLoopCreation(EventLoopError),
	WindowCreation(OsError)
}
impl From<GpuConnectionError> for HeatwaveInitialiseError {
    fn from(value: GpuConnectionError) -> Self {
        HeatwaveInitialiseError::GpuConnection(value)
    }
}
impl From<EventLoopError> for HeatwaveInitialiseError {
    fn from(value: EventLoopError) -> Self {
        HeatwaveInitialiseError::EventLoopCreation(value)
    }
}
impl From<OsError> for HeatwaveInitialiseError {
    fn from(value: OsError) -> Self {
        HeatwaveInitialiseError::WindowCreation(value)
    }
}
