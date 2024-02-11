//! A cross platform rendering engine designed for simplicity of use with maximum freedom of control.
//! Uses [wgpu](https://docs.rs/wgpu/latest/wgpu/) to communicate with the GPU. Add it as a dependency to use more advanced, or non rendering related features.
//!
//! To start using the engine, create a [`HeatwaveApp`] 
//! 
//! Though this project uses wgpu in its backend, it doesn't currently fully support wasm.

use std::{collections::HashMap, sync::{Arc, RwLock}};

use gpu::{GpuConnection, GpuConnectionError};
use rendering::Presenter;
use wgpu::{BindGroupLayoutDescriptor, ComputePipelineDescriptor, Features, PipelineLayout, PushConstantRange, RenderPipelineDescriptor};
use winit::{error::{EventLoopError, OsError}, event::Event, event_loop::{EventLoop, EventLoopWindowTarget}, window::{Fullscreen, Window, WindowBuilder, WindowButtons, WindowLevel}};

///Contains any structs relating to GPU connections and GPU objects
pub mod gpu;
///Contains any structs relating to rendering and presentation 
pub mod rendering;

///Represents an app in Heatwave, with references to the presenter, window and GPU bindings/

pub struct HeatwaveApp<'a> {
	connection: GpuConnection<'a>,
	
	buffers: HashMap<usize, wgpu::Buffer>,
	next_buffer_id: usize,

	render_pipelines: HashMap<usize, wgpu::RenderPipeline>,
	compute_pipelines: HashMap<usize, wgpu::ComputePipeline>,
	next_render_id: usize,
	next_compute_id: usize,
	
	pipeline_layout: PipelineLayout,

	event_loop: Option<EventLoop<()>>,
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

	pub async fn new<'b>(config: HeatwaveConfig<'b>) -> Result<Self, HeatwaveInitialiseError> {
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
	pub async fn new_with_window<'b>(builder: WindowBuilder, config: HeatwaveConfig<'b>) -> Result<Self, HeatwaveInitialiseError> {
		let event_loop = EventLoop::new()?;
		let window = builder.build(&event_loop)?;
		let window_ref = Arc::new(window);

		let connection_future = GpuConnection::new(window_ref.clone(), &config);

		let connection = connection_future.await?;

		let pipeline_layout = connection.device().create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Heatwave Render Pipeline Layout"),
			bind_group_layouts: &[], //todo: Bind groups https://sotrh.github.io/learn-wgpu/beginner/tutorial5-textures/#the-bindgroup
			push_constant_ranges: config.push_constants
		});
		Ok(HeatwaveApp {
			window: window_ref,
			event_loop: Some(event_loop),
			buffers: HashMap::new(),
			next_buffer_id: 0,
			render_pipelines: HashMap::new(),
			compute_pipelines: HashMap::new(),
			next_compute_id: 0,
			next_render_id: 0,
			connection,
			pipeline_layout
		})
	}

	///Adds a new render pipeline to the heatwave window using the descriptor provided.
	/// 
	///Fills in the layout with this heatwave instance's pipeline layout if none is provided
	/// 
	///Fills in a render target for the fragment shader is none are provided but a fragment shader is provided.\
	///Uses a ColorTarget with the format of the heatwave instance's surface, a blend mode of Replace and targets all colour channels. 
	pub fn add_render_pipeline<'b>(&mut self, descriptor: impl Into<RenderPipelineDescriptor<'b>>) -> usize {
		let mut desc: RenderPipelineDescriptor = descriptor.into();
		if desc.layout.is_none() {
			desc.layout = Some(&self.pipeline_layout);
		}

		let targets;
		if let Some(fragment) = &mut desc.fragment {
			if fragment.targets.is_empty() {
				targets = [Some(wgpu::ColorTargetState {
					format: self.connection.surface_config().format,
					blend: Some(wgpu::BlendState::REPLACE),
					write_mask: wgpu::ColorWrites::ALL
				})];
				fragment.targets = &targets;
			}
		}
		let pipeline = self.connection.device().create_render_pipeline(&desc);

		self.render_pipelines.insert(self.next_render_id, pipeline);
		self.next_render_id += 1;
		self.next_render_id - 1
	}
	///Adds a new compute pipeline to the heatwave window using the descriptor provided.
	/// 
	///Fills in the layout with this heatwave instance's pipeline layout if none is provided
	pub fn add_compute_pipeline<'b>(&mut self, descriptor: impl Into<ComputePipelineDescriptor<'b>>) -> usize {
		let mut desc: ComputePipelineDescriptor = descriptor.into();
		if desc.layout.is_none() {
			desc.layout = Some(&self.pipeline_layout);
		}
		let pipeline = self.connection.device().create_compute_pipeline(&desc);

		self.compute_pipelines.insert(self.next_compute_id, pipeline);
		self.next_compute_id += 1;
		self.next_compute_id - 1
	}

	///Finalises this app, returning a runnable version.\
	///Changes can be made later on, but they must be done either during a render operation or on a user input.
	pub fn build_runner<P>(mut self, presenter: P) -> HeatwaveRunner<'a, P> 
	where
		P: Presenter {
		HeatwaveRunner {
			presenter,
			event_loop: self.event_loop.take().expect("Expected valid event loop inside heatwave app"),
			app: self
		}
	}

	///Returns a thread safe reference to the window
	pub fn window(&self) -> Arc<Window> {
		self.window.clone()
	}
}

pub struct HeatwaveRunner<'a, Handler> where
	Handler: Presenter {
	presenter: Handler,
	event_loop: EventLoop<()>,
	app: HeatwaveApp<'a>,
}
impl<'a, Handler> HeatwaveRunner<'a, Handler> where
Handler: Presenter {
	///Runs the app, opening the window and starting the event loop. This runs with some premade event loop handler and instructions that make use of the presenter in the documented ways.\ 
	/// The default event loop handler is designed to be stable, lightweight and a suitable fit for most applications. It should handle edge cases gracefully.
	/// 
	/// If you need a custom event loop handler, see [`HeatwaveApp::run_custom`]
	/// 
	/// This consumes the HeatwaveApp object, and occupies the running thread.\
	/// Rendering and user input handling are on separate threads.
	pub fn run(self) {

	}
	///Runs the app, opening the window and starting the event loop. This runs with the provided event loop handler, and as such makes no guarantees about the interaction with the presenter.
	pub fn run_custom<HandleFn>(self, handler: HandleFn) -> Result<(), EventLoopError> where
	HandleFn: Fn(EventArgs, &HeatwaveApp, Arc<RwLock<Handler>>)
	{
		let presenter_ref = Arc::new(RwLock::new(self.presenter));

		self.event_loop.run(move |event, target| {
			handler(EventArgs {
				event,
				target
			}, &self.app, presenter_ref.clone());
		})
	}
}

///Contains the arguments for an event loop event.
pub struct EventArgs<'a> {
	///The event that was called, sent by winit
	event: Event<()>,
	///The target associating the sender with this event loop
	target: &'a EventLoopWindowTarget<()>,
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
pub struct HeatwaveConfig<'a> {
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
	/// Adds bind groups to shader sets.
	/// 
	/// Defaults to an empty list (no bind groups)
	pub bind_groups: &'a [BindGroupLayoutDescriptor<'a>],
	/// Adds push constant ranges to shaders.
	/// 
	/// Defaults to an empty list.
	/// 
	/// **The feature "Feature::PUSHCONSTANTS` must be enabled for this to work**
	pub push_constants: &'a [PushConstantRange]
}
impl<'a> Default for HeatwaveConfig<'a> {
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
			bind_groups: &[],
			push_constants: &[]
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
