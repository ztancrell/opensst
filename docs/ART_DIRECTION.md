# Art Direction: Starship Troopers Extermination Style

**Vision:** Replace procedural/AI-generated meshes with a cohesive, grounded military-sci-fi aesthetic inspired by **Starship Troopers: Extermination** — chitinous bugs, industrial UCF structures, and alien planets that feel like a real war zone.

---

## Core Pillars

| Pillar | Description |
|--------|-------------|
| **Grounded realism** | No cartoon proportions. Bugs feel like biological threats; terrain feels like real alien geology. |
| **Military-industrial** | UCF structures: blocky, functional, weathered. Think prefab bases, not fantasy castles. |
| **Chitinous threat** | Bugs: segmented carapace, visible weak points, distinct silhouettes per class. Dark, glossy, organic. |
| **Readable at distance** | Silhouettes and colors must read in combat. Bug types identifiable at 50m+. |

---

## Bug Design (Arachnids)

### Current Issue
Procedural `BugMeshGenerator` produces generic insectoid shapes. They lack the **chitinous armor plates**, **distinct class silhouettes**, and **weak-point clarity** of STE.

### STE Reference: Bug Classes

| Class | Silhouette | Key Features | Color Direction |
|-------|------------|--------------|-----------------|
| **Warrior** | Mid-size, 6–8 legs, forward mandibles | Armored thorax, sharp pincers, aggressive stance | Dark brown/black carapace, reddish undertones |
| **Charger** | Sleek, low profile, fast | Streamlined, less armor, built for speed | Darker, more matte |
| **Tanker** | Large, heavily armored | Thick plates, slow, imposing | Heavier armor plates, grey-brown |
| **Spitter/Gunner** | Bloated rear sac, ranged | Acid sac visible, distinct from melee bugs | Greenish tint on sac, dark body |
| **Hopper** | Wings/leaping legs | Jump-focused, lighter build | Similar to Warrior but more angular |

### Recommendations

1. **Replace procedural meshes with authored silhouettes**
   - Use hand-crafted low-poly meshes (or simplified STE-inspired shapes) instead of `BugMeshGenerator` output
   - Each bug type: **one canonical mesh** with clear silhouette
   - Optional: LOD variants for distance (simplified shapes at 80m+)

2. **Chitinous material**
   - Dark base: `#1a1510`–`#2a2218` (warm black-brown)
   - Specular highlights: glossy, not plastic — subtle sheen on armor plates
   - Rim lighting on carapace edges for readability
   - Weak points: slightly lighter, less glossy (nerve stem, joints)

3. **Armor plates**
   - Segmented plates on thorax/abdomen, not smooth blobs
   - Plates should catch light differently from joints
   - Consider normal maps (or vertex normals) for plate edges

4. **Distinct silhouettes**
   - **Warrior**: Broad, low, mandibles forward
   - **Charger**: Elongated, legs tucked, aerodynamic
   - **Tanker**: Wide, tall, heavy plates
   - **Spitter**: Bulbous rear, smaller head
   - **Hopper**: Wings or extended legs, lighter build

---

## Environment & Props

### Current Issue
`EnvironmentMeshes` uses generic primitives: spheres, cubes, low-poly rocks. Landmarks (UCFBase, bug holes, hive mounds) are scaled cubes/spheres — they read as placeholders.

### STE Reference
- **Bug holes**: Organic crater rims, resin-like edges, dark interior
- **Hive structures**: Organic, bulbous, resin-coated — not geometric
- **UCF structures**: Blocky, military grey, industrial — functional, not decorative
- **Terrain**: Varied but grounded — dust, rock, alien flora that reads as hostile

### Recommendations

1. **Bug holes**
   - Replace sphere with **organic crater mesh**: irregular rim, sloping sides, dark interior
   - Resin-like material: dark, slightly glossy, organic
   - Optional: subtle animation (pulsing, steam)

2. **Hive mounds / egg clusters**
   - Organic shapes: blobs, tendrils, not spheres
   - Resin texture: amber-brown, translucent where thin
   - Egg clusters: clustered ovoids, not uniform spheres

3. **UCF structures (bases, walls, colonies)**
   - Keep blocky — that’s correct for military prefab
   - Materials: matte grey `#3a3d42`, rust accents `#5c3d2e`, industrial
   - Add **beveled edges** — sharp corners read as low-poly; slight bevels read as manufactured
   - Optional: panel lines, vents, antennae for scale

4. **Rocks / terrain props**
   - Replace uniform spheres with **authored rock meshes**: 3–5 variants per biome
   - Angular, fractured — not smooth blobs
   - Scale and rotate for variety; avoid obvious repetition

---

## Terrain & Materials

### Current Issue
Procedural terrain with per-vertex biome colors. Can feel flat or noisy depending on biome.

### STE Reference
- Dusty, desolate, or hostile
- Clear material reads: sand, rock, ash, ice
- Not overly saturated — military ops feel gritty

### Recommendations

1. **Biome material palette**
   - **Desert**: Warm sand `#c4a574`, rock `#6b5d52`
   - **Badlands**: Red-brown `#7d5a50`, dry earth
   - **HiveWorld**: Dark organic `#3d3228`, resin `#4a3d2e`
   - **Volcanic**: Black rock `#2a2520`, ember `#8b4513`
   - **Frozen**: Pale blue-grey `#8b9da8`, ice `#a8c4d4`
   - **UCF/Base**: Industrial grey `#4a4d52`, concrete

2. **Terrain shading**
   - Slightly stronger normal response — terrain should have depth
   - Avoid flat, uniform color — subtle variation reads as real ground
   - Fog and atmosphere: dusty, hazy — not crystal clear

---

## Lighting & Atmosphere

### STE Reference
- Naturalistic lighting — sun, shadows, ambient
- Dust particles in air
- Smoke, fire, explosions feel grounded
- No overly stylized bloom or color grading

### Recommendations

1. **Sun direction**
   - Strong directional light — long shadows read as military drama
   - Slightly warm sun, cool shadow (natural)

2. **Ambient**
   - Fill light: subtle, not flat
   - Rim/back light on bugs for silhouette readability

3. **Atmosphere**
   - Volumetric dust/fog in key biomes
   - Haze in distance — not infinite draw distance clarity

---

## UI & HUD

### STE Reference
- Military, functional, readable
- Green/amber tactical displays
- Minimal decoration — information first

### Recommendations
- HUD: High contrast, readable at a glance
- Colors: Green `#00ff00` or amber `#ffaa00` for tactical elements
- Font: Monospace or military-style — not decorative

---

## Implementation Roadmap

### Phase 1: Bug Meshes (Highest Impact)
1. **Author replacement meshes** for Warrior, Charger, Tanker, Spitter, Hopper
   - Use Blender or similar; export as OBJ/GLTF
   - Low-poly (500–2000 tris per bug) for performance
2. **Replace `BugMeshGenerator`** with mesh loading from assets
3. **Update bug materials** — darker base, specular, rim for readability

### Phase 2: Environment Props
1. **Bug hole** — organic crater mesh
2. **Hive mound / egg cluster** — organic blob meshes
3. **Rock variants** — 3–5 authored rocks per biome
4. **UCF structures** — beveled cubes, panel detail

### Phase 3: Terrain & Materials
1. **Refine biome color palettes** per above
2. **Terrain normal/roughness** if shader supports
3. **Atmosphere/dust** tuning

### Phase 4: Polish
1. **Lighting** pass
2. **HUD** styling
3. **Death/gore** — chitin shards, acid splatter (STE has satisfying bug kills)

---

## Asset Pipeline Suggestion

- **Format**: GLTF 2.0 or OBJ for simplicity
- **Location**: `assets/meshes/bugs/`, `assets/meshes/env/`
- **Naming**: `warrior.gltf`, `bug_hole.gltf`, `rock_desert_01.gltf`
- **Loader**: Add `load_mesh(path)` to replace procedural generation where needed

---

## Summary

Move from **procedural generality** to **authored specificity**. Each bug type, each prop, each structure should have a clear, intentional design that reads in combat and supports the military-sci-fi tone. Starship Troopers Extermination succeeds because its bugs feel like a real threat — chitinous, armored, distinct — and its world feels like a war zone. Match that intent.
