//! A cross platform rendering engine designed for simplicity of use with maximum freedom of control.
//! Uses [wgpu](https://docs.rs/wgpu/latest/wgpu/) to communicate with the GPU. Add it as a dependency to use more advanced, or non rendering related features.
//!
//! To start using the engine, create a [`HeatwaveApp`] 
//! 
//! Though this project uses wgpu in its backend, it doesn't currently fully support wasm.

use std::{collections::HashMap, path::PathBuf, sync::{mpsc::channel, Arc, RwLock}, thread};

use gpu::{GpuConnection, GpuConnectionError};
use rendering::{AnalogAxisEventArgs, KeyPressEventArgs, MousePressEventArgs, MouseScrollEventArgs, Presenter, RenderHelper};
use wgpu::{util::{BufferInitDescriptor, DeviceExt}, BindGroupLayoutDescriptor, BufferDescriptor, ComputePipelineDescriptor, Features, PipelineLayout, PushConstantRange, RenderPipelineDescriptor};
use winit::{dpi::{PhysicalPosition, PhysicalSize}, error::{EventLoopError, OsError}, event::{AxisId, DeviceId, ElementState, Event, Ime, InnerSizeWriter, KeyEvent, Modifiers, MouseButton, MouseScrollDelta, Touch, TouchPhase}, event_loop::{EventLoop, EventLoopWindowTarget}, window::{Fullscreen, Window, WindowBuilder, WindowButtons, WindowLevel}};
use log::{error,warn};
use rayon::prelude::*;

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
	
	skybox: wgpu::Color,
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
			pipeline_layout,
			skybox: config.skybox
		})
	}

	///Adds a new buffer to the heatwave window using the descriptor provided.
	/// 
	///Returns the ID of the buffer, for later access
	///# Substitution
	/// Has no substitution behaviour.
	pub fn add_buffer<'b>(&mut self, descriptor: impl Into<BufferDescriptor<'b>>) -> usize {
		let desc = descriptor.into();

		let buffer = self.connection.device().create_buffer(&desc);

		self.buffers.insert(self.next_buffer_id, buffer);
		self.next_buffer_id += 1;
		self.next_buffer_id - 1
	}
	///Adds a new buffer to the heatwave window using the descriptor provided.
	///Specifically for buffers that have starting values
	/// 
	///Returns the ID of the buffer, for later access
	/// 
	/// # Substitution
	/// Has no substitution behaviour.
	pub fn add_buffer_with_defaults<'b>(&mut self, descriptor: impl Into<BufferInitDescriptor<'b>>) -> usize {
		let desc = descriptor.into();

		let buffer = self.connection.device().create_buffer_init(&desc);

		self.buffers.insert(self.next_buffer_id, buffer);
		self.next_buffer_id += 1;
		self.next_buffer_id - 1
	}
	///Adds a new render pipeline to the heatwave window using the descriptor provided.
	/// 
	///Returns the ID of the pipeline, for calling later
	/// 
	/// # Substitution
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
	///Returns the ID of the pipeline, for calling later
	/// 
	/// # Substitution
	/// Fills in the layout with this heatwave instance's pipeline layout if none is provided
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

	pub fn connection(&self) -> &GpuConnection {
		&self.connection
	}
}

pub struct HeatwaveRunner<'a, Handler> where
	Handler: Presenter {
	presenter: Handler,
	event_loop: EventLoop<()>,
	app: HeatwaveApp<'a>,
}
impl<'a, Handler> HeatwaveRunner<'a, Handler> where
Handler: Presenter + Send {
	///The handler used by [`HeatwaveRunner::run`]
	/// 
	///Public only for people using [`HeatwaveRunner::run_custom`] to use as a fallback/base implementation for their custom handlers
	pub fn default_handler(args: EventArgs, app: &HeatwaveApp, presenter: &mut Handler) {
		let (sender_presenter, receiver_user) = channel();
		let (sender_user, receiver_presenter) = channel();

		thread::spawn(move || {
			//Supplementary data, used to set up additional tracked information
			let mut mouse_position: PhysicalPosition<f64> = PhysicalPosition::new(0.0,0.0);

			loop {
				match receiver_user.recv() {
					Err(_) => { 
						warn!("Main thread disconnected early!");
						break;
					},
					Ok(event) => {
						match event {
							WindowEvent::CloseRequested | WindowEvent::Destroyed => { presenter.on_exit(); sender_user.send(UserEvent::ReadyToClose); }
							WindowEvent::CursorEntered { device_id } => presenter.on_cursor_enter(device_id),
							WindowEvent::CursorMoved { device_id, position } => {presenter.on_cursor_move(device_id, position); mouse_position = position;},
							WindowEvent::CursorLeft { device_id } => presenter.on_cursor_leave(device_id),
							WindowEvent::AxisMotion { device_id, axis, value } => presenter.on_analog_axis_motion(AnalogAxisEventArgs { device_id, axis, value }),
							WindowEvent::DroppedFile(path) => presenter.on_file_drop(path),
							WindowEvent::HoveredFile(path) => presenter.on_file_hover(path),
							WindowEvent::HoveredFileCancelled => presenter.on_file_hover_cancel(),
							WindowEvent::Focused(focus) => presenter.on_window_focus_changed(focus),
							WindowEvent::Ime(ime) => presenter.on_ime_input(ime),
							WindowEvent::KeyboardDown { device_id, event, is_synthetic } => presenter.on_key_press(KeyPressEventArgs { device_id, event, is_synthetic }),
							WindowEvent::KeyboardUp { device_id, event, is_synthetic } => presenter.on_key_release(KeyPressEventArgs { device_id, event, is_synthetic }),
							WindowEvent::ModifiersChanged(mods) => presenter.on_modifier_changed(mods),
							WindowEvent::MouseDown { device_id, state, button } => presenter.on_mouse_down(MousePressEventArgs { device_id, state, button, position: mouse_position }),
							WindowEvent::MouseUp { device_id, state, button } => presenter.on_mouse_up(MousePressEventArgs { device_id, state, button, position: mouse_position }),
							WindowEvent::MouseWheel { device_id, delta, phase } => presenter.on_mouse_scroll( MouseScrollEventArgs { device_id, delta, phase }),
							WindowEvent::Occluded(occlude) => presenter.on_occlusion(occlude),
							WindowEvent::RequestRenderData => { sender_user.send(UserEvent::RenderDataPrepared(presenter.package_render_data())); },
							WindowEvent::Resized(size) => presenter.on_window_resize(size),
							WindowEvent::Moved(position) => presenter.on_window_move(position),
							WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer } => presenter.on_scale_factor_change(scale_factor, inner_size_writer),
							WindowEvent::Touch(touch) => presenter.on_touch(touch),
							WindowEvent::Unknown => warn!("Unknown event from window!")
						}
					}
				}
			}
		}).join().expect("User event handling thread failed!");

		match args.event {
			Event::WindowEvent { window_id, event } if window_id == app.window().id() => {
				match event {
					winit::event::WindowEvent::RedrawRequested => {
						sender_presenter.send(WindowEvent::RequestRenderData);
						loop {
							let response = receiver_presenter.recv().expect("The user event thread exited early! Cannot render frame");
							if let UserEvent::RenderDataPrepared(render_data) = response {
								Handler::render(render_data, Some(RenderHelper::new(app)));
								break;
							}

 						    error!("Desync between render and user event thread! Ignoring non-render data response from user");
						}
					},
					winit::event::WindowEvent::CloseRequested => {

						args.target.exit();
						loop {
							match receiver_presenter.recv() {
								Err(_) => warn!("User thread closed improperly!"),
								Ok(message) => { match message {
									UserEvent::ReadyToClose => break,
									_ => error!("Desync between render and user event thread! Ignoring non-closing response from user"),
								}}
							}
						}
					},
					event => {
						sender_presenter.send(event.into());
					}
				} 
			},
			_ => {}
		};
	}
	///Runs the app, opening the window and starting the event loop. This runs with some premade event loop handler and instructions that make use of the presenter in the documented ways.\ 
	/// The default event loop handler is designed to be stable, lightweight and a suitable fit for most applications. It should handle edge cases gracefully.
	/// 
	/// If you need a custom event loop handler, see [`HeatwaveApp::run_custom`]
	/// 
	/// This consumes the HeatwaveApp object, and occupies the running thread.\
	/// Rendering and user input handling are on separate threads.
	pub fn run(self) -> Result<(), EventLoopError> {
		self.run_custom(Self::default_handler)
	}
	///Runs the app, opening the window and starting the event loop. This runs with the provided event loop handler, and as such makes no guarantees about the interaction with the presenter.
	pub fn run_custom<HandleFn>(mut self, handler: HandleFn) -> Result<(), EventLoopError> where
	HandleFn: Fn(EventArgs, &HeatwaveApp, &mut Handler)
	{
		self.event_loop.run(move |event, target| {
			handler(EventArgs {
				event,
				target
			}, &self.app, &mut self.presenter);
		})
	}
}

///Mirrors [`winit::event::WindowEvent`] and adds some Heatwave specific communication events
#[derive(Clone, PartialEq, Debug)]
pub enum WindowEvent {
	///Raises when the window is resized. Contains the new client dimensions
	Resized(PhysicalSize<u32>),
	///Raises when the window is moved. Contains the window's new coordinates
	Moved(PhysicalPosition<i32>),
	///Raises when the window is trying to close
	CloseRequested,
	///Raises when the window was destroyed
	Destroyed,
	///Raises when a file hover is dropped on the window
	DroppedFile(PathBuf),
	///Raises when one or more files are hovered over the window. Will raise for each file selected
	HoveredFile(PathBuf),
	///Raises when a file hover is cancelled. Will raise only once regardless of file count
	HoveredFileCancelled,
	///Raises when the window is focused or unfocused (If it's the current operating window)
	Focused(bool),
	///Raises when a key is pressed on the keyboard
	KeyboardDown { device_id: DeviceId, event:KeyEvent, is_synthetic: bool},
	///Raises when a key is released on the keyboard
	KeyboardUp { device_id: DeviceId, event:KeyEvent, is_synthetic: bool},
	///Raises when the keyboard modifiers have changed (Shift, Ctrl, etc.)
	ModifiersChanged(Modifiers),
	///Raises when typing a character via Ime
	Ime(Ime),
	///Raises when the mouse cursor moves inside the window
	CursorMoved { device_id: DeviceId, position: PhysicalPosition<f64>},
	///Raises when the mouse cursor enters the bounds of the window
	CursorEntered { device_id: DeviceId },
	///Raises when the mouse cursor leaves the bounds of the window
	CursorLeft { device_id: DeviceId },
	///Raises when a mouse wheel or touchpad scroll occurred
	MouseWheel { device_id: DeviceId, delta: MouseScrollDelta, phase: TouchPhase },
	///Raises when a mouse button was pressed
	MouseDown { device_id: DeviceId, state: ElementState, button: MouseButton },
	///Raises when a mouse button was raised
	MouseUp { device_id: DeviceId, state: ElementState, button: MouseButton },
	///Raises when some analog control device (such as a joystick or gamepad) changes stick position
	AxisMotion { device_id: DeviceId, axis: AxisId, value: f64 },
	///Raises when the window has been touched on a touch compatible screen
	Touch(Touch),
	///Raises when the window's scale factor changes (DPI change due to resolution change, scale change or just moving to a different screen)
	ScaleFactorChanged { scale_factor: f64, inner_size_writer: InnerSizeWriter },
	///Raises when the window is totally hidden from view. Not supported on Android or Windows.
	Occluded(bool),
	///This is raised for events that aren't handled by Heatwave (usually due to being widely unsupported, such as Mac forcetouch events)
	Unknown,
	
	///Raised when the window needs render data to render the next frame. 
	///
	///In the default event handler, the event loop is blocked until [`UserEvent::RenderDataPrepared`] is sent back 
	RequestRenderData
}
impl From<winit::event::WindowEvent> for WindowEvent {
	fn from(value: winit::event::WindowEvent) -> Self {
		match value {
			winit::event::WindowEvent::Resized(size) => Self::Resized(size),
			winit::event::WindowEvent::Moved(position) => Self::Moved(position),
			winit::event::WindowEvent::CloseRequested => Self::CloseRequested,
			winit::event::WindowEvent::Destroyed => Self::Destroyed,
			winit::event::WindowEvent::DroppedFile(path) => Self::DroppedFile(path),
			winit::event::WindowEvent::HoveredFile(path) => Self::HoveredFile(path),
			winit::event::WindowEvent::HoveredFileCancelled => Self::HoveredFileCancelled,
			winit::event::WindowEvent::Focused(focus) => Self::Focused(focus),
			winit::event::WindowEvent::KeyboardInput { device_id, event, is_synthetic } if event.state.is_pressed() => Self::KeyboardDown { device_id, event, is_synthetic },
			winit::event::WindowEvent::KeyboardInput { device_id, event, is_synthetic } => Self::KeyboardUp { device_id, event, is_synthetic },
			winit::event::WindowEvent::ModifiersChanged(mods) => Self::ModifiersChanged(mods),
			winit::event::WindowEvent::Ime(ime) => Self::Ime(ime),
			winit::event::WindowEvent::CursorMoved { device_id, position} => Self::CursorMoved { device_id, position },
			winit::event::WindowEvent::CursorEntered { device_id } => Self::CursorEntered { device_id },
			winit::event::WindowEvent::CursorLeft { device_id } => Self::CursorLeft { device_id },
			winit::event::WindowEvent::MouseWheel { device_id, delta, phase } => Self::MouseWheel { device_id, delta, phase },
			winit::event::WindowEvent::MouseInput { device_id, state, button } if state.is_pressed() => Self::MouseDown { device_id, state, button },
			winit::event::WindowEvent::MouseInput { device_id, state, button } => Self::MouseUp { device_id, state, button },
			winit::event::WindowEvent::AxisMotion { device_id, axis, value } => Self::AxisMotion { device_id, axis, value },
			winit::event::WindowEvent::Touch(touch) => Self::Touch(touch),
			winit::event::WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer } => Self::ScaleFactorChanged { scale_factor, inner_size_writer },
			winit::event::WindowEvent::Occluded(occlude) => Self::Occluded(occlude),
			_ => Self::Unknown
		}
	}
}
///A set of events returned by the user event handler for communication between threads
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum UserEvent<RenderData> {
	///Raised when the user event handler have finished its clean up
	ReadyToClose,
	RenderDataPrepared(RenderData)
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
	pub push_constants: &'a [PushConstantRange],
	///What to render behind everything
	pub skybox: wgpu::Color
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
			push_constants: &[],
			skybox: wgpu::Color {
				a: 1.0,
				r: 0.5,
				g: 0.5,
				b: 0.5
			}
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
