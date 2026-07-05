Procedural Texture Expansion
This package contains 25 additional terrain texture recipes plus recommended revisions of the four supplied samples. All generated files stay inside the three observed generator contracts: `Ground`, `Rock`, and `Sand`. No unsupported generator keys were added.

Schema recommendations for a future version
These are not added to the current files because the loader may reject unknown fields:
Add a physical scale field such as `meters_per_tile` or `texel_size_m`.
Define whether RGB triplets are interpreted as linear or sRGB values.
Make the top-level seed authoritative, or validate that it exactly matches the nested generator seed.
Add optional per-channel controls for height amplitude, ambient occlusion, and wetness response.
Add dedicated generators for layered strata, cracks, pebbles/cobbles, snow, ice, roots, and leaf shapes rather than forcing those surfaces through generic noise.
Add an automated seam test that compares opposing texture borders when `tileable: true`.
Keep 512px for previews; allow 1024–2048px exports for close camera views.

Additional textures
Ground family
dry grassland
lush meadow
moss
wet mud
compacted clay
peat bog
leaf litter
river silt
volcanic ash
tundra ground
red earth
jungle loam
Rock family
basalt
granite
limestone
sandstone
shale
scree
river gravel
weathered cliff
Sand family
wet beach sand
coral sand
black volcanic sand
dune sand
tidal-flat sand
Notes on use
The variants are designed as base materials. Biome realism will improve further when the terrain shader blends them using slope, elevation, moisture, sediment, coast distance, geology, and exposure masks rather than assigning a single material per biome.