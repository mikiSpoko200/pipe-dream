use crate::common::{camera::Orientation, Toggle};

use super::KeyBoard;

#[derive(Debug, Clone)]
pub struct KineticState {
    pub velocity: glm::Vec3,
    pub position: glm::Vec3,
}

impl KineticState {
    pub fn stationary(position: glm::Vec3) -> Self {
        Self {
            position,
            velocity: glm::Vec3::default()
        }
    }

    pub fn apply_acceleration(&mut self, acceleration: glm::Vec3, dt: f32) {
        self.position += self.velocity * dt + 0.5 * acceleration * dt * dt;
        self.velocity += acceleration * dt;
    }
}

pub trait Kinetic {
    fn kinetic_state(&mut self) -> &mut KineticState;

    fn velocity(&mut self) -> &mut glm::Vec3 {
        &mut self.kinetic_state().velocity
    }

    fn position(&mut self) -> &mut glm::Vec3 {
        &mut self.kinetic_state().position
    }
}

pub trait Oriented {
    fn orientation(&mut self) -> &mut Orientation;

    fn quaterion(&mut self) -> &mut glm::Quat {
        &mut self.orientation().quaterion
    }
}

pub fn rotate<O: Oriented>(oriented: &mut O, dx: f32, dy: f32) {
    let pitch_rad = dx.to_radians(); // camera-relative
    let yaw_rad = dy.to_radians();   // world-relative

    let orientation = &mut oriented.orientation().quaterion;

    // Compute local right vector (X axis) from current orientation
    let right = glm::quat_rotate_vec3(orientation, &glm::vec3(1.0, 0.0, 0.0));

    // Construct rotation quaternions
    let yaw = glm::quat_angle_axis(yaw_rad, &glm::vec3(0.0, 1.0, 0.0)); // global Y
    let pitch = glm::quat_angle_axis(pitch_rad, &right); // local X

    // Apply pitch first, then yaw
    *orientation = yaw * pitch * *orientation;
}



pub mod units {
    pub trait Unit {
        fn value(&self) -> f32;
    }

    macro_rules! unit {
        ($($ident:ident),+ $(,)?) => {
            $(
                pub struct $ident(f32);

                impl Unit for $ident {
                    #[inline(always)]
                    fn value(&self) -> f32 {
                        self.0
                    }
                }
            )+
        };
    }

    macro_rules! prefix {
        ($($ident:ident => $scalar:literal),+ $(,)?) => {
            $(
                pub struct $ident<U: Unit>(U);

                impl<U: Unit> Unit for $ident<U> {
                    #[inline(always)]
                    fn value(&self) -> f32 {
                        self.0.value() * $scalar
                    }
                }
            )+
        };
    }

    unit!(Seconds, Radians);
    
    prefix! {
        Milli => 1e-3,
        Micro => 1e-6,
    }
}

pub mod motion {
    use super::*;

    pub trait Model {
        fn update(&mut self, dt: f32);
    }

    pub struct ExponentialDecay<K: Kinetic> {
        pub decay_rate: f32,
        pub kinetic: K,
    }

    impl<K: Kinetic> Model for ExponentialDecay<K> {
        fn update(&mut self, dt: f32) {
            let KineticState { velocity, position } = self.kinetic.kinetic_state();
            *velocity *= (-self.decay_rate * dt).exp();
            *position += *velocity * dt;
        }
    }

    pub struct CriticallyDampedSpring<K: Kinetic> {
        target: glm::Vec3,
        stiffness: f32,
        damping: f32,
        kinetic: K,
    }

    impl<K: Kinetic> Model for CriticallyDampedSpring<K> {
        fn update(&mut self, dt: f32) {
            let KineticState { velocity, position } = self.kinetic.kinetic_state();
            let diff = self.target - *position;
            let force = diff * self.stiffness - *velocity * self.damping;
            self.kinetic.kinetic_state().apply_acceleration(force, dt);
        }
    }

    pub struct VelocityDamping<K: Kinetic> {
        friction: f32,
        kinetic: K,
    }

    impl<K: Kinetic> Model for VelocityDamping<K> {
        fn update(&mut self, dt: f32) {
            let friction_force = *self.kinetic.velocity() * -self.friction;
            self.kinetic.kinetic_state().apply_acceleration(friction_force, dt);
        }
    }
}

pub fn compute_input_force<T: Toggle>(input: &KeyBoard<T>, orientation: &Orientation, force_magnitude: f32) -> glm::Vec3 {
    let mut dir = glm::vec3(0.0, 0.0, 0.0);

    if input.key_w.as_ref().is_on() { dir.z -= 1.0; }
    if input.key_s.as_ref().is_on() { dir.z += 1.0; }
    if input.key_a.as_ref().is_on() { dir.x -= 1.0; }
    if input.key_d.as_ref().is_on() { dir.x += 1.0; }
    if input.left_shift.as_ref().is_on() { dir.y -= 1.0; }
    if input.space.as_ref().is_on() { dir.y += 1.0; }

    if glm::length(&dir) == 0.0 {
        return glm::vec3(0.0, 0.0, 0.0);
    }

    let world_dir = glm::quat_rotate_vec3(&orientation.quaterion, &glm::normalize(&dir));
    world_dir * force_magnitude
}
