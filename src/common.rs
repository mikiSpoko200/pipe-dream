#![allow(unused)]

use std::cell::{Cell, LazyCell};
use std::io::Write;
use std::num::NonZeroU32;
use std::time::{self, Duration};

use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::{context, surface};

use glutin_winit::{DisplayBuilder, GlWindow};
use imgui::{Condition, Key};
use raw_window_handle::HasWindowHandle as _;
use to_trait::To;
use winit::application::ApplicationHandler;
use winit::event::{self, DeviceEvent, ElementState, KeyEvent, RawKeyEvent, WindowEvent};
use winit::event_loop::{self, ActiveEventLoop, EventLoop};
use winit::keyboard::{self, KeyCode, PhysicalKey};
use winit::window::{self, CursorGrabMode};

use gpu_bulwark as gb;


/// Wrapper around boolean 
#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum BaseToggle {
    #[default]
    Off = 0,
    On = 1,
}

impl BaseToggle {
    pub const fn flip(&mut self) -> Self {
        match self {
            BaseToggle::Off => *self = Self::On,
            BaseToggle::On => *self = Self::Off,
        };
        *self
    }

    pub fn set(&mut self, state: bool) {
        match state {
            true => *self = Self::On,
            false => *self = Self::Off,
        }
    }

    pub const fn clear(&mut self) {
        *self = Self::Off;
    }

    /// Clear toggle to [Off] state, return true if it was [On].
    /// 
    /// TODO: rename this
    pub const fn impulse(&mut self) -> bool {
        let was_on = self.is_on();
        self.clear();
        was_on
    }

    pub const fn is_on(&self) -> bool {
        matches! { self, Self::On }
    }

    pub const fn is_off(&self) -> bool {
        matches! { self, Self::On }
    }

    pub fn map_on<T>(&self, f: impl FnOnce() -> T) -> Option<T> {
        (matches! { self, Self::On }).then(f)
    }

    pub fn map_off<T>(&self, f: impl FnOnce() -> T) -> Option<T> {
        (matches! { self, Self::Off }).then(f)
    }
}

pub trait Toggle: AsRef<BaseToggle> {
    type Mut<T>;

    /// Provide a mutable reference to a toggle.
    fn as_mut(&mut self) -> Self::Mut<&mut BaseToggle>;

    fn wrapper(&mut self) -> &mut Self {
        self
    }
}

impl AsRef<BaseToggle> for BaseToggle {
    fn as_ref(&self) -> &BaseToggle {
        self
    }
}   

impl Toggle for BaseToggle {
    type Mut<T> = T;

    fn as_mut(&mut self) -> Self::Mut<&mut BaseToggle> {
        self
    }
    
    fn wrapper(&mut self) -> &mut Self {
        self
    }
}

pub mod toggle {
    #[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
    pub struct Snooze {
        snoozed: bool,
        toggle: super::BaseToggle,
    }

    impl AsRef<super::BaseToggle> for Snooze {
        fn as_ref(&self) -> &super::BaseToggle {
            &self.toggle
        }
    }

    impl super::Toggle for Snooze {
        type Mut<T> = Option<T>;

        fn as_mut(&mut self) -> Self::Mut<&mut super::BaseToggle> {
            (!self.snoozed).then_some(&mut self.toggle)
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct KeyBoard<T: Toggle = BaseToggle> {
    pub key_w: T,
    pub key_s: T,
    pub key_a: T,
    pub key_d: T,
    pub key_q: T,
    pub key_e: T,
    pub space: T,
    pub left_shift: T,
    pub tab: T,
}

impl<T: Toggle> KeyBoard<T> {
    const N_TOGGLES: usize = std::mem::size_of::<KeyBoard>() / std::mem::size_of::<T>();
}

impl<T: Toggle> KeyBoard<T> {
    pub fn as_ref(&self, key: KeyCode) -> Option<&T> {
        match key {
            KeyCode::KeyW => Some(&self.key_w),
            KeyCode::KeyS => Some(&self.key_s),
            KeyCode::KeyA => Some(&self.key_a),
            KeyCode::KeyD => Some(&self.key_d),
            KeyCode::KeyE => Some(&self.key_e),
            KeyCode::KeyQ => Some(&self.key_q),
            _ => None,
        }
    }

    pub fn as_mut(&mut self, key: KeyCode) -> Option<&mut T> {
        match key {
            KeyCode::KeyW => Some(&mut self.key_w),
            KeyCode::KeyS => Some(&mut self.key_s),
            KeyCode::KeyA => Some(&mut self.key_a),
            KeyCode::KeyD => Some(&mut self.key_d),
            KeyCode::KeyE => Some(&mut self.key_e),
            KeyCode::KeyQ => Some(&mut self.key_q),
            KeyCode::Space => Some(&mut self.space),
            KeyCode::ShiftLeft => Some(&mut self.left_shift),
            _ => None,
        }
    }
}

pub mod config {
    use std::path::PathBuf;

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

pub mod camera {
    use super::config::{HEIGHT, WIDTH};
    use glm::{Mat4, Vec3};
    use crate::physics::{KineticState, Oriented};
    use crate::{gb, physics};

    #[derive(Debug, Copy, Clone)]
    pub struct Directions {
        pub up: glm::Vec3,
        pub down: glm::Vec3,
        pub front: glm::Vec3,
        pub back: glm::Vec3,
        pub left: glm::Vec3,
        pub right: glm::Vec3,
    }

    impl Directions {
        pub const FRONT: glm::Vec3 = glm::Vec3::new(0f32, 0f32, -1f32);
        pub const BACK: glm::Vec3 = glm::Vec3::new(0f32, 0f32, 1f32);
        pub const UP: glm::Vec3 = glm::Vec3::new(0f32, 1f32, 0f32);
        pub const DOWN: glm::Vec3 = glm::Vec3::new(0f32, -1f32, 0f32);
        pub const RIGHT: glm::Vec3 = glm::Vec3::new(1f32, 0f32, 0f32);
        pub const LEFT: glm::Vec3 = glm::Vec3::new(-1f32, 0f32, 0f32);
    }

    pub enum Direction {
        Front,
        Back,
        Up,
        Down,
        Left,
        Right,
    }

    struct RightHandCoordSys {
        front: Vec3,
    }

    impl RightHandCoordSys {
        const GLOBAL_UP: Vec3 = Directions::UP;

        pub fn new(front: Vec3) -> Self {
            Self { front }
        }

        pub fn direction(&self, direction: &Direction) -> Vec3 {
            use std::borrow::Borrow;

            let left = self.front.cross(&Self::GLOBAL_UP);
            let up = glm::rotate_vec3(&self.front, 90.0f32.to_radians(), left.borrow());
            match direction {
                Direction::Front => self.front,
                Direction::Back => -self.front,
                Direction::Up => up,
                Direction::Down => -up,
                Direction::Left => -left,
                Direction::Right => left,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct Orientation {
        pub quaterion: glm::Quat,
    }

    impl From<glm::Quat> for Orientation {
        fn from(orientation: glm::Quat) -> Self {
            Self { quaterion: orientation }
        }
    }

    impl Orientation {
        pub fn from_direction_and_up(direction: Vec3, up: Vec3) -> Self {
            // Normalize the forward vector (the viewing direction)
            let matrix = glm::look_at(&Vec3::default(), &direction, &glm::Vec3::y());
        
            Self { quaterion: glm::mat3_to_quat(&glm::mat4_to_mat3(&matrix)) }
        }
        
        pub fn matrix(&self, position: &Vec3) -> glm::Mat4 {
            glm::quat_to_mat4(&self.quaterion).transpose()
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Projection {
        Orthographic {
            width: f32,
            height: f32,
            near: f32,
            far: f32,
        },
        Perspective {
            near: f32,
            far: f32,
            aspect_ratio: f32,
            fovy: f32,
        },
    }

    impl Projection {
        pub const fn orthographic(near: f32, far: f32, width: f32, height: f32) -> Self {
            Self::Orthographic {
                near,
                far,
                width,
                height,
            }
        }

        pub const fn perspective(near: f32, far: f32, aspect_ratio: f32, fovy: f32) -> Self {
            Self::Perspective {
                near,
                far,
                aspect_ratio,
                fovy,
            }
        }

        pub fn matrix(&self) -> glm::Mat4 {
            match self {
                &Projection::Orthographic {
                    near,
                    far,
                    width,
                    height,
                } => glm::ortho(0.0, width, 0.0, height, near, far),
                &Projection::Perspective {
                    near,
                    far,
                    aspect_ratio,
                    fovy,
                } => glm::perspective(aspect_ratio, fovy.to_radians(), near, far),
            }
        }
    }

    pub struct Camera {
        orientation: Orientation,
        projection: Projection,
        kinetic: KineticState,
    }

    impl physics::Kinetic for Camera {
        fn kinetic_state(&mut self) -> &mut KineticState {
            &mut self.kinetic
        }
    }

    impl Oriented for Camera {
        fn orientation(&mut self) -> &mut Orientation {
            &mut self.orientation
        }
    }

    static_assertions::assert_impl_all!(Camera: physics::Kinetic);

    impl Camera {
        const DEFAULT_FOVY: f32 = 60.0;
        const DEFAULT_Z_NEAR: f32 = 0.1;
        const DEFAULT_Z_FAR: f32 = 150.0;

        const SENSITIVITY: f32 = 0.5;

        pub fn stationary(view: Orientation, projection: Projection, at: glm::Vec3) -> Self {
            Self::moving(view, projection, KineticState::stationary(at))
        }

        pub fn moving(view: Orientation, projection: Projection, kinetic: KineticState) -> Self {
            Self {
                orientation: view,
                projection,
                kinetic,
            }
        }

        pub fn view_matrix(&self) -> glm::Mat4 {
            self.orientation.matrix(&self.kinetic.position) * glm::translation(&-self.kinetic.position)
        }

        pub fn projection_matrix(&self) -> glm::Mat4 {
            self.projection.matrix()
        }

        pub fn view_projection_matrix(&self) -> glm::Mat4 {
            self.projection_matrix() * self.view_matrix()
        }
    }

    pub trait Rotatable {
        fn rotate(&mut self, x_angle: f32, y_angle: f32);
    }

    pub struct DynMotion<T>
    where
        T: physics::Kinetic,
    {
        moveable: T,
        effects: std::rc::Rc<dyn Fn(&mut KineticState)>,
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

pub trait Sample: Sized {
    fn initialize() -> anyhow::Result<Self>;

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

pub trait InteractiveSample: Sample {
    const FREQUENCY: usize;
    type DCtx;

    fn update(&mut self, dctx: &Self::DCtx, dt: Duration);
}

pub struct ImGui {
    pub ctx: imgui::Context,
    pub platform: imgui_winit_support::WinitPlatform,
    pub renderer: imgui_opengl_renderer::Renderer,
}

pub struct Snoozer {
    map: anymap::AnyMap,
}

impl Snoozer {
    pub fn register<T: 'static>(&mut self, value: T) {
        self.map.insert(value);
    }
}

pub struct Systems {
    snoozer: Snoozer,
}

pub struct State<T> {
    pub window: window::Window,
    pub surface: surface::Surface<surface::WindowSurface>,
    pub imgui: ImGui,
    pub context: context::PossiblyCurrentContext,
    pub inner: T,
}

impl<T> AsRef<T> for State<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsMut<T> for State<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

struct App<T: Sample> {
    frame_timer: Timer,
    state: Option<State<T>>,
}

impl<T: Sample> Default for App<T> {
    fn default() -> Self {
        Self {
            frame_timer: Timer::new(),
            state: None,
        }
    }
}

impl<T: Sample> App<T> {
    #[tracing::instrument(skip_all)]
    fn init(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.state.is_none() {
            event_loop.set_control_flow(event_loop::ControlFlow::Poll);

            let config = T::config();

            // let icon = hello_textures::logo::uwr();
            // let icon: &[u8] = unsafe { std::slice::from_raw_parts(icon.as_ptr() as *const _, icon.len() * 4) };

            // let icon = winit::window::Icon::from_rgba(Vec::from_iter(icon.into_iter().map(Clone::clone)), 256, 256).unwrap();

            let window_attributes = winit::window::WindowAttributes::default()
                // .with_window_icon(Some(icon))
                .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height))
                .with_title(T::name())
                .with_resizable(false)
                .with_fullscreen(config.fullscrreen.then(|| winit::window::Fullscreen::Borderless(None)));

            // Glutin gl context initialization
            let version = context::Version::new(4, 6);
            tracing::info!(
                "initializing OpenGL {}.{} core",
                version.major, version.minor
            );
            let template = glutin::config::ConfigTemplateBuilder::new();
            let display_builder =
                DisplayBuilder::new().with_window_attributes(Some(window_attributes));

            let config_selector = |configs: Box<
                dyn Iterator<Item = glutin::config::Config> + '_,
            >| {
                configs
                    .reduce(|accum, config| {
                        let transparency_check = config.supports_transparency().unwrap_or(false)
                            & !accum.supports_transparency().unwrap_or(false);

                        if transparency_check || config.num_samples() > accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .expect("at least one configuration is compatible with given template")
            };

            let (mut window, config) = display_builder
                .build(event_loop, template, config_selector)
                .expect("can create display");

            let raw_window_handle = window
                .as_ref()
                .and_then(|window| window.window_handle().map(|handle| handle.as_raw()).ok());

            let window = window.take().unwrap();

            window.set_cursor_grab(CursorGrabMode::Confined).ok();
            window.set_cursor_visible(false);

            let display = config.display();

            let context_attributes =
                context::ContextAttributesBuilder::new().build(raw_window_handle);

            let not_current_gl_context = unsafe {
                display
                    .create_context(&config, &context_attributes)
                    .expect("failed to create context")
            };

            
            let surface = {
                let attrs = window
                    .build_surface_attributes(surface::SurfaceAttributesBuilder::default())
                    .expect("Failed to build surface attributes");
                
                unsafe {
                    config
                        .display()
                        .create_window_surface(&config, &attrs)
                        .unwrap()
                }
            };

            
            let gl_context = not_current_gl_context
                .make_current(&surface)
                .expect("can make surface current");
            
            surface.set_swap_interval(&gl_context, surface::SwapInterval::DontWait);

            let loader = |symbol| {
                let symbol = std::ffi::CString::new(symbol).unwrap();
                display.get_proc_address(symbol.as_c_str()).cast()
            };

            gb::load_with(&loader);

            let imgui = {
                let mut ctx = imgui::Context::create();

                let mut platform = imgui_winit_support::WinitPlatform::new(&mut ctx);
                platform.attach_window(ctx.io_mut(), &window, imgui_winit_support::HiDpiMode::Default);

                let renderer = imgui_opengl_renderer::Renderer::new(&mut ctx, loader);

                ImGui {
                    platform,
                    renderer,
                    ctx,
                }
            };

            self.state = Some(match T::initialize() {
                Ok(inner) => State { inner, window, surface, imgui, context: gl_context },
                Err(err) => panic!("{err}"),
            });

            tracing::info!("press escape key to exit");
            tracing::info!("{}", self.state.as_ref().unwrap().inner.usage());
        }
    }

    fn render(&mut self) {
        self.state.as_mut().map(|state| {
            state.imgui.platform
                .prepare_frame(state.imgui.ctx.io_mut(), &state.window)
                .expect("can prepare frame");

            state.inner.render();
            
            let ui = state.imgui.ctx.new_frame();

            // testbed diagnostics
            ui.window("Global")
                .position([0.0, 0.0], Condition::Appearing)
                .size([300.0, 100.0], Condition::Appearing)
                .build(|| {
                    ui.text(format!("fps: {:.2}", 1.0 / self.frame_timer.get().as_secs_f32()));
                    ui.text(format!("frame time: {:.2} ms", self.frame_timer.get().as_millis()));
                    ui.text(format!("window size: {}x{}", state.window.inner_size().width, state.window.inner_size().height));
                    ui.text(format!("sample name: {}", T::name()));
                });

            // HOLY_SHIT: not rendering to the ui causes rust UB XD
            state.inner.ui(ui); // step 4

            state.imgui.platform.prepare_render(ui, &state.window); // step 5
            state.imgui.renderer.render(&mut state.imgui.ctx);

            state.surface
                .swap_buffers(&state.context)
                .expect("buffer swapping is successful");
        });
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
            .map(|sample| sample.on_mouse_movement(delta));
    }
}

impl<T: Sample> ApplicationHandler for App<T> {
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
                self.state.as_mut().map(|state| state.inner.update(dt));
            },
            event::StartCause::Init => self.init(event_loop),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: window::WindowId, event: WindowEvent) {
        let Some(ref mut state) = self.state else {
            return
        };
        
        state.imgui.platform.handle_event(&mut state.imgui.ctx.io_mut(), &state.window, &event::Event::<()>::WindowEvent { window_id, event: event.clone() });

        match event {
            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    state.surface.resize(
                        &state.context,
                        NonZeroU32::new(size.width).unwrap(),
                        NonZeroU32::new(size.height).unwrap(),
                    );
                    state.window.request_redraw();
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                state.imgui.ctx.io_mut().update_delta_time(self.frame_timer.get());
                
                state.window.request_redraw();
                self.render();
            }
            _ => (),
        }
    }
}

pub fn run_sample<S: Sample>() -> anyhow::Result<()> {
    let mut app = App::<S>::default();
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
