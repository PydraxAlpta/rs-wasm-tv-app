//! Framerate-independent value smoothing (exponential ease toward a target).

/// A scalar that eases toward a target with a time-constant `tau` (seconds).
///
/// The smoothing is frame-rate independent: the fraction covered per step is
/// `1 - e^(-dt/tau)`, so behaviour is identical at 30fps or 120fps.
#[derive(Debug, Clone, Copy)]
pub struct Tween {
    current: f32,
    target: f32,
    tau: f32,
}

impl Tween {
    pub fn new(value: f32, tau: f32) -> Self {
        Self {
            current: value,
            target: value,
            tau,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Jump instantly to `value` (no animation).
    pub fn snap(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    pub fn value(&self) -> f32 {
        self.current
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn is_settled(&self) -> bool {
        (self.target - self.current).abs() < 1e-3
    }

    /// Advance by `dt` seconds and return the new value.
    pub fn step(&mut self, dt: f32) -> f32 {
        if self.tau <= 0.0 || dt <= 0.0 {
            self.current = self.target;
            return self.current;
        }
        let alpha = 1.0 - (-dt / self.tau).exp();
        self.current += (self.target - self.current) * alpha;
        if self.is_settled() {
            self.current = self.target;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_toward_target() {
        let mut t = Tween::new(0.0, 0.1);
        t.set_target(10.0);
        for _ in 0..1000 {
            t.step(0.016);
        }
        assert!((t.value() - 10.0).abs() < 1e-3);
        assert!(t.is_settled());
    }

    #[test]
    fn moves_in_right_direction_and_is_bounded() {
        let mut t = Tween::new(0.0, 0.1);
        t.set_target(1.0);
        let after = t.step(0.016);
        assert!(after > 0.0 && after < 1.0, "should move partway, got {}", after);
    }

    #[test]
    fn snap_skips_animation() {
        let mut t = Tween::new(0.0, 0.1);
        t.snap(5.0);
        assert_eq!(t.value(), 5.0);
        assert!(t.is_settled());
    }
}
