
    use glm::{Vec3};
    use crate::physics::{self, KineticState, Oriented};

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