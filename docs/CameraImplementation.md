## Recommended design target

For your engine, treat the third-person camera as a **gameplay camera controller**, not as a camera entity directly parented to the character.

Use four conceptual points:

```text
Player root
    ↓
Follow anchor       Smoothly follows the player
    ↓
Orbit pivot         Yaw and pitch rotate around this point
    ↓
Camera boom         Desired camera distance
    ↓
Camera              Collision-adjusted final position
```

The camera should follow an upper-body point rather than the player’s feet:

```text
player position
+ standing eye/shoulder offset
+ optional framing offset
```

This builds naturally on your engine’s existing “camera as a smooth-tracking entity” concept, while adapting it to a fully orbital third-person view. 

The implementation below targets **Bevy 0.18**, released January 13, 2026. Bevy’s current official orbit example uses accumulated mouse motion with separate yaw and pitch values and clamps pitch to avoid rotation through the poles. ([bevy.org][1])

---

# 1. MMO-style input behavior

A polished MMO camera usually has several related control modes.

## No mouse button held

* Mouse cursor is visible.
* Camera retains its orientation.
* `WASD` moves relative to camera heading.
* The character turns toward the actual movement direction.
* UI remains interactive.

## Left mouse button held: free look

* Mouse rotates the camera around the character.
* Character facing does not change.
* Releasing the button leaves the character facing where it was.
* Useful for looking behind the player while continuing forward.

## Right mouse button held: steering mode

* Mouse rotates the camera.
* Character facing follows camera yaw.
* `W` moves forward in the camera/character facing direction.
* Cursor is hidden and locked.
* This should feel like direct third-person action control.

## Both mouse buttons held

Classic MMO behavior:

* Character moves forward.
* Mouse steers character and camera.
* Equivalent to holding `W` plus right-mouse steering.

Make this configurable; some players dislike automatic two-button movement.

## Optional bindings

```text
Mouse wheel          Zoom
Home                 Reset camera
Num Lock / middle    Autorun
Q / E                Strafe or turn, selectable
Alt                   Temporary free-look
Escape                Release cursor
```

I recommend making `Q/E` strafing by default. Keyboard turning feels dated once camera-relative movement and mouse steering work properly.

---

# 2. Separate camera intent from camera transform

Do not store the camera’s entire state in `Transform`.

Use an explicit component:

```rust
use bevy::prelude::*;
use std::f32::consts::PI;

#[derive(Component, Debug)]
pub struct MmoCamera {
    pub target: Entity,

    // Desired orbit state.
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,

    // Smoothed state.
    pub current_yaw: f32,
    pub current_pitch: f32,
    pub current_distance: f32,
    pub current_focus: Vec3,

    // Framing.
    pub focus_height: f32,
    pub shoulder_offset: f32,

    // Input configuration.
    pub mouse_sensitivity: Vec2,
    pub zoom_speed: f32,
    pub invert_y: bool,

    // Limits.
    pub min_pitch: f32,
    pub max_pitch: f32,
    pub min_distance: f32,
    pub max_distance: f32,

    // Smoothing frequencies.
    pub rotation_sharpness: f32,
    pub follow_sharpness: f32,
    pub zoom_sharpness: f32,

    // Collision.
    pub collision_radius: f32,
    pub collision_margin: f32,
    pub collision_return_sharpness: f32,
}

impl Default for MmoCamera {
    fn default() -> Self {
        Self {
            target: Entity::PLACEHOLDER,

            yaw: 0.0,
            pitch: -0.28,
            distance: 7.0,

            current_yaw: 0.0,
            current_pitch: -0.28,
            current_distance: 7.0,
            current_focus: Vec3::ZERO,

            focus_height: 1.55,
            shoulder_offset: 0.0,

            mouse_sensitivity: Vec2::new(0.0035, 0.0030),
            zoom_speed: 1.2,
            invert_y: false,

            min_pitch: -1.30,
            max_pitch: 1.10,
            min_distance: 1.5,
            max_distance: 14.0,

            rotation_sharpness: 24.0,
            follow_sharpness: 18.0,
            zoom_sharpness: 20.0,

            collision_radius: 0.25,
            collision_margin: 0.10,
            collision_return_sharpness: 8.0,
        }
    }
}
```

Keep desired and current values separate. This lets input update the desired values instantly while the rendered camera converges smoothly.

---

# 3. Use frame-rate-independent smoothing

Avoid ordinary fixed-factor lerping:

```rust
current = current.lerp(target, 0.1);
```

That behaves differently at different frame rates.

Use exponential smoothing:

```rust
#[inline]
fn exp_smoothing_factor(sharpness: f32, delta_seconds: f32) -> f32 {
    1.0 - (-sharpness * delta_seconds).exp()
}

#[inline]
fn smooth_vec3(
    current: Vec3,
    target: Vec3,
    sharpness: f32,
    delta_seconds: f32,
) -> Vec3 {
    current.lerp(
        target,
        exp_smoothing_factor(sharpness, delta_seconds),
    )
}

#[inline]
fn smooth_scalar(
    current: f32,
    target: f32,
    sharpness: f32,
    delta_seconds: f32,
) -> f32 {
    current + (target - current)
        * exp_smoothing_factor(sharpness, delta_seconds)
}
```

For angles, use shortest-path interpolation:

```rust
#[inline]
fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(2.0 * PI) - PI
}

#[inline]
fn smooth_angle(
    current: f32,
    target: f32,
    sharpness: f32,
    delta_seconds: f32,
) -> f32 {
    let difference = wrap_angle(target - current);

    current
        + difference * exp_smoothing_factor(
            sharpness,
            delta_seconds,
        )
}
```

This prevents the camera from spinning the long way around when yaw crosses `-π/π`.

---

# 4. Camera mode state

Use one resource to describe the current mouse-control mode:

```rust
#[derive(Resource, Debug, Default)]
pub struct CameraInputState {
    pub left_look: bool,
    pub right_steer: bool,
    pub cursor_captured: bool,
    pub autorun: bool,
}

impl CameraInputState {
    pub fn rotating_camera(&self) -> bool {
        self.left_look || self.right_steer
    }

    pub fn steering_character(&self) -> bool {
        self.right_steer
    }

    pub fn two_button_forward(&self) -> bool {
        self.left_look && self.right_steer
    }
}
```

Input capture should be disabled when:

* a menu is open,
* a text field has focus,
* the window loses focus,
* the player presses Escape,
* the game enters a cinematic or dialogue mode.

Bevy 0.18 exposes cursor visibility and grab behavior through the window’s `CursorOptions`. On Windows, `CursorGrabMode::Locked` is appropriate for captured camera input. ([Docs.rs][2])

```rust
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

fn update_cursor_capture(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input_state: ResMut<CameraInputState>,
    mut windows: Query<
        &mut CursorOptions,
        With<PrimaryWindow>,
    >,
) {
    input_state.left_look = mouse.pressed(MouseButton::Left);
    input_state.right_steer = mouse.pressed(MouseButton::Right);

    if keyboard.just_pressed(KeyCode::Escape) {
        input_state.left_look = false;
        input_state.right_steer = false;
        input_state.cursor_captured = false;
    } else {
        input_state.cursor_captured =
            input_state.rotating_camera();
    }

    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };

    cursor.visible = !input_state.cursor_captured;
    cursor.grab_mode = if input_state.cursor_captured {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
}
```

In production, add UI-focus and window-focus conditions rather than using mouse state alone.

---

# 5. Orbit input

Bevy’s current input API provides `AccumulatedMouseMotion`, which aggregates raw mouse movement for the frame. Raw mouse delta should not normally be multiplied by frame delta; it already represents motion accumulated during that frame. The official camera orbit example follows this approach. ([Docs.rs][3])

```rust
use bevy::input::mouse::{
    AccumulatedMouseMotion,
    AccumulatedMouseScroll,
};

fn read_camera_input(
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    input_state: Res<CameraInputState>,
    mut cameras: Query<&mut MmoCamera>,
) {
    if let Ok(mut camera) = cameras.single_mut() {
        if input_state.rotating_camera() {
            let delta = mouse_motion.delta;

            camera.yaw -= delta.x * camera.mouse_sensitivity.x;

            let pitch_sign = if camera.invert_y {
                -1.0
            } else {
                1.0
            };

            camera.pitch -=
                delta.y * camera.mouse_sensitivity.y * pitch_sign;

            camera.pitch = camera.pitch.clamp(
                camera.min_pitch,
                camera.max_pitch,
            );

            camera.yaw = wrap_angle(camera.yaw);
        }

        camera.distance = (
            camera.distance
                - mouse_scroll.delta.y * camera.zoom_speed
        )
            .clamp(
                camera.min_distance,
                camera.max_distance,
            );
    }
}
```

You may want different zoom scaling for pixel-based trackpad input and line-based mouse-wheel input, but the accumulator is sufficient for the initial slice.

---

# 6. Calculate the orbital transform

Derive the camera basis from yaw and pitch:

```rust
fn orbit_rotation(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y(yaw)
        * Quat::from_rotation_x(pitch)
}
```

Then calculate the desired camera position:

```rust
fn desired_camera_position(
    focus: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
    shoulder_offset: f32,
) -> Vec3 {
    let rotation = orbit_rotation(yaw, pitch);

    let backward = rotation * Vec3::Z;
    let right = rotation * Vec3::X;

    focus
        + backward * distance
        + right * shoulder_offset
}
```

In Bevy, a camera looks along local `-Z`, so placing it along rotated positive `Z` and then calling `looking_at` produces the expected view.

---

# 7. Follow-anchor smoothing

The focus point should follow the rendered/interpolated character transform:

```rust
#[derive(Component)]
pub struct CameraFollowTarget;
```

```rust
fn update_camera_focus(
    time: Res<Time>,
    targets: Query<&GlobalTransform, With<CameraFollowTarget>>,
    mut cameras: Query<&mut MmoCamera>,
) {
    let dt = time.delta_secs();

    for mut camera in &mut cameras {
        let Ok(target_transform) = targets.get(camera.target) else {
            continue;
        };

        let desired_focus =
            target_transform.translation()
                + Vec3::Y * camera.focus_height;

        camera.current_focus = smooth_vec3(
            camera.current_focus,
            desired_focus,
            camera.follow_sharpness,
            dt,
        );

        camera.current_yaw = smooth_angle(
            camera.current_yaw,
            camera.yaw,
            camera.rotation_sharpness,
            dt,
        );

        camera.current_pitch = smooth_scalar(
            camera.current_pitch,
            camera.pitch,
            camera.rotation_sharpness,
            dt,
        );
    }
}
```

For your voxel terrain, follow smoothing should be fairly responsive. Excessive lag makes climbing slopes and stepping over voxel edges feel floaty.

Good starting values:

```text
Follow sharpness:    16–24
Rotation sharpness:  20–30
Zoom sharpness:      14–22
```

---

# 8. Camera collision

A ray cast is not enough for a polished camera. It treats the camera as a point, allowing its near plane or sides to clip through walls.

Use a **sphere cast** from the focus point toward the desired camera position:

```text
focus point
    ↓ sphere sweep
desired camera position
```

Rapier describes shape casting as a sweep test and exposes it through `RapierContext::cast_shape`. It can limit travel distance and exclude selected colliders. ([rapier.rs][4])

The algorithm should be:

```rust
let desired_offset = desired_position - focus;
let desired_distance = desired_offset.length();
let direction = desired_offset.normalize_or_zero();

let allowed_distance = camera_collision_query(
    focus,
    direction,
    desired_distance,
    camera.collision_radius,
    camera.target,
);

let collision_distance = (
    allowed_distance - camera.collision_margin
).max(camera.min_distance);
```

Then apply asymmetric smoothing:

* **Move inward immediately or very quickly** when an obstruction appears.
* **Move outward slowly** when the obstruction disappears.

```rust
fn resolve_distance(
    current_distance: f32,
    desired_distance: f32,
    collision_distance: f32,
    zoom_sharpness: f32,
    return_sharpness: f32,
    dt: f32,
) -> f32 {
    let target = desired_distance.min(collision_distance);

    let sharpness = if target < current_distance {
        40.0
    } else {
        return_sharpness.min(zoom_sharpness)
    };

    smooth_scalar(
        current_distance,
        target,
        sharpness,
        dt,
    )
}
```

This prevents:

* clipping through cave walls,
* seeing through hills,
* jittering beside narrow voxel passages,
* the camera snapping far backward immediately after leaving a doorway.

## Collision filters

Exclude:

* the player’s collider,
* equipped items,
* hair and cosmetic attachments,
* particles,
* triggers and sensors,
* foliage that should fade instead,
* water volumes.

Include:

* terrain,
* structures,
* large props,
* closed doors,
* other solid world geometry.

For your voxel world, avoid one collider per voxel. Query against chunk-level triangle or compound colliders.

---

# 9. Final camera transform

After collision resolution:

```rust
fn apply_camera_transform(
    time: Res<Time>,
    mut cameras: Query<(&mut Transform, &mut MmoCamera)>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut camera) in &mut cameras {
        camera.current_distance = smooth_scalar(
            camera.current_distance,
            camera.distance,
            camera.zoom_sharpness,
            dt,
        );

        let position = desired_camera_position(
            camera.current_focus,
            camera.current_yaw,
            camera.current_pitch,
            camera.current_distance,
            camera.shoulder_offset,
        );

        transform.translation = position;
        transform.look_at(camera.current_focus, Vec3::Y);
    }
}
```

In the finished version, feed `collision_distance` into this system rather than always using `camera.distance`.

---

# 10. Camera-relative WASD movement

Flatten camera forward onto the world’s horizontal plane:

```rust
fn camera_planar_basis(yaw: f32) -> (Vec3, Vec3) {
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);

    (
        forward.normalize_or_zero(),
        right.normalize_or_zero(),
    )
}
```

Alternatively, derive from the camera transform:

```rust
let forward = transform.forward().as_vec3();
let planar_forward =
    Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();

let planar_right =
    Vec3::new(-planar_forward.z, 0.0, planar_forward.x);
```

Build movement intent:

```rust
#[derive(Component, Default)]
pub struct PlayerMoveIntent {
    pub direction: Vec3,
    pub sprinting: bool,
    pub jumping: bool,
}

fn gather_player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    camera_state: Res<CameraInputState>,
    cameras: Query<&MmoCamera>,
    mut players: Query<&mut PlayerMoveIntent>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };

    let mut axis = Vec2::ZERO;

    if keyboard.pressed(KeyCode::KeyW) {
        axis.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        axis.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        axis.x += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        axis.x -= 1.0;
    }

    if camera_state.two_button_forward()
        || camera_state.autorun
    {
        axis.y = axis.y.max(1.0);
    }

    axis = axis.clamp_length_max(1.0);

    let (forward, right) = camera_planar_basis(camera.yaw);
    let world_direction =
        (forward * axis.y + right * axis.x).normalize_or_zero();

    for mut intent in &mut players {
        intent.direction = world_direction;
        intent.sprinting =
            keyboard.pressed(KeyCode::ShiftLeft);
        intent.jumping =
            keyboard.just_pressed(KeyCode::Space);
    }
}
```

Use camera **intent yaw**, rather than its delayed smoothed transform, for movement. Otherwise, smoothing causes visible input lag: the player presses forward, but movement uses yesterday’s camera direction.

---

# 11. Character rotation behavior

There are two useful rules.

## Normal camera-relative movement

When the player supplies movement input, rotate the character toward the movement vector:

```rust
fn facing_from_direction(direction: Vec3) -> f32 {
    direction.x.atan2(direction.z)
}
```

Because model-forward axes vary, you may need:

```rust
let yaw = direction.x.atan2(direction.z) + PI;
```

## Right-mouse steering

When right mouse is held, character yaw should approach camera yaw even when stationary:

```rust
desired_character_yaw = camera.yaw;
```

This makes pressing `W` immediately move forward without the character first turning from an unrelated facing direction.

Use smooth rotation:

```rust
#[derive(Component)]
pub struct CharacterFacing {
    pub yaw: f32,
    pub desired_yaw: f32,
    pub turn_speed: f32,
}

fn update_character_facing(
    time: Res<Time>,
    camera_state: Res<CameraInputState>,
    camera: Query<&MmoCamera>,
    mut players: Query<(
        &PlayerMoveIntent,
        &mut CharacterFacing,
        &mut Transform,
    )>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };

    for (intent, mut facing, mut transform) in &mut players {
        if camera_state.steering_character() {
            facing.desired_yaw = camera.yaw;
        } else if intent.direction.length_squared() > 0.001 {
            facing.desired_yaw =
                intent.direction.x.atan2(intent.direction.z);
        }

        facing.yaw = smooth_angle(
            facing.yaw,
            facing.desired_yaw,
            facing.turn_speed,
            time.delta_secs(),
        );

        transform.rotation = Quat::from_rotation_y(facing.yaw);
    }
}
```

For animation quality, distinguish:

```text
Character facing
Movement direction
Camera direction
Aim direction
```

Do not collapse them into one value. Strafing, backward movement, lock-on, casting and combat animation will need them separately.

---

# 12. Fixed simulation versus rendered camera

Your player movement and collision should run in `FixedUpdate`. Camera input and camera rendering should update every display frame.

Bevy’s official fixed-timestep example specifically notes that cameras should update as frequently as possible; it rotates the camera before fixed simulation when camera orientation controls movement, and updates camera translation after simulation using an interpolated player transform. ([bevy.org][5])

Recommended schedule:

```text
RunFixedMainLoop / BeforeFixedMainLoop
    Read camera mouse input
    Update desired camera yaw/pitch
    Gather movement intent

FixedUpdate
    Character controller
    Gravity
    Ground detection
    Movement collision
    Character facing simulation

RunFixedMainLoop / AfterFixedMainLoop
    Interpolate rendered player transform
    Smooth camera focus
    Run camera collision query
    Apply final camera transform
```

This avoids:

* input feeling one physics tick late,
* a choppy 60 Hz camera on a 144 Hz display,
* camera vibration caused by following unsmoothed physics positions,
* movement direction being based on stale yaw.

---

# 13. Ground and slope considerations

Because your terrain includes slopes, caves and overhangs, movement should be based on the character controller’s ground plane rather than blindly applying horizontal translation.

The movement pipeline should be:

```text
Camera-relative desired direction
        ↓
Character controller evaluates ground
        ↓
Project movement along traversable slope
        ↓
Apply acceleration and speed
        ↓
Resolve collision
        ↓
Snap to ground when appropriate
```

Camera code should not decide whether a slope is walkable. It only produces intent.

For steep terrain:

* keep camera forward projected against global `Y`, not the current ground normal;
* let the character controller adjust for slope;
* do not tilt the orbital horizon to match the ground;
* apply only small camera roll effects, if any.

A stable horizon is particularly important in irregular voxel terrain.

---

# 14. Near-wall and indoor behavior

Collision alone is not enough inside caves and buildings.

Add these policies:

## Minimum pitch changes at close distance

As the camera gets very close to the player, extreme downward pitch becomes uncomfortable. Blend toward a safer pitch:

```text
distance > 3.0: full pitch range
distance 1.5–3.0: gradually reduce downward pitch
```

## Character fade

When the camera is inside roughly one body radius:

```text
fade player mesh
fade hair and head first
hide head in near-first-person range
```

Do not instantly switch the entire character off.

## Obstacle fade

For foliage or small decorative geometry between camera and character:

* fade the obstruction,
* do not push the camera inward.

Reserve camera collision for genuinely solid objects.

## First-person threshold

You can optionally treat very close zoom as a first-person mode:

```text
distance <= 0.6
→ hide head
→ move focus toward eyes
→ narrow collision radius
→ adjust near clip
```

For the vertical slice, stop at approximately `1.5–2.0` world units instead.

---

# 15. Data-driven YAML settings

Since the rest of your engine is data-driven, camera feel should also be externalized:

```yaml
schema_version: 1
id: camera.third_person_mmo

orbit:
  default_distance: 7.0
  minimum_distance: 1.5
  maximum_distance: 14.0

  default_pitch_degrees: -16.0
  minimum_pitch_degrees: -74.0
  maximum_pitch_degrees: 63.0

  mouse_sensitivity_x: 0.0035
  mouse_sensitivity_y: 0.0030
  invert_y: false

follow:
  focus_height: 1.55
  shoulder_offset: 0.0
  follow_sharpness: 18.0
  rotation_sharpness: 24.0
  zoom_sharpness: 20.0

collision:
  radius: 0.25
  margin: 0.10
  inward_sharpness: 40.0
  outward_sharpness: 8.0

controls:
  left_mouse_free_look: true
  right_mouse_steers_character: true
  both_buttons_move_forward: true
  camera_relative_movement: true
  q_e_behavior: strafe
  autorun_key: num_lock

framing:
  dynamic_shoulders: false
  close_character_fade_start: 1.8
  close_character_fade_end: 0.8
```

Convert degrees to radians during validation/loading, not repeatedly inside camera systems.

---

# 16. Plugin organization

```rust
pub struct ThirdPersonCameraPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CameraSystemSet {
    Input,
    MovementIntent,
    Follow,
    Collision,
    Transform,
}

impl Plugin for ThirdPersonCameraPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CameraInputState>()
            .configure_sets(
                Update,
                (
                    CameraSystemSet::Input,
                    CameraSystemSet::MovementIntent,
                    CameraSystemSet::Follow,
                    CameraSystemSet::Collision,
                    CameraSystemSet::Transform,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    update_cursor_capture,
                    read_camera_input,
                )
                    .chain()
                    .in_set(CameraSystemSet::Input),
            )
            .add_systems(
                Update,
                gather_player_movement
                    .in_set(CameraSystemSet::MovementIntent),
            )
            .add_systems(
                Update,
                update_camera_focus
                    .in_set(CameraSystemSet::Follow),
            )
            .add_systems(
                Update,
                apply_camera_transform
                    .in_set(CameraSystemSet::Transform),
            );
    }
}
```

For the finished version, move the input and follow systems into the fixed-main-loop ordering described earlier.

Recommended module layout:

```text
camera/
├── mod.rs
├── components.rs
├── config.rs
├── input.rs
├── orbit.rs
├── follow.rs
├── collision.rs
├── framing.rs
├── cursor.rs
├── shake.rs
└── debug.rs
```

---

# 17. Camera shake and impulses

Do not directly change orbital yaw, pitch or distance for shake.

Keep a separate additive layer:

```rust
#[derive(Component, Default)]
pub struct CameraImpulse {
    pub positional_offset: Vec3,
    pub rotational_offset: Vec3,
}
```

Final composition:

```text
Base orbital transform
+ collision correction
+ shoulder framing
+ camera impulse
+ cinematic override
```

This prevents an explosion from permanently modifying the player’s preferred camera angle.

Your event-driven architecture can send events such as:

```text
CameraImpulse {
    source,
    magnitude,
    falloff,
    duration,
    frequency
}
```

The camera then reacts to nearby impacts without becoming part of physical gameplay state.

---

# 18. Important edge cases

Test these explicitly:

1. **Yaw crosses ±180°**
   Camera and character must not rotate 360° unexpectedly.

2. **Camera starts inside terrain**
   Resolve penetration or temporarily snap to the focus point.

3. **Target teleports**
   Snap or greatly increase follow sharpness above a distance threshold.

4. **Player enters a cave opening**
   Camera should contract smoothly before hitting the ceiling.

5. **Camera passes a chunk boundary**
   Collision must query loaded neighboring chunk colliders.

6. **Target despawns**
   Camera should disable cleanly rather than panic.

7. **Window loses focus during right-click**
   Release cursor capture.

8. **UI opens while mouse is captured**
   Restore the cursor and suppress gameplay input.

9. **Very low frame rate**
   Clamp visual-frame delta used by smoothing, for example to `0.05`.

10. **Character backs toward a wall**
    Camera should contract without forcing character movement.

11. **Rapid zoom while obstructed**
    Desired zoom and collision-constrained zoom must remain separate.

12. **Player stands under a low ceiling**
    Sphere cast should prevent near-plane clipping above the character.

---

# 19. Debug visualization

Add a camera debug mode displaying:

```text
Green sphere:  desired focus point
Blue line:     desired camera boom
Red sphere:    collision hit
Yellow sphere: final camera position
White arrow:   camera-relative movement forward
Purple arrow:  character facing
```

Also display:

```text
Desired yaw/pitch/distance
Current yaw/pitch/distance
Collision-limited distance
Input mode
Target entity
Camera obstruction entity
```

This will make “camera feels wrong” problems diagnosable instead of subjective.

---

# 20. Recommended implementation order

## Stage 1: basic orbit

* Follow target.
* Left/right mouse orbit.
* Pitch clamp.
* Scroll zoom.
* Camera-relative WASD.
* Character turns toward movement.

## Stage 2: MMO control modes

* Left-button free look.
* Right-button steering.
* Two-button forward.
* Cursor capture.
* Autorun.
* UI input suppression.

## Stage 3: camera collision

* Ray cast prototype.
* Replace with sphere cast.
* Ignore player collider.
* Fast contraction and slow restoration.
* Low-ceiling tests.

## Stage 4: simulation integration

* Fixed-update player movement.
* Interpolated visual character transform.
* Camera input before fixed simulation.
* Camera position after fixed simulation.

## Stage 5: polish

* Character fade.
* Foliage fading.
* Shoulder offset.
* Camera recenter.
* Camera impulse/shake.
* Controller right-stick support.
* Accessibility and sensitivity options.

---

## Practical acceptance criteria

The camera is ready for the vertical slice when:

* `WASD` remains intuitive at every camera angle.
* Holding left mouse can look behind the character without changing facing.
* Holding right mouse aligns character facing with camera yaw.
* Both mouse buttons move and steer consistently.
* The camera never enters terrain during ordinary movement.
* Cave entrances and low ceilings do not produce violent snapping.
* Zoom restoration after obstruction is smooth.
* Movement speed is independent of frame rate.
* Camera rendering remains smooth when physics uses a fixed timestep.
* Opening UI reliably releases the cursor.
* Every tuning value can be changed through YAML.

[1]: https://bevy.org/news/?utm_source=chatgpt.com "Bevy News"
[2]: https://docs.rs/bevy/latest/bevy/window/struct.CursorOptions.html "CursorOptions in bevy::window - Rust"
[3]: https://docs.rs/bevy/latest/bevy/input/mouse/struct.MouseMotion.html?utm_source=chatgpt.com "MouseMotion in bevy::input::mouse - Rust"
[4]: https://rapier.rs/docs/user_guides/bevy_plugin/scene_queries_shape_casting/ "scene_queries_shape_casting | Rapier"
[5]: https://bevy.org/examples/movement/physics-in-fixed-timestep/ "Run physics in a fixed timestep"
