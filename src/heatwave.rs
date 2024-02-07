//! A cross platform rendering engine designed for simplicity of use with maximum freedom of control.
//! Uses [wgpu](https://docs.rs/wgpu/latest/wgpu/) to communicate with the GPU. Add it as a dependency to use more advanced, or non rendering related features.
//!
//! To start using the engine, create a [`HeatwaveApp`] 
//! 
//! Though this project uses wgpu in its backend, it doesn't currently fully support wasm.

use std::collections::HashMap;

use gpu::GpuConnection;
use wgpu::Features;
use winit::window::Window;

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

	window: Window
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

	pub fn new(config: HeatwaveConfig) -> Self {
		
	}
}

pub struct HeatwaveConfig {
	pub power_pref: wgpu::PowerPreference,
	pub name: String,
	pub features: wgpu::Features
}
impl HeatwaveConfig {
	pub fn name(&self) -> String {
		self.name
	}
	pub fn power_preference(&self) -> wgpu::PowerPreference {
		self.power_pref
	}
	pub fn gpu_features(&self) -> wgpu::Features {
		self.features
	}
}
impl Default for HeatwaveConfig {
    fn default() -> Self {
        Self { 
			power_pref: wgpu::PowerPreference::HighPerformance, //Default to the best GPU it can find, as this is the most common one to ask for
			name: String::from("Heatwave App"),
			features: Features::empty()
		}
    }
}

