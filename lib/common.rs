#![allow(unused)]

use std::cell::{Cell, LazyCell};
use std::io::Write;
use std::num::NonZeroU32;
use std::time::{self, Duration};

use glutin::display::GetGlDisplay as _;
use glutin::prelude::*;
use glutin::{context, surface};

use imgui::{Condition, Key};
use raw_window_handle::HasWindowHandle as _;
use to_trait::To;
use winit::application::ApplicationHandler;
use winit::event::{self, DeviceEvent, ElementState, KeyEvent, RawKeyEvent, WindowEvent};
use winit::event_loop::{self, ActiveEventLoop, EventLoop};
use winit::keyboard::{self, KeyCode, PhysicalKey};
use winit::window::{self, CursorGrabMode};

pub mod config {
    use std::path::PathBuf;

    #[derive(Debug)]
    pub struct Config {
        pub fullscrreen: bool,
        pub width: u32,
        pub height: u32,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                width: WIDTH,
                height: HEIGHT,
                fullscrreen: false,
            }
        }
    }

    pub const WIDTH: u32 = 960;
    pub const HEIGHT: u32 = 640;
    pub const MOUSE_SENSITIVITY: f32 = 0.0005;
    pub const MOVEMENT_SPEED: f32 = 0.1;

    const RESOURCE_PATH: &'static str = "resources";
    const SHADER_PATH: &'static str = "shaders";
    const MODEL_PATH: &'static str = "models";

    pub fn resource_path(resource: &'static str) -> PathBuf {
        format!("{}/{}", RESOURCE_PATH, resource).into()
    }

    pub fn shader_path(shader: &'static str) -> PathBuf {
        format!("{}/{}", SHADER_PATH, shader).into()
    }

    pub fn model_path(model: &'static str) -> PathBuf {
        format!("{}/{}", MODEL_PATH, model).into()
    }
}

pub struct Timer {
    prev_frame_at: time::Instant,
    prev_frame_time: time::Duration,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            prev_frame_at: time::Instant::now(),
            prev_frame_time: time::Duration::ZERO,
        }
    }

    pub fn get(&self) -> time::Duration {
        std::time::Instant::now() - self.prev_frame_at
    }

    pub fn update(&mut self) -> time::Duration {
        self.prev_frame_time = self.prev_frame_at.elapsed();
        self.prev_frame_at = time::Instant::now();
        self.prev_frame_time
    }
}

pub trait Logic: Sized {
    fn initialize(platform: &Platform) -> anyhow::Result<Self>;

    fn configure_event_loop(event_loop: &winit::event_loop::ActiveEventLoop);

    fn configure_platform(platform: &Platform);

    fn render(&mut self);

    fn on_key(&mut self, code: winit::keyboard::KeyCode, state: winit::event::ElementState);

    fn on_mouse_movement(&mut self, delta: (f64, f64));

    fn update(&mut self, dt: Duration) {}

    fn ui(&self, ui: &mut imgui::Ui);

    fn name() -> String;


    fn usage(&self) -> String;
    fn config() -> config::Config {
        config::Config::default()
    }
}

pub trait InteractiveSample: Logic {
    const FREQUENCY: usize;
    type DCtx;

    fn update(&mut self, dctx: &Self::DCtx, dt: Duration);
}

pub trait Backend: Sized {
    type FrameCtx;

    fn initalize(platform: &Platform) -> anyhow::Result<Self>;

    fn pre(&mut self) -> Self::FrameCtx;
    
    fn post(&mut self);
}

pub struct Platform {
    pub window: window::Window,
}

pub struct AppData<Logic, Backend> {
    pub platform: Platform,
    pub backend: Backend,
    pub logic: Logic,
}

impl<T, B> AsRef<T> for AppData<T, B> {
    fn as_ref(&self) -> &T {
        &self.logic
    }
}

impl<T, B> AsMut<T> for AppData<T, B> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.logic
    }
}

pub type AfterStartup<T> = Option<T>;

struct Harness<Logic, Backend> {
    frame_timer: Timer,
    state: AfterStartup<AppData<Logic, Backend>>,
}

impl<T: Logic, B> Default for Harness<T, B> {
    fn default() -> Self {
        Self {
            frame_timer: Timer::new(),
            state: None,
        }
    }
}

impl<L: Logic, B: Backend> Harness<L, B> {
    // #[tracing::instrument(skip_all)]
    fn init(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) -> anyhow::Result<()> {
        if self.state.is_none() {

            L::configure_event_loop(event_loop);

            let platform = {
                let config = L::config();
                
                let window_attributes = winit::window::WindowAttributes::default()
                    .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height))
                    .with_title(L::name())
                    .with_resizable(true);
                    // .with_fullscreen(dbg!(config.fullscrreen.then(|| winit::window::Fullscreen::Borderless(None))));

                let window = event_loop.create_window(window_attributes)?;

                
                Platform {
                    window,
                }
            };
            
            L::configure_platform(&platform);

            let backend = B::initalize(&platform)?;
            let logic = L::initialize(&platform)?;

            self.state = Some(AppData { platform, backend, logic });

            tracing::info!("press escape key to exit");
            tracing::info!("{}", self.state.as_ref().unwrap().logic.usage());
            
            Ok(())
        } else {
            Ok(())
        }
    }

    fn render(&mut self) {
        if let Some(state) = &mut self.state {
            state.backend.pre();

            state.logic.render();
            
            state.backend.post();
        }
    }

    fn process_key(&mut self, key: keyboard::KeyCode, state: ElementState) {
        if key == KeyCode::Escape {
            std::process::exit(0)
        }
        self.state
            .as_mut()
            .map(AsMut::as_mut)
            .map(|sample| sample.on_key(key, state));
    }

    fn process_mouse_input(&mut self, delta: (f64, f64)) {
        self.state
            .as_mut()
            .map(AsMut::as_mut)
            .map(|logic| logic.on_mouse_movement(delta));
    }
}

impl<L: Logic, B: Backend> ApplicationHandler for Harness<L, B> {
    fn resumed(&mut self, event_loop: &event_loop::ActiveEventLoop) {}

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _: event::DeviceId,
        event: event::DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { delta } => self.process_mouse_input(delta),
            DeviceEvent::Key(RawKeyEvent {
                physical_key: PhysicalKey::Code(key),
                state,
            }) => self.process_key(key, state),
            _ => (),
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: event::StartCause) {
        match cause {
            event::StartCause::ResumeTimeReached { start, requested_resume } => println!("wait time reached"),
            event::StartCause::WaitCancelled { start, requested_resume } => panic!("we dont use wait here!"),
            event::StartCause::Poll => {
                let dt = self.frame_timer.update();
                self.state.as_mut().map(|state| state.logic.update(dt));
            },
            event::StartCause::Init => match self.init(event_loop) {
                Ok(_initialized) => (),
                Err(err) => panic!("{}", err),
            },
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: window::WindowId, event: WindowEvent) {
        let Some(ref mut state) = self.state else {
            return
        };
        
        match event {
            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    // TODO: resize the backend
                    state.platform.window.request_redraw();
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // TODO: is this necessary here? Doesn't render trigger redraw automatically?
                state.platform.window.request_redraw();
                
                self.render();
            }
            _ => (),
        }
    }
}

pub fn run_sample<S: Logic, B: Backend>() -> anyhow::Result<()> {
    let mut app = Harness::<S, B>::default();
    let event_loop = EventLoop::new()?;

    Ok(event_loop.run_app(&mut app)?)
}

/// Visitor that provides extra context `T`.
pub trait Visitor<T> {
    fn visit(&mut self, value: &T) -> anyhow::Result<()>;

    fn visit_mut(&mut self, value: &mut T) -> anyhow::Result<()>;
}

pub trait Visit<T> {
    fn accept<V: Visitor<T>>(&mut self, visitor: &mut V) -> anyhow::Result<()>;
}
