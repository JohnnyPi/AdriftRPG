//! Stable field identifiers for atlas storage and pass I/O.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FieldKey {
    BoundaryDistance,
    BoundaryMask,
    OceanBasin,
    IslandInfluence,
    IslandId,
    IslandAge,
    BaseElevation,
    Bathymetry,
    CoastDistance,
    LandMask,
    Bedrock,
    RockHardness,
    Erodibility,
    Permeability,
    FractureIntensity,
    ValueConstraint,
    GradientConstraint,
    RegionalResidual,
    FinalElevation,
    // Phase 8 — climate
    Temperature,
    Rainfall,
    Humidity,
    Evaporation,
    WindExposure,
    // Phase 9 — hydrology
    FilledElevation,
    FlowDirection,
    FlowAccumulation,
    Runoff,
    RiverMask,
    LakeMask,
    WetlandMask,
    Slope,
    SedimentThickness,
    // Phase 10 — erosion
    ErodedElevation,
    // Phase 11 — coastal / marine
    BeachSuitability,
    CliffSuitability,
    ReefSuitability,
    LagoonSuitability,
    MangroveSuitability,
    TidalFlatSuitability,
    SeaCaveSuitability,
    WaveExposureCoastal,
    ShelfMask,
    CoastalElevation,
    // Phase 12 — ecology
    SoilDepth,
    PrimaryBiome,
    BiomeBlendWeight,
    // Phase 13 — strata
    RegolithDepth,
    WeatheringDepth,
    DepositMask,
    // Phase 14 — caves
    LavaTubeSuitability,
    LimestoneCaveSuitability,
    SeaCaveRegionSuitability,
    // Phase 15 — water carve
    CarvedElevation,
}
