use winit::keyboard::KeyCode;


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