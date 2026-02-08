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
- [done] BR4a. Rapier migration pass for current driving dynamics:
  - run vehicle body under `bevy_rapier2d` rigidbody/collider simulation.
  - keep box-collider terrain for tuning pass before visual polish.
  - preserve current control semantics (rear-drive traction, spring behavior, stunt telemetry) on top of Rapier.
  - tune starter values from vehicle-physics references (spring frequency/damping ratio ranges) and validate against in-game feel.
- [done] BR4b. Spline-style ground and measurement aid:
  - replace jagged tower ground with thick extruded spline-strip segments for both visuals and fixed colliders.
  - add lower-left yardstick overlay with 5m minor notches and 10m major notches.
- [in progress] BR5. 3D part asset schema and import pipeline:
  - define separate model refs for chassis/turret/tire parts and/or node-segment extraction from source model.
  - add config for attachment points/local offsets so parts mount at correct locations.
- [not started] BR6. Runtime assembly + validation:
  - assemble parts into correct hierarchy at spawn, keep transforms synchronized, and add debug checks for misalignment/scale.
- [not started] BR7. Visual migration pass:
  - replace coder-art placeholders with production part models while preserving physics/tuning behavior.
- [not started] BR8. Gameplay mesh depth/parallax pass:
  - decide/render strategy for player/enemy 3D meshes so they can have controlled depth/parallax without destabilizing the 2D gameplay readability.

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
- [in progress] D6. Boss v0:
  - [done] Segment boss trigger: when player reaches `segment_end - 20m`, spawn a boss encounter enemy for that segment.
  - [done] Added first boss archetype `segment_boss_drone` (drone-derived, larger HP/size, right-half screen behavior, spread fire).
  - [done] Boss defeat transition: killing the segment boss teleports player to the next segment start and resets local encounter flow.
  - [not started] Optional phase logic (adds/weak-spot) if needed after baseline pacing validation.

**DoD**
- Multiple enemy types appear across distance; boss encounter is possible and ends a segment cleanly.

---

### Epic E - Background segments + streaming + environment modifiers
**Goal:** splat segments are first-class gameplay segments, concatenated linearly.

**Tasks**
- [in progress] E1. Define `SegmentConfig` (asset ref, length, env id, spawn sets, music cue).
  - first splat asset hook is active via `backgrounds.toml::splat_asset_id` + `assets.toml::splats`.
  - [done] Added background placement tuning workflow (`B` debug panel): live `parallax`, `offset_x/y/z`, `scale_x/y/z`, `loop_length_m`, `ground_lowering_m` edits + apply persists to `backgrounds.toml` and `game.toml`.
  - [done] Segment naming pass: active segment IDs are now `cemetery`, `castle`, and `mythical` (with `cemetery` first).
  - [done] Runtime splat background sync now follows the currently active segment by run distance and swaps splat assets when segment changes.
  - [done] Terrain wave settings (`wave_a/b/c` amplitude/frequency) are now segment-specific via `backgrounds.toml`; all terrain sampling systems resolve values from the active segment.
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
- [done] F1. Score sources:
  - distance, kills, stunts (airtime/wheelie/flip), "no damage" bonus.
- [done] F2. Currency drops (coins/parts):
  - enemies now drop collectible gold coin circles and occasional green health crates.
  - coin pickups award score bonus; health pickups restore player HP.
- [done] F3. Upgrade selection UI:
  - coin milestone trigger implemented: every `game.toml::run_upgrades.coins_per_offer` coin pickups, an on-screen two-choice panel appears.
  - choices are drawn from `game.toml::run_upgrades.options` with stack-cap filtering for future expansion beyond the current 3-option MVP.
- [done] F4. Upgrade application system:
  - implemented runtime application for MVP upgrades: `health +10`, `gun fire rate +10%`, `missile fire rate +10%`.
  - added upgrade effects for `car power +10%`, `targeting cone +5 deg`, `missile turn speed +10%`, and `targeting range +10%`.
  - upgrade definitions are data-driven (`effect`, `value`, `max_stacks`) and update live runtime config/player state.
- [done] F5. Run end conditions:
  - health hits 0; show results screen with summary + restart.
- [not started] F6. High score persistence (local file; for web use local storage if available later).

**DoD**
- A full run has an arc: start -> escalation -> upgrades -> fail/win -> summary -> restart quickly.

---

### Epic G - AI commentator integration (Neocortex Web API)
**Goal:** narrator is reliable, rate-limited, and clearly reactive to gameplay.

**Tasks**
- [done] G1. Event model:
  - `GameEvent` enum (JumpBig, WheelieLong, Flip, Kill, BossKill, Crash, SpeedTier, NearDeath, Streak).
  - [done] Added runtime commentary triggers for `3+ enemies visible on screen`, `boss spawned`, `boss defeated`, and `player hit by bomb`.
- [done] G2. Event aggregation:
  - batch events into a compact "what happened" text summary.
  - de-duplicate spammy events; apply cooldowns and priorities.
  - route lines in round-robin order between two commentators.
- [done] G3. Prompt builder:
  - include run context (segment name, score streak, player health).
  - include what the other commentator said last time.
  - style knobs from `commentator.toml` (tone, length, profanity filter if desired).
- [done] G4. Neocortex API client:
  - async request queue is now wired through a non-blocking worker thread; chat -> audio flow is active.
  - per-commentator `character_id` and per-commentator chat `session_id` context are now used.
  - strict request timeout is now enforced via curl timeout flags.
  - stale-request timeout policy now cancels old requests and immediately falls back to dry subtitles.
  - retry/backoff policy is now active (configurable retry count and backoff delay).
- [in progress] G5. Audio decode & playback:
  - response audio is now read from generated files into memory and played through Bevy audio.
  - narration volume is now configurable in `commentator.toml`.
  - remaining: duck SFX/music under narration.
- [in progress] G6. Fallback behavior:
  - if API key is missing or API call fails, fallback dry text lines are emitted immediately so gameplay never stalls.
  - remaining: optional canned local VO playback path.
- [in progress] G7. UI subtitles:
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
- [in progress] H2. HUD: health, distance, speed, score, upgrade icons, current segment label.
  - [done] Added first-pass in-run HUD panel with score, HP bar, distance/speed, kills/coins, stunt totals, active segment, and live upgrade summary list.
  - [done] Converted old debug overlay into a compact diagnostics panel so gameplay HUD remains readable during tuning.
  - [done] Added root `README.md` with setup/run instructions, controls, config map, optional Neocortex API env setup, and credits.
- [done] H3. Hit indicators (directional damage, screen shake light):
  - edge damage indicators now flash by incoming hit direction (left/right/top/bottom).
  - light camera shake now reacts to incoming damage, enemy crashes, and hard landings.
- [done] H4. Feedback polish:
  - muzzle flash, screen shake on big hits, dust on landing, coin pickup sparkle.
  - expanded impact/death particle pass:
    - richer burst/smoke particles for projectile hits and enemy deaths.
    - enemy bomb ground impacts now spawn visible explosion/dust effects.
- [in progress] H5. Audio mix:
  - music bed loop; mix levels; narration ducking.
  - [done] Added gameplay SFX layer (engine loop + gun/missile shot/hit/miss + explosion) with configurable per-sound relative volumes in `game.toml::sfx`.
  - [done] Added randomized pitch variation for one-shot SFX and subtle runtime pitch jitter for the engine loop.
  - [done] Added looping background music (`assets/audio/music.wav`) with startup fade-in during loading/logo state and configurable `game.toml::sfx.music_volume`.
  - [done] Added in-game audio tuning debug window (`M`) with live sliders + numeric fields for key volume parameters and Apply-to-`game.toml` persistence.
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

## 6.1) Decisions log (curated)
- Scope sequencing: start implementation with Epic A and keep the game runnable at each step.
- Placeholder-first content policy: simple polygons/boxes are acceptable until real art/splats are integrated.
- Version/toolchain baseline: Bevy 0.17 + `bevy_gaussian_splatting` v6, stable Rust (`rustc/cargo 1.88.0`).
- Splat compatibility strategy: use a vendored patch-crate approach so Gaussian splats work on stable builds.
- Config architecture: core gameplay parameters are data-driven through `config/*.toml` with fail-fast cross-reference validation.
- Runtime iteration workflow: `F5` hot-reloads config safely; invalid reloads are rejected without corrupting runtime state.
- Live tuning workflow:
  - `V` vehicle tuning panel edits runtime values immediately.
  - `Apply` persists vehicle tuning back to `config/vehicles.toml`.
- Background tuning workflow:
  - `B` background tuning panel edits parallax/offset/scale/loop length plus per-segment terrain wave amplitude/frequency and terrain `ground_lowering_m` live.
  - `Apply` persists background values to `config/backgrounds.toml` and terrain lowering to `config/game.toml`.
- Units policy: gameplay, physics, HUD, and config use meters directly (no separate world-unit conversion layer).
- Physics direction: `bevy_rapier2d` is the runtime backend for player/enemy dynamics.
- Vehicle roadmap decision: Epic B+ introduces modular vehicle parts (chassis/turret/wheels), suspension, drivetrain tuning, and 3D model integration.
- Drivetrain/control decisions:
  - configurable front/rear drive split (default 30/70).
  - airborne A/D control uses direct angular velocity target (`air_max_rotation_speed`) instead of torque.
- Ground pipeline decision: move formal ground authoring/import workflow to end of Epic E.
- Terrain representation decision:
  - replaced jagged tower ground with spline-strip terrain.
  - render continuous strip + curtain meshes; physics uses a single strip-top line-strip/polyline collider (no per-segment collider seams).
- Combat readability decisions:
  - always render blue target line + green cone boundaries.
  - target cone defaults to 60 degrees and remains configurable/upgradable.
- Commentary design decision:
  - gameplay emits dry factual events; style/color comes from LLM output.
  - two commentators speak in round-robin.
  - each prompt includes the other commentator's previous line.
  - subtitles are always shown and color-coded by speaker.
- Commentary integration strategy:
  - build/use stub pipeline first.
  - real Neocortex path uses chat first, then audio generation.
  - per-commentator character ID + persistent session ID contexts.
  - strict fallback path ensures gameplay never blocks on API failures.
  - `commentator.toml` has `api_enabled` switch for cost control.
- Prompt policy:
  - removed rigid `OUTPUT_RULES` boilerplate.
  - style is controlled per commentator in config.
- Audio robustness decision: support Neocortex WAV playback with header normalization for non-standard RIFF/data sizes.
- Run flow decisions:
  - HP zero transitions to `Results`.
  - scoring uses distance + kills + stunts + bonuses from TOML.
  - upgrade offers trigger by coin milestones and pause gameplay for selection.
- Segment pacing decision:
  - segment lengths set to `cemetery=768m`, `castle=512m`, `mythical=1024m`.
  - boss encounter triggers 20m before segment end.
  - while the segment boss is alive (or portal is pending), player progression is gated at the boss line so the run stays in the current segment.
  - defeating the boss advances/teleports run to the next segment start.
- Segment portal loading decision:
  - boss defeat now opens a loading overlay and waits for the next segment background asset readiness before completing the portal.
- Boss camera gate decision:
  - while a segment boss is alive, gameplay camera x is clamped to not pan past the boss.
- Restart flow decision:
  - `Space` from Results returns to `Loading` and reuses the startup asset-readiness gate before entering `InRun`.
- Upgrade UX decision: two random choices, selected with left/right controls, requiring a fresh keypress after panel opens.
- Stability policy:
  - prefer disjoint queries/`Without`/combined-query patterns to avoid Bevy `B0001` conflicts.
  - use `try_despawn()` for resilience against duplicate-despawn command races.
- Rendering/dev policy:
  - loading state should wait for critical assets to load before entering `InRun`.
  - isolate splat background rendering on a dedicated render layer to prevent gameplay meshes (for example 3D vehicle model) from being rendered by the splat camera.
  - keep a dedicated gameplay `Camera3d` for vehicle models, synchronized to the main `Camera2d`, so 3D gameplay meshes remain visible after splat-layer isolation.
  - `bevy_gaussian_splatting` sort system expects non-negative camera order; do not use negative `Camera::order` on Gaussian cameras.
  - on Windows, default WGPU backend to DX12 (unless overridden via `WGPU_BACKEND`) to improve compatibility with external game-capture tools.
  - lowered vendored Gaussian radix-sort shader workgroup pressure (4-bit digits) to avoid compute pipeline validation failures on adapters with `max_compute_invocations_per_workgroup < 1024` (for example 768).
- Maintenance decisions:
  - split oversized files during follow-up refactors (`vehicle` and `commentary_stub` are both past the preferred size threshold).
  - completed first `vehicle` split pass: `src/gameplay/vehicle/mod.rs` now coordinates constants/types/plugin only; systems moved into `scene.rs`, `model.rs`, `runtime.rs`, and `terrain.rs`.
- BR5 status decision: 3D vehicle integration is active.
- BR5 current workflow:
  - active model is `assets/models/vehicles/car_rally.glb#Scene0`.
  - `N` dumps loaded scene node names/transforms for mapping.
  - runtime fit derives orientation/scale from wheel/chassis/turret geometry as iterative bootstrap.
  - wheel/turret node animation uses local-pivot compensation (instead of origin-only rotation) and wheel visual size is derived from measured wheel mesh bounds toward physics tire radius.
  - wheel-node runtime placement now pins each wheel pivot to the corresponding wheel-pair (physics visual proxy) world position each frame for exact suspension/position tracking.
- Enemy model visual-fit pass:
  - hooked `owl_tower.glb` and `owl_bomber.glb` via `assets.toml` model entries.
  - enemy model setup now uses a simplified bounds fit (auto scale + center offset) against existing gameplay body size, keeping collider/hitbox logic unchanged.
  - hooked `beetle_rough.glb` for walker, `beetle_green.glb` for charger, and `bullfinch.glb` for flier via enemy-specific model IDs.
- Terrain/sample consistency note:
  - all gameplay systems that sample terrain height (vehicle, combat/projectile impacts, enemies, pickups) now apply `game.toml::terrain.ground_lowering_m` consistently.
  - terrain wave sampling now resolves per active segment (`backgrounds.toml` overrides with fallback to `game.toml::terrain`) for vehicle, enemies, combat impacts, pickups, and terrain mesh generation.
- Ground-follow behavior note:
  - walker/charger follow terrain tangent and use tuned uphill movement to handle slopes reliably.
  - bomber uses a long-period sine-wave cruise path.
  - enemy projectile firing is constrained to forward/up sectors; backward shots are clamped out.
- HUD polish note:
  - introduced a dedicated gameplay HUD overlay (`src/ui/mod.rs`) separate from debug diagnostics to keep play info readable while preserving tuning data.
- Camera readability note:
  - gameplay camera now uses smoothed speed-based zoom (closer view at low speed, wider view at high speed).
  - keep gameplay and model cameras projection-synced during zoom to prevent visual-vs-physics drift.
  - turret visuals now use a light smoothing filter, while targeting line and projectile firing remain immediate/off the unsmoothed aim direction.
- Feedback polish note:
  - added a dedicated gameplay feedback layer (`src/gameplay/feedback/mod.rs`) for directional damage indicators, lightweight camera shake, landing dust particles, and pickup sparkle particles.
  - feedback layer now also renders richer hit/death particles from combat and enemy projectile impact events, including bomb-on-ground impacts.
  - extended runtime events with world positions (`PlayerDamageEvent`, `PickupCollectedEvent`, `VehicleLandingEvent`) so UI/FX can react contextually.
- Audio mix note:
  - gameplay SFX now uses data-driven mix values under `game.toml::sfx` (master, per-sound relative volume, pitch random range, engine loop response/jitter).
  - WAV-only audio pipeline for runtime playback: removed Bevy `mp3` decoding feature and now reject non-WAV Neocortex narration payloads to avoid demuxer false-positive crashes.
  - SFX runtime loader now reads WAV bytes directly and normalizes malformed PCM `fmt` header fields (e.g., incorrect mono `byte_rate`) before playback to prevent Bevy decode panics.

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



