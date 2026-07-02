These questions are designed to lock down each system’s **gameplay purpose, simulation boundaries, data model, multiplayer implications, persistence requirements, and integration with the voxel world and quest-driven AI**. They assume a data-driven Rust/Bevy architecture in which the engine owns authoritative world state and quests express desired world-state changes.  

## 1. Combat

1. Is combat fully real-time, cooldown-based, action-timed, or capable of switching between real-time and turn-like tactical pacing?
2. Should attacks use hitboxes and physical projectiles, statistical accuracy rolls, or a hybrid model?
3. How much should facing, reach, elevation, cover, stance, movement, stamina, and terrain affect attack and defense?
4. What damage model is required: simple health, body parts, wounds, armor layers, status effects, structural damage, or some combination?
5. How should weapons interact with voxel materials, shields, creatures, buildings, vegetation, and water?
6. Can combatants surrender, flee, become incapacitated, be captured, or negotiate instead of fighting to the death?
7. How should NPC combat decisions balance immediate reactions, group tactics, schedules, survival needs, and active quest commitments?
8. Which combat results must be deterministic for replays, saves, multiplayer synchronization, and automated testing?

## 2. Inventory

1. Is inventory limited by slots, weight, volume, container dimensions, accessibility, or a combination?
2. Are items discrete entities, stackable commodities, or either depending on their type and condition?
3. Can containers be nested, and how deeply may items, bags, vehicles, structures, and inventories contain one another?
4. How should ownership, theft, reservations, faction property, abandoned goods, and disputed claims be represented?
5. Can items degrade, spoil, become wet, burn, corrode, contaminate other items, or change material state?
6. How should equipped, carried, packed, stored, and nearby items differ in interaction speed and availability?
7. What happens to contents when a container, vehicle, structure, corpse, or parent entity is destroyed?
8. Which inventory operations must be transactional to prevent duplication during saving, multiplayer, quest delivery, or simulation-LOD transitions?

## 3. Quests

1. Which world conditions can generate quests: personal needs, settlement shortages, faction plans, ecological pressures, discoveries, hazards, or authored events?
2. Should quest completion always be verified through authoritative world predicates rather than scripted objective checkboxes?
3. How many alternative methods may satisfy a goal, and can the player propose a solution that was not selected by the original planner?
4. Which quests remain internal AI work, and what causes one to become observable, publicly available, secret, or accepted by the player?
5. How should assignments, bidding, reservations, delegation, subcontracting, competition, and partial fulfillment work?
6. What happens when another NPC completes the goal, the goal becomes obsolete, the issuer dies, or the world changes enough to invalidate the plan?
7. How should quest failure generate escalation, political consequences, new pressures, or replacement quests?
8. What information should the player know about a quest, and how are rumors, uncertainty, false beliefs, hidden causes, and discoveries represented?

The existing quest architecture already supports a shared lifecycle of need, pressure, goal, planning, assignment, execution, and world-state verification rather than separate player and NPC quest systems. 

## 4. NPC AI

1. What belongs to immediate reactive AI, routine scheduling, personal goals, assigned quests, faction strategy, and long-term planning?
2. How do NPCs prioritize danger, survival, combat, mandatory duties, schedules, personal desires, and quest commitments?
3. Do NPCs act from individual knowledge and beliefs, or may some systems access omniscient world state?
4. How should skills, personality, ideology, relationships, permissions, equipment, and risk tolerance affect decisions?
5. Which behaviors require detailed physical execution, and which may be completed statistically when NPCs are off-screen?
6. How do individual NPCs merge into or separate from households, work crews, squads, crowds, or macro entities?
7. How should blocked plans generate child tasks such as acquiring tools, requesting assistance, finding another route, or abandoning the goal?
8. What debugging information must explain why an NPC selected, rejected, delayed, or abandoned a particular action?

## 5. Crafting

1. Is crafting recipe-based, component-based, process-based, material-property-based, or a layered combination?
2. Can materials be substituted according to hardness, flexibility, conductivity, quality, culture, or other physical tags?
3. Which crafts occur instantly, through timed work, at workstations, through multi-stage production chains, or through background labor?
4. How do skill, tools, workstation condition, environment, input quality, and worker fatigue affect output?
5. Are intermediate products, waste, by-products, failed items, and partially completed work represented physically?
6. Can recipes be discovered, taught, reverse-engineered, culturally restricted, patented, forgotten, or improved?
7. How do NPCs and settlements create crafting work orders from shortages, maintenance needs, trade plans, and quest blockers?
8. What provenance must crafted items retain, such as maker, materials, recipe version, quality, ownership, and production site?

## 6. Full Save Games

1. Is the entire world saved exactly, or regenerated from deterministic seeds with only player and simulation changes stored as deltas?
2. Which systems own their save data, and which indexes, caches, meshes, navigation data, and presentation state should be rebuilt after loading?
3. How are stable IDs maintained across ECS entity destruction, chunk unloading, procedural regeneration, and schema upgrades?
4. Must saving be possible during combat, asynchronous generation, chunk transitions, multiplayer sessions, or partially applied simulation passes?
5. How will atomic writes, backup saves, corruption detection, checksums, and recovery from interrupted saves work?
6. What generator and content versions must be stored so older worlds remain loadable after terrain, structure, or quest algorithms change?
7. How will references to unloaded NPCs, active quests, reserved resources, scheduled events, and partially generated structures be restored?
8. What determinism test will prove that “run, save, load, continue” produces the same simulation result as uninterrupted execution?

## 7. Multiplayer

1. Is multiplayer cooperative, competitive, persistent-server, peer-hosted, drop-in/drop-out, or some supported subset?
2. Will the server be authoritative over movement, combat, inventory, terrain edits, quests, AI, crafting, and procedural generation?
3. Which systems require client prediction, interpolation, lag compensation, rollback, or simple server confirmation?
4. How should players share, compete over, delegate, sabotage, or independently complete simulation-generated quests?
5. What happens when two players edit the same terrain, claim the same item, damage the same structure, or reserve the same work?
6. How are procedural chunks synchronized: shared seed and version, transmitted field data, transmitted voxel deltas, or complete chunk snapshots?
7. What portions of the world and simulation should be replicated to each client based on distance, visibility, faction knowledge, or relevance?
8. How will disconnection, host migration, reconnecting, server persistence, cheating, and incompatible mods or data files be handled?

## 8. Large-World Streaming

1. What are the distinct units for generation, voxel storage, rendering, physics, navigation, AI simulation, persistence, and networking?
2. How many detail levels are needed for terrain, structures, vegetation, water, NPCs, and distant horizon silhouettes?
3. What determines chunk priority: player distance, camera direction, velocity, quest relevance, visibility, network ownership, or predicted movement?
4. Which data remains resident after a chunk unloads, such as regional summaries, structure state, NPC commitments, hydrology, and terrain deltas?
5. How are seamless borders guaranteed for density samples, meshes, lighting, water, navigation, weather, and edited terrain?
6. How should detailed NPC simulation transition into regional or statistical simulation without duplicating inventory, progress, or movement?
7. What frame-time and memory budgets apply to generation, meshing, collider creation, uploads, unloading, and save operations?
8. How will teleportation, rapid vehicles, multiplayer separation, and quests spanning distant islands affect streaming strategy?

The terrain plan already distinguishes persistent world fields, generation regions, voxel chunks, render chunks, and simulation regions rather than forcing them into one chunk abstraction. 

## 9. Dynamic Weather

1. Is weather locally simulated from fields such as pressure, humidity, temperature, wind, elevation, and ocean influence, or selected from regional state machines?
2. At what spatial scale do storms, clouds, fog, rainfall, wind, lightning, and temperature vary?
3. How should mountains, coastlines, forests, ocean temperature, and prevailing winds modify weather?
4. Which gameplay systems are affected: visibility, movement, fire, water levels, erosion, farming, sailing, combat, AI schedules, and creature behavior?
5. Can weather create lasting world changes such as floods, landslides, damaged structures, wet inventories, crop failure, or altered river discharge?
6. How predictable should weather be, and what forecasts, signs, instruments, NPC knowledge, or uncertainty should be exposed?
7. How does the weather system continue in unloaded regions, and how are large time skips processed without simulating every minute?
8. Which weather state must be saved or synchronized so loading and multiplayer clients reproduce the same conditions?

## 10. Day/Night Cycle

1. What is the game-time scale, and can it change during travel, sleep, menus, combat, multiplayer, or accelerated simulation?
2. Will the sun and moon follow physically inspired celestial paths, simplified authored curves, or configurable world-specific models?
3. How do latitude, season, moon phase, cloud cover, atmospheric conditions, and terrain occlusion affect illumination?
4. Which behaviors depend on time of day: schedules, predators, visibility, stealth, temperature, tides, plants, quests, and settlement activity?
5. Is darkness a physical lighting result, a gameplay visibility modifier, or both?
6. How should artificial lights, fire, moonlight, reflected light, caves, interiors, and transitions between them be handled?
7. What happens to scheduled events and simulations when the player sleeps or advances time by several hours or days?
8. Which celestial and clock state must remain deterministic across saving, multiplayer, and procedural world generation?

## 11. Destructible Gameplay Terrain

1. Can players modify only surface materials, or may they dig tunnels, collapse caves, divert rivers, undermine buildings, and reshape coastlines?
2. Is the authoritative editable representation occupancy voxels, signed-density samples, material cells, constructive operations, or a hybrid?
3. How do tools, explosives, impacts, fire, water, material hardness, fractures, and support affect terrain damage?
4. Is structural collapse fully simulated, locally approximated, or triggered through validated support rules?
5. How are terrain edits propagated to meshes, collision, navigation, water, lighting, vegetation, structures, AI plans, and quests?
6. What limits prevent players or simulation events from creating excessive remeshing, physics load, save growth, or multiplayer bandwidth?
7. How are modifications stored: changed samples, density deltas, operation records, modified chunk snapshots, or a hybrid?
8. Can natural processes repair or further alter terrain through erosion, sedimentation, plant growth, volcanic activity, or construction?

The volumetric terrain design uses a signed-density field specifically so caves, overhangs, excavation, and persistent terrain modification remain possible rather than being constrained to heightmap columns. 

## 12. Building System

1. Is construction freeform voxel editing, modular placement, blueprint-based assembly, room-based construction, or a combination?
2. Must buildings obey structural support, foundations, material strength, weather exposure, access, ventilation, utilities, and terrain suitability?
3. Are construction materials generic quantities, individual physical items, processed components, or all three at different production stages?
4. Who performs construction: the player directly, assigned NPC labor, contractors, autonomous settlements, or abstract off-screen work crews?
5. Can blueprints reserve terrain, materials, labor, tools, and access routes before physical construction begins?
6. How are doors, stairs, roofs, floors, rooms, utilities, workstations, storage, defenses, and navigation recognized semantically after placement?
7. Can generated structures and player buildings use the same spatial graph, voxel realization, damage, occupancy, and persistence systems?
8. How do modification, repair, expansion, ownership transfer, abandonment, destruction, and historical layering alter an existing building?

A shared structure framework can represent generated ruins, bunkers, villages, caves, and player construction through semantic graphs, spatial blueprints, voxel realization, and persistent simulation state. 
