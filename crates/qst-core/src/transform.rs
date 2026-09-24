use super::*;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TransformState {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl TransformState {
    pub fn identity() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn lerp(self, next: Self, alpha: f32) -> Self {
        Self {
            translation: self.translation.lerp(next.translation, alpha),
            rotation: self.rotation.slerp(next.rotation, alpha),
            scale: self.scale.lerp(next.scale, alpha),
        }
    }
}
