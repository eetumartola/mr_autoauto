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
- rate limiting: min seconds between voice lines, priority rules
- template prompts + "style" settings
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
- [not started] A2. TOML loader + schema structs; validate references (enemy IDs, weapon IDs, env IDs).
- [not started] A3. Hot-reload (optional but high value): re-read TOML on keypress.
- [not started] A4. Basic asset registry (sprites, placeholder polygons/boxes, audio placeholders).
- [not started] A5. Minimal debug overlay (FPS, distance, active segment, enemy count).
- [not started] A6. Commentary stub pipeline (event queue + debug text output, no network).

**DoD**
- Running build loads configs, spawns player + a background segment, no panics, shows HUD/debug.

---

### Epic B - Vehicle controller (Hill Climb core)
**Goal:** two-button driving feels "good enough" quickly; physics is stable.

**Tasks**
- [not started] B1. Input mapping (keyboard + gamepad optional; touch overlay for web optional).
- [not started] B2. Vehicle kinematics v0:
  - integrate velocity, gravity, clamp, basic ground collision with flat ground
  - grounded vs airborne state
- [not started] B3. In-air rotation controls (CCW/CW based on accel/brake).
- [not started] B4. Camera follow (look-ahead based on speed; fixed z).
- [not started] B5. Terrain v1 placeholder: simple height function (sine/ramps) from config.
- [not started] B6. Stunt metrics:
  - airtime, wheelie timer, flip detection, max speed, crash detection.

**DoD**
- You can drive, jump, rotate in air, land without jitter; stunt metrics are tracked.

---

### Epic C - Combat: turret, bullets/missiles, hits
**Goal:** Mr Autofire-style "auto shoots and feels punchy."

**Tasks**
- [not started] C1. Turret targeting:
  - select nearest enemy within cone/range; configurable prioritization (nearest/strongest).
- [not started] C2. Firing logic:
  - fire rate, burst/spread, projectile spawn offsets.
- [not started] C3. Projectile simulation:
  - bullet: straight + optional drag
  - missile: ballistic + optional homing (bounded turn rate)
- [not started] C4. Collision & damage:
  - simple circle/box overlap (2D), friendly-fire rules, hitstop optional.
- [not started] C5. Effects v0:
  - tracer sprite, impact sprite, enemy hit flash, simple explosion quad
- [not started] C6. Audio SFX placeholders (gun, hit, explosion) with volume ducking under narration.

**DoD**
- Shooting reliably hits enemies, feedback is readable, and performance stays stable with moderate projectile counts.

---

### Epic D - Enemies & spawners (content without code edits)
**Goal:** enemies spawn from data and create pressure.

**Tasks**
- [not started] D1. Enemy "quad" renderer + hitbox.
- [not started] D2. Enemy behaviors v0 (config-driven):
  - Walker (ground), Flier (sine hover), Turret (stationary shooter), Charger.
- [not started] D3. Enemy shooting patterns (simple): aimed shots, arcs, spreads.
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

**DoD**
- You can traverse multiple segments seamlessly; environment changes are noticeable and stable.

---

### Epic F - Scoring, coins, upgrades, and run flow
**Goal:** simple progression that makes replay meaningful.

**Tasks**
- [not started] F1. Score sources:
  - distance, kills, stunts (airtime/wheelie/flip), "no damage" bonus.
- [not started] F2. Currency drops (coins/parts).
- [not started] F3. Upgrade selection UI:
  - after boss / at checkpoints / on level-up, present 2-3 choices.
- [not started] F4. Upgrade application system:
  - modify weapon/vehicle params; stack rules.
- [not started] F5. Run end conditions:
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
- [not started] G3. Prompt builder:
  - include run context (segment name, score streak, player health).
  - style knobs from `commentator.toml` (tone, length, profanity filter if desired).
- [not started] G4. Neocortex API client:
  - async request queue; cancellation (don't narrate stale moments).
  - retries with backoff; strict timeout.
- [not started] G5. Audio decode & playback:
  - store response to file/memory; feed to Bevy audio.
  - duck SFX/music under narration.
- [not started] G6. Fallback behavior:
  - if API fails/offline, play local canned VO lines or text captions.
- [not started] G7. UI subtitle (optional but nice):
  - show last line as captions; helps in noisy demo spaces and web autoplay restrictions.

**DoD**
- Narrator reacts to stunts/kills/crashes with minimal latency and without spamming; game never stalls on API.

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
- Physics direction for later epics: bevy_rapier2d.
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

### Milestone 2 - Combat and one enemy
- Epic C (v0) + Epic D (one enemy type, simple spawner).

### Milestone 3 - Segments and progression
- Epic E (segment concatenation + seam masking) + Epic F (score/run flow).

### Milestone 4 - AI commentator is "the feature"
- Epic G end-to-end; add subtitles + debug panel to show emitted events.

### Milestone 5 - Content + polish
- Expand enemy roster, add boss, add 5-10 segment configs, upgrade variety.
- UI/audio polish (Epic H).

### Milestone 6 - Web (only if stable)
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


