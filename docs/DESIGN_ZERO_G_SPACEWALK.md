# Zero-G & Spacewalk — Design Brainstorm

**Goal:** Realistic zero-gravity movement in outer space and optional spacewalk/EVA gameplay.

---

## Is it possible? **Yes.**

The game already has a clear “in space” state and movement path you can extend.

### Current state

- **When you’re in space:** `current_planet_idx == None` (after `leave_planet()`, or at main menu, or after FTL).
- **Current “space” movement:** `handle_noclip_movement()` → camera `process_fly()`. That’s a **fly camera**: every frame you apply a new velocity from input and move by `velocity * dt`. There is **no momentum** and **no gravity** (gravity only runs when on a planet in `handle_fps_movement()`). So space is already “no gravity”; it just doesn’t *feel* like zero-g because there’s no inertia.
- **How you get to space today:**
  - Fly up off the planet (noclip or high altitude) until `altitude > atmo_height * 2` → `leave_planet()`.
  - Extraction boat climbs to orbit (camera locked in boat; no free EVA yet).
  - FTL to a system (you’re in ship, then you can deploy to planet).

So the **outer space area** already exists; the missing piece is **zero-g physics** (persistent velocity + thrust) and, optionally, a dedicated **spacewalk/EVA** entry point and gameplay.

---

## 1. Real zero-g physics (minimal change)

**Idea:** When in space, movement is **momentum-based**: velocity persists, and input applies **thrust** (acceleration), not direct position change.

- **State:** Reuse or extend `player_velocity`. When `current_planet_idx.is_none()` and not in extraction boat, treat it as EVA velocity.
- **Update each frame:**
  - **No gravity** (already true when not on planet).
  - **Thrust from input:** WASD + Space/Ctrl → direction in camera space (forward/right/up) → add acceleration to `player_velocity` (e.g. EVA pack thrust).
  - **Position:** `position += player_velocity * dt`.
  - **Optional:** Very low or zero drag (true vacuum); optional “brake” key that thrusts opposite to current velocity to slow down.
- **Where to plug in:** In `handle_player_input()`, when `current_planet_idx.is_none()` and not in boat, call a new `handle_zero_g_movement(dt)` instead of `handle_noclip_movement(dt)`. You can keep noclip as a debug-only option (e.g. when `debug.noclip` and on planet) so zero-g is the “real” space behavior.

**Result:** As soon as the player is in orbit (after leave_planet or any future “in space” state), they get realistic zero-g: they drift when they release keys and use thrust to speed up or slow down. No new phase or assets required.

---

## 2. Spacewalk / EVA as a gameplay feature

### Entry points

- **From orbit (existing):** Once zero-g movement is in place, “spacewalk” is just “being in space and moving.” You could add a simple HUD label (“EVA” / “Zero-G”) and maybe an EVA pack fuel/O₂ bar later.
- **From the ship (new):** Add an **airlock** in the Roger Young (e.g. a door near the CIC or corridor). Interact (E) → transition to “in space” with camera position set **near the ship** in world space (e.g. a fixed offset from the Roger Young hull). Then use zero-g movement. “Return to ship” could be: fly back to airlock volume → interact → transition back to InShip.
- **From extraction (optional):** When the boat reaches orbit, allow “exit boat” (risky, no return?) for a short EVA before main menu / ship transition.

### Gameplay ideas

- **Repair / maintenance:** Hull nodes or “damage” zones on the ship; player floats to them and interacts (repair minigame or timer).
- **Retrieve cargo / rescue pod:** Spawn a floating crate or pod in space; player must EVA to it and interact to bring it back (or tag it for pickup).
- **Defend from boarding:** Enemy EVA units or boarding craft approach the ship; player fights them in zero-g (weapons with recoil affecting velocity, or melee).
- **Navigate:** Float to another airlock, or to a corvette/destroyer, for a “visit” or objective.
- **Avoid hazards:** Debris field (moving obstacles), radiation zones, or time limit (O₂ / EVA pack fuel) so the player must return to ship or reach a safe zone.

### Technical scope (rough)

| Scope   | What you add |
|--------|---------------|
| **Minimal** | Zero-g movement when `current_planet_idx.is_none()` (velocity + thrust, no new phase). |
| **Medium**  | Zero-g + ship airlock: exit ship to EVA near Roger Young, return by re-entering airlock volume. Optional simple collision with hull so you can’t fly through the ship. |
| **Larger**  | Dedicated EVA phase, O₂/fuel meter, objectives (repair, retrieve, defend), and specific assets (hull nodes, cargo pods, enemy EVA). |

---

## 3. Implementation notes (zero-g only)

- **Camera:** Keep first-person; no need to change camera. Only the **movement** logic changes from “instant velocity from input” to “integrate acceleration into velocity, then integrate velocity into position.”
- **Coordinate system:** Space position is already in “universe” or “orbit” space (`universe_position`, `camera.transform.position`). Zero-g can stay in that same space; no need for a separate “EVA local” space unless you want a separate “near ship” map.
- **Collision:** Initially you can do zero-g with no collision (like current noclip). Later: simple AABB or capsule vs. Roger Young hull (or simplified hull boxes) so the player can’t pass through the ship when doing EVA near it.
- **Rendering:** Existing space skybox and celestial bodies already work when in space; no change needed for “look and feel” of outer space.

---

## 4. Summary

- **Outer space already exists** in your code; it just uses a noclip-style fly camera.
- **Real zero-g** = use **persistent velocity** and **thrust from input** when in space instead of direct position change. That’s a small, well-defined change in the movement path (`handle_noclip_movement` → `handle_zero_g_movement` when in space).
- **Spacewalk** can start as “zero-g in orbit” with no new phase; then add an **airlock** on the ship for explicit EVA entry/exit and optional objectives (repair, retrieve, defend, navigate) to turn it into a full gameplay feature.

If you want to implement only the physics first, the next step is to add `handle_zero_g_movement(dt)` and switch to it whenever the player is in space (and not in the extraction boat).
