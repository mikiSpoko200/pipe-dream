pub mod common;
pub mod physics;

use common::KeyBoard;
use common::camera::{Camera, Directions, Orientation, Projection};
use common::config::{model_path, shader_path};
use imgui::Condition;

use std::time::Duration;

use gb::{gl, glsl};
use gl::buffer::{Draw, Static};
use gl::shader;
use gl::vertex_array::Attribute;
use gl::{Buffer, Program, VertexArray};
use glsl::MatchingInputs as _;
use gpu_bulwark as gb;
use winit::event::ElementState;

use crate::common::toggle::Snooze;
use crate::common::Toggle;
use crate::physics::motion::Model as _;
use crate::physics::{Kinetic, Oriented};

type Inputs = glsl::Inputs! {
    layout(location = 0) vec3;
    layout(location = 1) vec3;
};

type VsOutputs = glsl::Outputs! {
    layout(location = 0) vec4;
};

type FsOutputs = glsl::Outputs! {
    layout(location = 0) vec4;
};

type NormalShaderUniforms = glsl::Uniforms! {
    layout(location = 0) mat4;
    layout(location = 1) vec3;
};

type DepthShaderUniforms = glsl::Uniforms! {
    layout(location = 0) mat4;
};

type Attributes = gb::HList! {
    Attribute<glm::Vec3, 0>,
    Attribute<glm::Vec3, 1>,
};

pub struct ResumableIterator<'a, T: Iterator>(std::iter::Skip<&'a mut T>);

impl<'a, T: Iterator> ResumableIterator<'a, T> {
    pub fn new(iter: &'a mut T, index: usize) -> Self {
        ResumableIterator(iter.skip(index))
    }
}

impl<T> Iterator for ResumableIterator<'_, T>
where
    T: Iterator,
{
    type Item = T::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub struct Programs {
    light_directions: Program<Inputs, FsOutputs, NormalShaderUniforms, ()>,
    depth: Program<Inputs, FsOutputs, DepthShaderUniforms, ()>,
    current_program: usize,
}

impl Programs {
    pub fn new(camera: &Camera, global_light_dir: glm::Vec3) -> anyhow::Result<Self> {
        let vs_inputs = Inputs::default();
        let vs_outputs = VsOutputs::default();

        let fs_inputs = vs_outputs.matching_inputs();
        let glsl::vars![fs_output] = FsOutputs::default();

        let glsl::vars![view_matrix_location, global_light_direction_location] =
            NormalShaderUniforms::default();

        fn load<T: shader::target::Target>(shader: &'static str) -> anyhow::Result<shader::Shader<gb::ts::Uncompiled, T, ()>> {
            let source = std::fs::read_to_string(shader_path(shader))?;
            let mut uncompiled_shader = shader::create::<T>();
            uncompiled_shader.source(&[&source]);
            Ok(uncompiled_shader)
        }

        // TODO: load shaders as assets
        let vs = load::<shader::target::Vertex>("bunny.vert")?
            .uniform(&view_matrix_location)
            .compile()?
            .into_main()
            .inputs(&vs_inputs)
            .outputs(&vs_outputs);
        let normal_fs = load::<shader::target::Fragment>("bunny.frag")?
            .uniform(&global_light_direction_location)
            .compile()?
            .into_main()
            .inputs(&fs_inputs)
            .output(&fs_output);
        let depth_fs = load::<shader::target::Fragment>("depth.frag")?
            .compile()?
            .into_main()
            .inputs(&fs_inputs)
            .output(&fs_output);

        let matrix = camera.view_projection_matrix();

        let light_directions = Program::builder()
            .uniforms(|definitions| {
                definitions
                    .define(&view_matrix_location, &matrix)
                    .define(&global_light_direction_location, &global_light_dir)
            })
            .no_resources()
            .vertex_main(&vs)
            .uniforms(|matcher| {
                matcher
                    .bind(&view_matrix_location)
            })
            .fragment_main(&normal_fs)
            .uniforms(|matcher| {
                matcher.bind(&global_light_direction_location)
            })
            .build()?;
    
        
        let depth = Program::builder()
            .uniforms(|definitions| {
                definitions
                    .define(&view_matrix_location, &matrix)
            })
            .no_resources()
            .vertex_main(&vs)
            .uniforms(|matcher| {
                matcher
                    .bind(&view_matrix_location)
            })
            .fragment_main(&depth_fs)
            .build()?;

        Ok(Self {
            light_directions,
            depth,
            current_program: 0,
        })
    }

    pub fn current(&mut self) -> CurrentProgram<'_> {
        if self.current_program == 0 {
            CurrentProgram::LightDirections(&mut self.light_directions)
        } else {
            CurrentProgram::Depth(&mut self.depth)
        }
    }

    pub fn toggle(&mut self) {
        self.current_program ^= 1;
    }
}

pub enum CurrentProgram<'a> {
    LightDirections(&'a mut Program<Inputs, FsOutputs, NormalShaderUniforms, ()>),
    Depth(&'a mut Program<Inputs, FsOutputs, DepthShaderUniforms, ()>),
}

pub struct Sample {
    programs: Programs,
    vao: VertexArray<Attributes, u16>,
    global_light_dir: glm::Vec3,
    camera: physics::motion::ExponentialDecay<Camera>,
    keyboard: KeyBoard<Snooze>,
}

struct Model {
    pub positions: Vec<glm::Vec3>,
    pub normals: Vec<glm::Vec3>,
    pub index: Vec<u16>,
}

impl Sample {
    fn load_model() -> Model {
        use obj::load_obj;
        use std::fs::File;
        use std::io::BufReader;

        let input = BufReader::new(File::open(model_path("bunny.obj")).expect("model file exists"));
        let obj = load_obj::<obj::Vertex, _, _>(input).expect("model can be loaded");
        let (positions, normals) = obj
            .vertices
            .iter()
            .map(|vertex| {
                (
                    glm::Vec3::from(vertex.position),
                    glm::Vec3::from(vertex.normal),
                )
            })
            .unzip();
        Model {
            positions,
            normals,
            index: obj.indices,
        }
    }
}

impl common::Sample for Sample {
    fn initialize() -> anyhow::Result<Self> {
        let glsl::vars![vin_position, vin_color] = Inputs::default();

        gl::call! {
            #[panic]
            unsafe {
                gl::raw::ClearColor(0.4, 0.5, 0.6, 1.0);
                gl::raw::Enable(gl::raw::DEPTH_TEST);
            }
        }

        let model = Self::load_model();

        let mut positions = Buffer::create();
        positions.data::<(Static, Draw)>(&model.positions);

        let mut colors = Buffer::create();
        colors.data::<(Static, Draw)>(&model.normals);

        let mut index = Buffer::create();
        index.data::<(Static, Draw)>(&model.index);


        let vao = VertexArray::create()
            .vertex_attrib_pointer(&vin_position, positions)
            .vertex_attrib_pointer(&vin_color, colors)
            .element_buffer(index);

        let global_light_dir = glm::vec3(-1f32, -1f32, -1f32);
        let camera = {
            let view = Orientation::from_direction_and_up(Directions::BACK, Directions::UP);
            let projection = Projection::perspective(0.01, 100.0, 16.0 / 9.0, 60.0);

            Camera::stationary(view, projection, glm::Vec3::zeros())
        };

        let programs = Programs::new(&camera, global_light_dir)?;

        let inner = Self {
            programs,
            vao,
            global_light_dir,
            camera: physics::motion::ExponentialDecay {
                decay_rate: 0.9,
                kinetic: camera,
            },
            keyboard: KeyBoard::default(),
        };

        Ok(inner)
    }

    fn ui(&self, ui: &mut imgui::Ui) {
        ui.window("Hello world")
            .size([300.0, 100.0], Condition::Appearing)
            .build(|| {
                ui.text("Hello world!");
                ui.text("This is a simple testbed for the physics engine.");
                ui.text("Use W, A, S, D, space, l-shift keys to move around and mouse to operate the camera");
            });
    }

    fn render(&mut self) {
        gl::call! {
            #[panic]
            unsafe {
                gl::raw::Clear(gl::raw::COLOR_BUFFER_BIT | gl::raw::DEPTH_BUFFER_BIT);
            }
        }
        match self.programs.current() {
            CurrentProgram::LightDirections(program) => {
                program.draw_elements(&self.vao);
            }
            CurrentProgram::Depth(program) => {
                program.draw_elements(&self.vao);
            }
        }
    }

    fn on_key(&mut self, code: winit::keyboard::KeyCode, state: ElementState) {
        if matches!(code, winit::keyboard::KeyCode::Tab) {
            self.programs.toggle();
            return;
        }

        self.keyboard
            .as_mut(code)
            .map(Toggle::as_mut)
            .flatten()
            .map(|key| key.set(state == ElementState::Pressed))
            .unwrap_or_else(|| tracing::debug!("unsupported key code {:?}", code));
    }

    fn on_mouse_movement(&mut self, (dx, dy): (f64, f64)) {
        physics::rotate(
            &mut self.camera.kinetic,
            (-dy as f32).to_radians() * 3.0,
            (-dx as f32).to_radians() * 3.0,
        );
    }

    fn update(&mut self, dt: Duration) {
        let glsl::vars![matrix, light] = NormalShaderUniforms::default();

        let input_force = physics::compute_input_force(&self.keyboard, self.camera.kinetic.orientation(), 1.0); // physics::steer(&self.keyboard, &mut self.camera.kinetic, dt);

        let mass = 1.0;
        let acceleration = input_force / mass;

        self.camera.kinetic.kinetic_state().apply_acceleration(acceleration, dt.as_secs_f32());
        self.camera.update(dt.as_secs_f32());

        match self.programs.current() {
            CurrentProgram::LightDirections(program) => program.uniform(&matrix, &self.camera.kinetic.view_projection_matrix()),
            CurrentProgram::Depth(program) => program.uniform(&matrix, &self.camera.kinetic.view_projection_matrix()),
        }
        self.global_light_dir =
            (glm::rotate(&glm::Mat4::identity(), self.camera.kinetic.kinetic_state().velocity.norm() / 10.0, &glm::Vec3::y_axis())
                * glm::vec3_to_vec4(&self.global_light_dir))
            .xyz();
        if let CurrentProgram::LightDirections(program) = self.programs.current() {
            program.uniform(&light, &self.global_light_dir)
        }
    }

    fn usage(&self) -> String {
        // TODO: render this as tooltip in the ui
        String::from(
            "use W, A, S, D, space, l-shift keys to move around and mouse to operate the camera",
        )
    }

    fn name() -> String {
        String::from("testbed")
    }

    fn config() -> common::config::Config {
        common::config::Config {
            fullscrreen: true,
            width: 1920,
            height: 1080,
        }
    }
}

impl common::InteractiveSample for Sample {
    const FREQUENCY: usize = 120;
    type DCtx = common::KeyBoard;

    fn update(&mut self, _: &KeyBoard, _: std::time::Duration) {}
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    common::run_sample::<Sample>()?;
    Ok(())
}
