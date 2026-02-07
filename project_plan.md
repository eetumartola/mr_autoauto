# Mr. Autoauto - Project Plan (Bevy + Splats + AI Commentator)

## 0) One-paragraph pitch
**Mr. Autoauto** is a side-scrolling Hill Climb-style driving action game: you control a vehicle with **two buttons** (accelerate / brake; in-air rotate CCW/CW). You drive across a 2D physics plane while a turret **auto-fires** at enemies (Mr Autofire vibe). Levels are made from **concatenated linear Gaussian Splat background segments**. A runtime **AI commentator** (Neocortex Web API) narrates stunts and combat based on real game events (big jumps, wheelies, kills, crashes, speed milestones).

**Core design goal:** ship a stable, fun, readable loop fast, and keep the game continuously playable while layering in content and polish.

---

## 1) Core gameplay loop
### Moment-to-moment (10-30 seconds)
- Drive forward across terrain: throttle/brake, manage pitch in air.
- Auto-turret engages enemies in front/around the vehicle.
- Dodge projectiles + terrain hazards; maintain speed.
- Perform stunts (air time, wheelies, flips) for score multipliers / pickups.
- AI commentator reacts to meaningful events.

### Run-to-run (2-5 minutes)
- Progress through a sequence of splat background segments.
- Difficulty escalates with distance and segment type.
- Periodic mini-boss / boss encounters.
- Earn coins/score -> pick upgrades (weapon/vehicle perks).

### Meta (hackathon scope)
- Minimal persistence: last run stats, high score, optionally unlocked upgrades.
- No complex economy; keep upgrades impactful and readable.

---

## 2) Non-goals (to avoid scope creep)
- No handcrafted character animation requirements (enemies start as quads).
- No complex multi-lane / multi-vehicle racing.
- No procedural terrain extraction from splats (optional post-hack upgrade).
- Web build is **desirable**, but not a blocker if it threatens stability.

---

## 3) Technical constraints & approach
- World is **3D**, but gameplay is **2D in the screen plane** (x-y plane, fixed z).
- Start with **quads** for everything moving (player, bullets, enemies, pickups).
- Terrain v0 is **straight**; later upgrade to spline/heightfield.
- Gaussian splats are rendered as background strips via **bevy_gaussian_splatting**.
- Keep systems **data-driven** via **TOML** for:
  - background/segment sequencing and environment parameters
  - enemy types + spawners
  - weapons/vehicles/upgrades
  - AI commentator rules and thresholds
- Engine target: **Bevy 0.17**
- Splat renderer target: **bevy_gaussian_splatting v6.0**
- Physics backend target (Epic B+): **bevy_rapier2d** (mature 2D joints + collisions; start simple, then add wheel/suspension behavior)
- Input defaults: A/D and Left/Right
- No splat assets yet: use simple polygon/box background placeholders until assets arrive.
- AI commentary implementation plan: ship a local stub first; wire real Neocortex chat->audio API later in Epic G.

---

## 4) Architecture overview (ECS shape)
### Key entities
- **PlayerVehicle**
  - components: `Transform`, `Velocity2D`, `VehicleState`, `GroundContact`, `Health`, `ScoreState`
- **Turret**
  - components: `TurretConfig`, `TurretState`, `Targeting`
- **Projectile** (bullet / missile)
  - components: `ProjectileConfig`, `Lifetime`, `Damage`, `Team`, `Trail`
- **Enemy**
  - components: `EnemyTypeId`, `Health`, `AIState`, `Team`, `DropTable`
- **Spawner**
  - components: `SpawnerConfig`, `SpawnerState` (distance-based, timed, wave, boss gate)
- **Terrain**
  - components: `TerrainProfile` (v0 straight, v1 spline samples), `Collision`
- **BackgroundSegment (Splat)**
  - components: `SplatAssetRef`, `SegmentConfig`, `Bounds`, `EnvironmentModifiers`
- **FX**
  - tracer, hit sparks, explosions, dust puffs (all sprite/billboard quads initially)
- **UI**
  - HUD (speed, distance, health, score, upgrade prompts), debug overlay

### Key resources
- `GameConfig` (merged TOML)
- `RunState` (distance, segment index, difficulty scalar)
- `InputState` (accelerate/brake, rotate intent)
- `AICommentaryQueue` (events awaiting prompt packaging)
- `AudioQueue` (decoded narrator audio ready to play)
- `AssetRegistry` (loaded splats, sprites, sounds)

### Core system groups (recommended ordering)
1. Input -> `InputState`
2. Vehicle physics & terrain contact
3. Enemy + spawner updates
4. Turret targeting + firing
5. Projectile simulation + collisions
6. Damage/health resolution + death/drops
7. Scoring/stunts computation
8. AI event emission & commentary requests
9. Audio playback
10. UI update
11. Segment streaming (load/unload backgrounds)

---

## 5) Data-driven TOML plan (initial file set)
Keep schemas **simple and bounded** (IDs reference rows). Validate on load; fail fast with good error messages.

### 5.1 `config/game.toml`
- global settings: tick rates, difficulty ramps, score scaling, camera settings
- input bindings (keyboard + touch mapping flags)
- debug toggles

### 5.2 `config/segments.toml`
Defines the ordered level: list of segment IDs and their placement rules.

Example:
```toml
[[segment_sequence]]
id = "museum_hall_01"
length = 120.0
environment = "normal"

[[segment_sequence]]
id = "ice_cave_01"
length = 140.0
environment = "ice"
```

### 5.3 `config/backgrounds.toml`
- splat asset path(s)
- cylinder unwrap parameters (if needed)
- parallax hints (optional)
- per-segment environment override

### 5.4 `config/environments.toml`
- `gravity`, `drag`, `traction`, `air_control`, `wheel_friction`, `projectile_drag`
- also "style" knobs: dust amount, impact FX intensity

### 5.5 `config/enemy_types.toml`
- health, speed, contact damage
- attack pattern IDs (simple: shooter, charger, turret)
- sprite/mesh asset refs
- size/hitbox

### 5.6 `config/spawners.toml`
- spawner archetypes: distance-triggered, timed, wave, boss gate
- spawn lanes/offsets relative to terrain
- max alive, cooldowns, scaling with difficulty

### 5.7 `config/weapons.toml`
- bullet speed, fire rate, spread, damage, projectile type
- homing missile parameters (optional)
- upgrade hooks (which stats can be modified)

### 5.8 `config/vehicles.toml`
- mass-ish params, acceleration, brake strength
- pitch torque in air
- suspension/grounding params (even if simplified)
- health

### 5.9 `config/upgrades.toml`
- named upgrades, rarity, max stacks
- stat deltas (add/mul), unlock conditions
- UI text

### 5.10 `config/commentator.toml`
- thresholds for calling out: airtime, wheelie time, flip count, speed tier
- thresholds should include magnitude buckets (for example big vs huge jump) so game emits precise factual event descriptors.
- rate limiting: min seconds between voice lines, priority rules
- two commentator profiles (character IDs/style/voice settings + subtitle colors)
- round-robin scheduler rules (which commentator speaks next)
- template prompts + "style" settings
- prompt context should include what the other commentator said last time
- game-side event summaries should stay dry/factual ("player made a huge jump", "player is near death"); stylistic color comes from the LLM layer.
- fallback lines (non-AI) if API fails

---

## 6) Epics and initial task list

### Task status legend
- [not started] not implemented yet.
- [in progress] currently being worked on.
- [blocked] waiting for dependency/decision.
- [done] implemented and validated.
> **Principle:** every epic should keep the game runnable and fun at each step.  
> Each epic has "Definition of Done" (DoD) so you can cut scope cleanly.

### Epic A - Project skeleton & config loading
**Goal:** boot to a playable scene with config-driven entities.

**Tasks**
- [done] A1. Bevy app scaffold (states: `Boot -> Loading -> InRun -> Pause -> Results`).
- [done] A2. TOML loader + schema structs; validate references (enemy IDs, weapon IDs, env IDs).
- [done] A3. Hot-reload (optional but high value): re-read TOML on keypress.
- [done] A4. Basic asset registry (sprites, placeholder polygons/boxes, audio placeholders).
- [done] A5. Minimal debug overlay (FPS, distance, active segment, enemy count).
- [done] A6. Commentary stub pipeline (event queue + debug text output, no network).
- [done] A7. Vehicle/physics live tuning panel:
  - toggle with `V`; sliders + free-form float input per parameter.
  - edits apply live in-memory during run.
  - `Apply` writes current values back to `config/vehicles.toml`.

**DoD**
- Running build loads configs, spawns player + a background segment, no panics, shows HUD/debug.

---

### Epic B - Vehicle controller (Hill Climb core)
**Goal:** two-button driving feels "good enough" quickly; physics is stable.

**Tasks**
- [done] B1. Input mapping (keyboard + gamepad optional; touch overlay for web optional).
- [done] B2. Vehicle kinematics v0:
  - integrate velocity, gravity, clamp, basic ground collision with flat ground
  - grounded vs airborne state
- [done] B3. In-air rotation controls (CCW/CW based on accel/brake).
- [done] B4. Camera follow (look-ahead based on speed; fixed z).
- [done] B5. Terrain v1 placeholder: simple height function (sine/ramps) from config.
- [done] B6. Stunt metrics:
  - airtime, wheelie timer, flip detection, max speed, crash detection.

**DoD**
- You can drive, jump, rotate in air, land without jitter; stunt metrics are tracked.

---

### Epic B+ - Player vehicle refinement (chassis/turret/tires + suspension + 3D parts)
**Goal:** move from placeholder single-body car to a modular physical vehicle that supports final art and better feel.

**Tasks**
- [done] BR1. Split vehicle into modular entities:
  - chassis root, turret mount, front tire(s), rear tire(s) as explicit child entities/components.
- [done] BR2. Rear-wheel drive implementation:
  - apply drive force/torque only through rear tire contact; front tires are non-driven by default.
- [done] BR3. Tire suspension model:
  - add spring-damper behavior per tire with configurable rest length, stiffness, damping, and travel limits.
- [done] BR4. Tire-ground contact and push response:
  - integrate tire contact, traction/slip tuning, and stable collision response between chassis/enemies/terrain.
- [in progress] BR4a. Rapier migration pass for current driving dynamics:
  - run vehicle body under `bevy_rapier2d` rigidbody/collider simulation.
  - keep box-collider terrain for tuning pass before visual polish.
  - preserve current control semantics (rear-drive traction, spring behavior, stunt telemetry) on top of Rapier.
  - tune starter values from vehicle-physics references (spring frequency/damping ratio ranges) and validate against in-game feel.
- [done] BR4b. Spline-style ground and measurement aid:
  - replace jagged tower ground with thick extruded spline-strip segments for both visuals and fixed colliders.
  - add lower-left yardstick overlay with 5m minor notches and 10m major notches.
- [not started] BR5. 3D part asset schema and import pipeline:
  - define separate model refs for chassis/turret/tire parts and/or node-segment extraction from source model.
  - add config for attachment points/local offsets so parts mount at correct locations.
- [not started] BR6. Runtime assembly + validation:
  - assemble parts into correct hierarchy at spawn, keep transforms synchronized, and add debug checks for misalignment/scale.
- [not started] BR7. Visual migration pass:
  - replace coder-art placeholders with production part models while preserving physics/tuning behavior.

**DoD**
- Player vehicle runs as modular chassis/turret/tire parts, rear-wheel drive is active, suspension travel is visible/stable, and imported 3D parts are correctly aligned at runtime.

---

### Epic C - Combat: turret, bullets/missiles, hits
**Goal:** Mr Autofire-style "auto shoots and feels punchy."

**Tasks**
- [done] C1. Turret targeting:
  - select nearest enemy within cone/range; configurable prioritization (nearest/strongest).
  - always draw a blue targeting laser to current aim point.
  - always draw two green cone boundary lines parented to car/turret transform.
  - default targeting cone width is 60 degrees and must be configurable/upgradable.
- [done] C2. Firing logic:
  - fire rate, burst/spread, projectile spawn offsets.
- [done] C3. Projectile simulation:
  - bullet: straight + optional drag
  - missile: ballistic + optional homing (bounded turn rate)
- [done] C4. Collision & damage:
  - simple circle/box overlap (2D), friendly-fire rules, hitstop optional.
- [done] C5. Effects v0:
  - tracer sprite, impact sprite, enemy hit flash, simple explosion quad
- [not started] C6. Audio SFX placeholders (gun, hit, explosion) with volume ducking under narration.

**DoD**
- Shooting reliably hits enemies, feedback is readable, and performance stays stable with moderate projectile counts.

---

### Epic D - Enemies & spawners (content without code edits)
**Goal:** enemies spawn from data and create pressure.

**Tasks**
- [done] D1. Enemy "quad" renderer + hitbox.
- [done] D2. Enemy behaviors v0 (config-driven):
  - Walker (ground), Flier (sine hover), Turret (stationary shooter), Charger, Bomber (high straight flight + bomb drops).
- [done] D3. Enemy shooting patterns (simple): aimed shots, arcs, spreads.
- [done] D3a. Enemy body dynamics/collisions on Rapier:
  - enemy bodies now run as Rapier dynamic rigidbodies/colliders (mass/friction/damping/gravity scale by behavior).
  - removed custom enemy overlap impulse solver that caused oversized push impulses against player/enemies.
- [not started] D4. Spawner system:
  - distance-based triggers, timed spawns, max alive, cooldown.
- [not started] D5. Difficulty scaling:
  - scale spawn rate/health/damage with distance and per-segment multiplier.
- [not started] D6. Boss v0:
  - big enemy with phases: spawn adds, fire pattern, weak spot (optional).

**DoD**
- Multiple enemy types appear across distance; boss encounter is possible and ends a segment cleanly.

---

### Epic E - Background segments + streaming + environment modifiers
**Goal:** splat segments are first-class gameplay segments, concatenated linearly.

**Tasks**
- [not started] E1. Define `SegmentConfig` (asset ref, length, env id, spawn sets, music cue).
- [not started] E2. Segment placement:
  - concatenate along +x; maintain a "segment cursor" at current distance.
- [not started] E3. Streaming:
  - load next N segments ahead; unload behind to cap memory.
- [not started] E4. Environment application:
  - when segment is active, apply gravity/drag/traction modifiers smoothly (lerp).
- [not started] E5. Segment "props" v0:
  - simple polygons/planes marking edges / floor framing (museum frame).
- [not started] E6. Seam masking:
  - "door frame" quad at segment boundaries to hide discontinuity.
- [not started] E7. Ground authoring pipeline:
  - implement simple in-game editor mode OR import external spline files for drivable ground.

**DoD**
- You can traverse multiple segments seamlessly; environment changes are noticeable and stable; ground authoring/import path is usable.

---

### Epic F - Scoring, coins, upgrades, and run flow
**Goal:** simple progression that makes replay meaningful.

**Tasks**
- [in progress] F1. Score sources:
  - distance, kills, stunts (airtime/wheelie/flip), "no damage" bonus.
- [not started] F2. Currency drops (coins/parts).
- [not started] F3. Upgrade selection UI:
  - after boss / at checkpoints / on level-up, present 2-3 choices.
- [not started] F4. Upgrade application system:
  - modify weapon/vehicle params; stack rules.
- [done] F5. Run end conditions:
  - health hits 0; show results screen with summary + restart.
- [not started] F6. High score persistence (local file; for web use local storage if available later).

**DoD**
- A full run has an arc: start -> escalation -> upgrades -> fail/win -> summary -> restart quickly.

---

### Epic G - AI commentator integration (Neocortex Web API)
**Goal:** narrator is reliable, rate-limited, and clearly reactive to gameplay.

**Tasks**
- [not started] G1. Event model:
  - `GameEvent` enum (JumpBig, WheelieLong, Flip, Kill, BossKill, Crash, SpeedTier, NearDeath, Streak).
- [not started] G2. Event aggregation:
  - batch events into a compact "what happened" text summary.
  - de-duplicate spammy events; apply cooldowns and priorities.
  - route lines in round-robin order between two commentators.
- [not started] G3. Prompt builder:
  - include run context (segment name, score streak, player health).
  - include what the other commentator said last time.
  - style knobs from `commentator.toml` (tone, length, profanity filter if desired).
- [not started] G4. Neocortex API client:
  - async request queue; cancellation (don't narrate stale moments); maintain per-commentator context/session.
  - retries with backoff; strict timeout.
- [not started] G5. Audio decode & playback:
  - store response to file/memory; feed to Bevy audio.
  - duck SFX/music under narration.
- [not started] G6. Fallback behavior:
  - if API fails/offline, play local canned VO lines or text captions.
- [not started] G7. UI subtitles:
  - draw returned chat message on screen.
  - subtitle color must depend on which commentator spoke.
  - show captions for both commentators in noisy demo spaces and web autoplay restrictions.

**DoD**
- Two commentators react to stunts/kills/crashes in round-robin order with minimal latency and without spamming; colored subtitles identify the speaker; game never stalls on API.

---

### Epic H - UI/UX polish and "Supercell demo readiness"
**Goal:** the game reads instantly to judges.

**Tasks**
- [not started] H1. Title screen + quick start.
- [not started] H2. HUD: health, distance, speed, score, upgrade icons, current segment label.
- [not started] H3. Hit indicators (directional damage, screen shake light).
- [not started] H4. Feedback polish:
  - muzzle flash, screen shake on big hits, dust on landing, coin pickup sparkle.
- [not started] H5. Audio mix:
  - music bed loop; mix levels; narration ducking.
- [not started] H6. Controller / touch affordances:
  - on-screen buttons + haptics (optional).

**DoD**
- A first-time player understands controls in <10 seconds and sees the "AI commentator" clearly.

---

### Epic I - Web build (optional epic; can be dropped)
**Goal:** deliver WASM build with acceptable performance.

**Tasks**
- [not started] I1. Web-compatible asset loading paths.
- [not started] I2. Touch input UI and pointer capture.
- [not started] I3. Audio autoplay policy handling:
  - require first user tap to enable audio; show prompt.
- [not started] I4. Networking constraints:
  - CORS setup for Neocortex endpoint; API key injection strategy.
- [not started] I5. Performance knobs:
  - cap projectiles; lower splat detail; reduce FX.
- [not started] I6. Build pipeline:
  - `wasm32-unknown-unknown` + bundling + simple hosting script.

**DoD**
- The web build runs, controls work, narration works after user gesture, and framerate is "demoable."

---

## 6.1) Decisions log (2026-02-06)
- Start implementation scope with Epic A only.
- Placeholder backgrounds are required now (simple quads/polygons), not real splat content yet.
- Version pinning: Bevy 0.17 and bevy_gaussian_splatting v6.0.
- Rust toolchain pinning for Bevy 0.17 compatibility: `rustc/cargo 1.88.0`.
- `bevy_gaussian_splatting v6.0` currently requires nightly Rust when compiled; keep it feature-gated (`gaussian_splats`) and disabled for now.
- Long-term splat strategy: use a vendored/patch-crate version of `bevy_gaussian_splatting v6.0.0` without the nightly-only `#![feature(lazy_type_alias)]` gate, so builds stay on stable toolchain.
- A2 implementation detail: `config/*.toml` is now loaded/merged at startup with fail-fast validation for cross-file IDs (environment, weapon, enemy, vehicle, spawner).
- A3 implementation detail: press `F5` in-game to hot-reload all `config/*.toml`; invalid reloads are rejected and previous in-memory config stays active.
- A4 implementation detail: `assets.toml` now defines sprite/model/splat/audio catalogs; model entries include hierarchy metadata (`root_node`, `wheel_nodes`, optional `turret_node`) for vehicle-style compositions.
- A5 implementation detail: debug HUD now shows FPS, distance, active segment, enemy count, and commentary queue status.
- Debug overlay visibility detail: overlay text now uses Bevy default UI font fallback (no bundled font required), keybind help is hidden by default, and `H` toggles it.
- A6 implementation detail: commentary stub queue is active with key-driven events (`J` big jump, `K` kill, `C` crash), zero network dependency.
- A7 implementation detail: added a `V`-toggled live vehicle tuning panel (egui) with slider + free-form float controls for all vehicle numeric constants; edits apply in-memory immediately during gameplay.
- A7 persistence detail: tuning panel has `Apply To vehicles.toml`; on apply, it writes the selected vehicle values to `config/vehicles.toml`, reloads `GameConfig`, and rolls back file changes automatically if validation fails.
- A7 safety detail: while the vehicle tuning panel is open, player HP damage intake is disabled (landing impacts, enemy projectile hits, and enemy contact damage).
- B1-B4 implementation detail: Epic B now has keyboard input mapping (`D`/`Right` accelerate, `A`/`Left` brake), visible placeholder player+ground, flat-ground kinematics with grounded/airborne states, in-air pitch control, and speed-based camera follow.
- Visual motion detail: temporary checkerboard pattern was added to both background and ground to make movement readability obvious during placeholder art phase.
- Vehicle feel tuning detail: increased linear speed scaling/caps and replaced frame-based ground friction with time-based damping to produce clearer movement and stronger inertia.
- B5 implementation detail: drivable ground now uses config-driven terrain height from `game.toml` with two overlapping sine waves plus optional ramp slope; checkerboard ground tiles follow this terrain profile.
- Vehicle tuning config detail: speed caps, linear speed scale, damping/inertia, and camera look-ahead are now loaded from `vehicles.toml` per vehicle.
- Terrain readability detail: placeholder ground rendering now uses extruded spline-like columns plus a ridge strip so terrain remains visible at gameplay speed.
- Default driving tune detail: reduced starter-car top speed/look-ahead and adjusted damping in `vehicles.toml` for better readability while preserving inertia.
- Stability fix detail: terrain hot-reload updater uses a single combined query for ridge/body tiles to avoid Bevy query-conflict panic (`B0001`).
- Terrain visibility fix detail: ground tile parent transform is now identity (no vertical offset), so extruded terrain columns/ridge render at the actual spline height.
- Scale normalization detail: removed dual-unit conversion; gameplay/physics/config/HUD now use meters directly, with camera orthographic scale adjusted for readable on-screen size.
- B6 implementation detail: stunt metrics now track airtime (current/best), wheelie time (current/best), flip count, max speed, and crash count from hard/awkward landings; metrics are shown in debug HUD.
- Vehicle mass/jump tuning detail: gravity scale is now vehicle-configurable in `vehicles.toml`; starter car gravity scale was reduced to make jumps possible.
- Epic C targeting readability requirement: always show blue target laser and green target-cone boundaries; cone defaults to 60 degrees and is configurable/upgradable.
- Commentary event-style decision: game systems should emit dry factual descriptors and thresholds (including big/huge buckets); narrator style/tone belongs to LLM output, not gameplay event text.
- Vehicle handling tuning detail: starter car now uses mass/gravity at 70% of prior value, rotational inertia +20%, and linear inertia at 80% of prior value via new vehicle config knobs.
- D1-D2 implementation detail: enemies are now visible quads with hitbox data and config-driven movement behaviors (walker/flier/turret/charger), enabling meaningful turret-targeting work before C1.
- C1 implementation detail: player now has an always-on turret targeting overlay (blue aim laser + two green cone boundary lines parented to the car), with target selection constrained by configurable `turret_range_m` and `turret_cone_degrees` and priority mode (`nearest`/`strongest`) from `vehicles.toml`.
- Bevy ECS stability note: turret visual sync uses disjoint `Query` filters (`Without`) to avoid B0001 transform-access conflicts; non-rendering parent entities that own rendered children now include visibility (`Visibility::Inherited`) to avoid B0004 hierarchy warnings.
- C1 visual alignment fix: laser/cone line center translations are now projected along each line direction so all targeting lines originate at the turret mount on the car instead of crossing around an offset midpoint.
- C1 readability tweak: turret cone and aim lines now render at 30% opacity.
- Loading polish: `assets/sprites/autoauto_logo.jpg` is now shown during `Loading` for a minimum 0.75s before entering `InRun`.
- Loading reliability fix: enabled Bevy `jpeg` feature and made `Loading -> InRun` wait for logo load completion (or fail-fast on load failure) so the logo is visible as soon as the window is up.
- C2 implementation detail: auto-fire now uses `weapons.toml` data for `fire_rate`, `spread_degrees`, `burst_count`, `burst_interval_seconds`, and muzzle spawn offsets (`muzzle_offset_x/y`), spawning visible placeholder projectiles from the turret.
- C3 implementation detail: projectile simulation now supports config-driven bullet drag plus missile ballistic gravity and optional bounded homing turn-rate, with projectile type selected by `weapons.toml::projectile_type`.
- C3 stability fix: projectile simulation queries now use explicit `Without` filters between enemy and projectile transform access to avoid Bevy ECS B0001 conflicts at runtime.
- Missile channel behavior update: player now has a separate optional secondary missile weapon slot (`vehicles.toml::secondary_weapon_id`) with independent cadence (`missile_fire_interval_seconds`, default 2.0s), so bullets and missiles auto-fire in parallel when a target is currently acquired; launched missiles continue homing by physics even if the target later leaves the cone.
- Missile launch vector rule: secondary missiles now always launch along the upper cone boundary direction (not the current blue target ray) before homing behavior takes over.
- C4 implementation detail: player projectiles now resolve 2D overlap against enemy hit radii, apply config-driven projectile damage to live enemy health, despawn consumed projectiles, and despawn enemies at zero HP (player shots do not damage player entities).
- C5 implementation detail: combat feedback now includes projectile tracer sprites, hit impact sprites, enemy hit flash, and simple explosion quads; wounded enemies now show an HP bar above the unit (hidden at full health, visible once damaged).
- C5 tracer readability update: projectile trails now render as attached solid multi-segment lines with per-segment alpha falloff (instead of time-spawned detached tracer pieces).
- Projectile-ground interaction update: bullets and missiles now collide with terrain and despawn on ground impact, spawning impact FX (missiles also trigger explosion FX).
- Player survivability update: player now has HP state and an in-world HP bar above the vehicle; crash-impact landings apply HP damage so the bar reflects vehicle health changes.
- D3 implementation detail: enemies now fire back via behavior-driven patterns (walker/turret aimed shots, flier arcing shots, charger spreads), using their configured weapon IDs and weapon stats from TOML.
- Enemy threat model update: player HP now also takes damage from enemy projectiles and from direct enemy overlap/contact (continuous damage while colliding, scaled by each enemy type's `contact_damage`).
- Enemy roster update: added `bomber` behavior and a `high_bomber` config type for high, mostly straight flight pressure.
- Run-end update: when player HP reaches 0 during `InRun`, the game now transitions automatically to `Results` and displays a run summary with score and distance.
- Bomber behavior tuning update: bombers now fly 20% higher than the previous baseline and attack only by dropping free-falling bombs (no aimed pea-shooter fire).
- Enemy body interaction update: enemies now resolve physical body overlap against player and each other, including size/mass-weighted pushback and impulse carry-over.
- Scoring update: enemy types now have configurable `kill_score` in `enemy_types.toml`; kills award per-type score and results now show score with kill contribution breakdown.
- Vehicle refinement scope decision: add Epic B+ for modular vehicle architecture (chassis/turret/tires), rear-wheel drive, suspension, and 3D part import/alignment workflow.
- BR1 implementation detail: player vehicle is now split into modular child entities (chassis, turret body, front/rear wheel pairs) under a 2D kinematic root transform.
- BR1 visual debug detail: wheel pairs render as hexagons and rotate from vehicle linear speed so tire rotation is visibly readable.
- BR2 implementation detail: drive acceleration is now rear-wheel-contact gated (rear-wheel drive); front wheel pair is explicitly non-driven.
- Drivetrain update: drive split is now configurable via `vehicles.toml::front_drive_ratio`; starter car defaults to 30% front / 70% rear torque distribution.
- Handling cap update: airborne angular velocity cap is now configurable via `vehicles.toml::air_max_rotation_speed` and is exposed in the `V` tuning panel.
- Air-control behavior change: airborne A/D input now sets angular velocity directly (using `air_max_rotation_speed`) instead of applying rotational torque.
- Vehicle placeholder tuning detail: wheel hexagons were scaled +20% and moved downward by half a wheel radius for stronger tire readability and stance.
- BR3 implementation detail: added per-wheel spring-damper suspension state (front/rear) with config-driven rest length, stiffness, damping, and compression/extension travel limits.
- BR4 implementation detail: tire-ground contact now drives traction/slip scaling from wheel compression; terrain penetration is corrected from wheel clearance, and vehicle stability is improved while preserving existing chassis-enemy push response.
- Traction stability tuning: rear-wheel drive now includes a configurable near-ground traction assist window (default 20 cm) to reduce micro-hop traction loss; reverse torque now uses the same rear-drive contact path so forward/reverse traction is consistent.
- BR4 regression fix detail: if front wheel is grounded and rear wheel hovers near terrain, rear-drive assist now extends to a fallback assist window (0.30 m) so forward acceleration does not deadlock.
- BR4 handling tweak: removed vertical wheel/chassis snap-to-ground behavior to preserve throttle/wheelie balance nuance; slope alignment remains in place.
- Enemy damage tuning: `enemy_bomb_drop` weapon damage was doubled (9.0 -> 18.0).
- BR4 handling tuning: removed forced terrain-angle alignment entirely and reduced grounded angular damping (`0.80 -> 0.94`); added a stronger rear-drive fallback assist path (support-aware, fallback distance 0.90 m) to avoid deadlock when perched.
- BR4 suspension readability update: wheel visuals now exaggerate spring travel (render-only) so compression/extension is clearly visible; starter suspension retuned to `stiffness` 14.0, `damping` 2.0, travel `compression/extension` 0.46/0.46 with `rotational_inertia` at 1.8.
- Run lifecycle reliability fix: all `InRun` gameplay entities are now explicitly cleaned up on `OnExit(GameState::InRun)` (vehicle scene, combat visuals/projectiles/FX, enemies/projectiles), preventing immediate re-death when restarting from Results.
- Vehicle tuning tweak: starter-car `acceleration` in `vehicles.toml` was doubled (4.0 -> 8.0) to increase forward drive force.
- Physics stack migration: `bevy_rapier2d` is now integrated and active; player vehicle is a Rapier dynamic rigid body with collider/forces, terrain checker columns now include fixed box colliders, and per-wheel suspension sampling uses Rapier raycasts against fixed ground colliders.
- Migration constraint note: BR5 (3D part import/alignment) remains deferred; current focus is achieving good-feeling box-based dynamics first.
- Maintenance note: `src/gameplay/vehicle/mod.rs` has grown past 1000 LOC; split into focused submodules (input/physics/visuals/telemetry) during BR5/BR6.
- Scope decision: keep C6 audio/SFX placeholder wiring deferred for later iteration.
- Validation policy: run `gaussian_splats` feature checks only when changes touch splat/rendering integration.
- Ground pipeline decision: move terrain authoring/import workflow from Epic B to the end of Epic E.
- Commentary decision: use two commentators in round-robin order; each prompt includes what the other commentator said last; subtitles are always shown with speaker-specific colors.
- Physics direction update: bevy_rapier2d is now the active runtime physics backend for ongoing vehicle dynamics tuning.
- BR4a tuning update: chassis now uses explicit Rapier mass/inertia properties and a lowered center of mass; suspension force is applied along raycast contact normals; the old forced `velocity.y = 0` grounded clamp was removed so spring/jump behavior is driven by physics contacts.
- BR4a tuning values update: starter-car suspension/traction/drive numbers in `vehicles.toml` were retuned for Rapier (`acceleration` 14.0, `suspension_stiffness` 170.0, `suspension_damping` 44.0, `rotational_inertia` 2.4, raised traction floor/assist).
- Stability fix: player root now explicitly carries visibility inheritance components to avoid Bevy hierarchy warning `B0004` for visual children.
- Stability fix: gameplay despawn calls were switched to `try_despawn()` to silence duplicate-despawn command warnings when multiple systems target the same entity in a frame.
- BR4a suspension stability fix: corrected spring-damper sign so damping resists compression (prevents energy gain), restricted grounded hits to sufficiently upward-facing raycast normals, and applied spring support in world-up direction to avoid lateral impulse injection from checker-column side faces.
- BR4a handling pass: increased starter acceleration 3x (`14 -> 42`) as requested; raised air pitch torque and changed air-control gating to use suspension support force threshold so A/D pitch control remains responsive while airborne.
- BR4a anti-snap pass: wheel suspension now uses non-solid raycasts plus compression/rebound rate limits, and wheel visual spring length is lerped to reduce visible snapping to terrain steps.
- BR4a stability pass: strengthened grounded angular damping to reduce excessive roll while driving without suppressing airborne rotation control.
- BR4a handling adjustment: moved vehicle center of mass to the lower-edge midpoint of the chassis quad (`COM y = -0.54`), restored explicit airborne A/D torque path with only a small grounded torque factor, increased suspension damping (`44 -> 78`), and tightened rebound/compression rate limits to reduce bounce.
- Ground representation update: replaced checker-tower terrain with thick spline-strip ground segments (visual + fixed colliders) to remove jagged side-face artifacts and improve contact continuity.
- Traction safety fix: wheel suspension raycasts now use vehicle-local down direction with alignment gating, and rear-drive assist no longer grants fallback traction when no real wheel contact exists, preventing upside-down/air traction.
- HUD utility: added a camera-anchored lower-left yardstick with 5m notches and emphasized 10m marks for scale readout while tuning.
- BR4a tuning tweak: increased starter-car drive acceleration to `210.0` (5x prior) and in-air rolling torque (`air_pitch_torque`) to `180.0` (10x prior) per latest handling request.
- BR4a tuning tweak: set starter-car weight proxy (`linear_inertia`) to `8.0` (10x prior) and tuned reverse force to 60% of forward by setting `brake_strength` to `126.0` while forward `acceleration` stays `210.0`.
- Enemy physics migration update: enemy entities now use Rapier dynamic bodies/colliders with behavior-driven velocity steering; the previous custom enemy-enemy/player impulse resolver was removed to prevent extreme contact impulses.
- Debug UI input fix: vehicle tuning panel rendering moved to `EguiPrimaryContextPass`, and `bevy_egui` is pinned to a Bevy-0.17-compatible version so panel buttons/sliders are clickable again.
- AI commentary in early milestones is stub-first; real Neocortex integration is a dedicated later task.
- Neocortex request flow to use later: /api/v2/chat then /api/v2/audio/generate.
- Initial controls are keyboard A/D and Left/Right.
- Preferred narrator audio format for easiest playback: wav (fallback mp3 if needed).
- `reference/voice_api_example.txt` was repaired and can be used as the API integration reference.

---
## 7) Recommended build sequence (milestones)
This is a suggested order that keeps "always playable":

### Milestone 1 - Playable driving slice
- Epic A (minimum) + Epic B (v0) + straight ground + placeholder background.

### Milestone 2 - Vehicle refinement pass
- Epic B+ (modular vehicle parts + rear-wheel drive + suspension + 3D part assembly).

### Milestone 3 - Combat and one enemy
- Epic C (v0) + Epic D (one enemy type, simple spawner).

### Milestone 4 - Segments and progression
- Epic E (segment concatenation + seam masking) + Epic F (score/run flow).

### Milestone 5 - AI commentator is "the feature"
- Epic G end-to-end; add subtitles + debug panel to show emitted events.

### Milestone 6 - Content + polish
- Expand enemy roster, add boss, add 5-10 segment configs, upgrade variety.
- UI/audio polish (Epic H).

### Milestone 7 - Web (only if stable)
- Epic I as a final pass.

---

## 8) Risks & mitigations
### R1: Vehicle physics feels bad / unstable
- Mitigation: start with a **simple rigidbody** + ground normal + traction; postpone "true wheel suspension."
- Add clamped angular velocity and forgiving landing logic.

### R2: Splat rendering too heavy
- Mitigation: treat splats as **background-only**; keep them static; reduce point count / LOD.
- Keep a "2D fallback background" path to avoid blocking gameplay.

### R3: AI narration latency / spam / failure
- Mitigation: strict cooldowns + priority queue + cancellation of stale requests.
- Always have fallback canned VO + subtitle text.
- Never block the main thread; narration should be "best effort."

### R4: Web build audio/network issues
- Mitigation: ship desktop first; web only if it's stable.
- Require user click for audio; show clear UI.

### R5: Data-driven complexity slows iteration
- Mitigation: keep TOML minimal; allow defaulting; add a `validate` command and log errors with line context.

---

## 9) Debug & telemetry (high leverage)
- On-screen event log: last 10 `GameEvent`s with timestamps.
- "Physics debug" toggle: show ground normal, contact point, vehicle center of mass.
- Spawner debug: show upcoming spawns along distance line.
- Narration debug: show:
  - queued events -> built summary -> request status -> audio playback.

---

## 10) Suggested repo layout
```
mr_autoauto/
  assets/
    sprites/
    audio/
    splats/
    ui/
  config/
    game.toml
    segments.toml
    backgrounds.toml
    environments.toml
    vehicles.toml
    weapons.toml
    enemy_types.toml
    spawners.toml
    upgrades.toml
    commentator.toml
  src/
    main.rs
    states/
    config/
    gameplay/
      vehicle/
      combat/
      enemies/
      segments/
      scoring/
      upgrades/
    ai_commentator/
    ui/
    fx/
    audio/
    debug/
```

---

## 11) Minimal "Day 0" implementation checklist (first runnable slice)
- Spawn player quad at origin.
- Straight ground collision.
- Two-button accelerate/brake + in-air rotate.
- Auto-turret firing bullets forward.
- One enemy spawns ahead and can be killed.
- Score increments on distance + kill.
- AI commentator stub receives at least: `Kill`, `BigJump`, `Crash`, and shows debug text (network/audio wiring is deferred).

---

## Appendix: initial `GameEvent` set (good coverage, low noise)
- Movement/stunts: `SpeedTierReached(tier)`, `Airtime(duration)`, `Wheelie(duration)`, `Flip(count)`, `HardLanding(g_force)`, `Crash`
- Combat: `EnemyKilled(type)`, `MultiKill(n)`, `BossSpawned(id)`, `BossKilled(id)`, `NearDeath(hp)`
- Progress: `SegmentEntered(id)`, `UpgradeChosen(id)`, `MilestoneDistance(d)`

Use `commentator.toml` to:
- define thresholds for "big" vs "small" jumps
- rate-limit each class of callout
- define priorities (BossKill > NearDeath > BigJump > Kill)

---

*This plan is intentionally "cuttable": if something threatens stability, drop Web epic, drop missiles, drop boss phases, keep the core drive+shoot loop and the AI commentator shining.*


