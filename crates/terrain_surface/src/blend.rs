use crate::material_id::TerrainMaterialId;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceMaterialBlend {
    pub materials: [TerrainMaterialId; 4],
    pub weights: [f32; 4],
}

impl SurfaceMaterialBlend {
    pub fn single(material: TerrainMaterialId) -> Self {
        Self {
            materials: [material; 4],
            weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn normalize(mut self) -> Self {
        for weight in &mut self.weights {
            *weight = weight.max(0.0);
        }
        let sum: f32 = self.weights.iter().sum();
        if sum <= f32::EPSILON {
            self.weights = [1.0, 0.0, 0.0, 0.0];
        } else {
            for weight in &mut self.weights {
                *weight /= sum;
            }
        }
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TerrainMaterialVertex {
    pub indices: [u32; 4],
    pub weights: [f32; 4],
}

pub fn resolve_blend(
    blend: SurfaceMaterialBlend,
    layers: &crate::registry::MaterialLayerRegistry,
) -> TerrainMaterialVertex {
    TerrainMaterialVertex {
        indices: blend.materials.map(|id| layers.layer(id)),
        weights: blend.weights,
    }
}

pub fn validate_blend(blend: SurfaceMaterialBlend) {
    assert!(
        blend.weights.iter().all(|value| value.is_finite() && *value >= 0.0),
        "invalid blend weights"
    );
    let sum: f32 = blend.weights.iter().sum();
    assert!((sum - 1.0).abs() < 0.01, "blend weights sum to {sum}");
}

pub trait SurfaceClassifier: Send + Sync {
    fn classify(&self, context: &crate::context::SurfaceContext) -> SurfaceMaterialBlend;
}

pub trait SurfaceMeshResolver: Send + Sync {
    fn vertex_blend(&self, position: [f32; 3], normal: [f32; 3]) -> ([u16; 4], [f32; 4]);
}
