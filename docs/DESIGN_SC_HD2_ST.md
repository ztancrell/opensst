# OpenSST: Star Citizen × Helldivers 2 × Starship Troopers

**Vision:** A realistic space first-person bug hunter that combines the scale and immersion of **Star Citizen**, the co-op tactics and stratagems of **Helldivers 2**, and the military-sci‑fi feel of **Starship Troopers**.

---

## Pillars

| Pillar | Source | In OpenSST |
|--------|--------|----------------|
| **Space first-person** | Star Citizen | FPS on ground; ship phase aboard Roger Young; future: pilot approach, EVA. |
| **Orbital deployment** | All three | Drop pods from orbit, retrieval boat extraction, fleet in space. |
| **Stratagems / call-ins** | Helldivers 2 | Tac Fighter CAS, extraction, supply drop; more call-ins (orbital strike, reinforce). |
| **Mission structure** | HD2 + ST | Mission types (Extermination, Bug Hunt, Hold the Line); select planet → deploy → complete objective → extract. |
| **MI vs bugs** | Starship Troopers | Trooper classes, squad, fleet, Roger Young, cinematic military tone. |
| **Realistic space** | Star Citizen | Space sky, fleet corvettes, orbit/atmosphere blend, cinematic renderer. |

---

## Current Loop

1. **In Ship** – Aboard Roger Young; war table to pick system/planet and mission (1–5); walk to drop bay.
2. **Approach** – First-person cockpit view; SPACE to begin EVA.
3. **EVA** – Zero-G float to drop pod; [E] or 6s to enter pod.
4. **Drop** – Pod descent to planet (streaming terrain).
5. **Playing** – FPS bug hunt, stratagems [B/N/R], extraction [V], tac fighter, squad.
6. **Extract** – Retrieval boat → orbit → Roger Young; space + fleet.
7. **Back to Ship** – Stats; galactic war progress saved. Ready for next drop.

---

## Roadmap (High Level)

- **Mission system** – Done. Typed missions (Extermination, Bug Hunt, Hold the Line, Defense, Hive Destruction) with objectives and “Mission complete – extract when ready”. War table keys 1–5.
- **Stratagems** – Done. Orbital Strike [B], Supply Drop [N], Reinforce [R], Extraction [V]; key-bound with cooldowns and smoke.
- **First-person piloting** – Done. Approach phase: cockpit view toward planet; SPACE to begin EVA.
- **Galactic war** – Done. Liberation, kills, extractions, major orders. **Persistent save**: `opensst_save.ron` (seed + current system + war state); load on startup, save on extraction.
- **EVA / zero-G** – Done. EVA phase: zero-G float from ship to drop pod (WASD thrust, SPACE/Ctrl up/down); [E] or timer to enter pod → drop sequence.
- **Larger scale** – More planets/systems/missions. **Co-op (networked)** – deferred.

---

## Design Principles

- **First-person focus** – Combat, ship interior, and (future) piloting in FP.
- **One life per drop** – No respawn on planet; extract or die (Helldivers 2 style).
- **Fleet matters** – CAS, extraction, and supply feel like the fleet supporting the trooper.
- **Cinematic tone** – 97 movie / Heinlein: military, propaganda-adjacent, gritty.

This doc is the north star for combining SC, HD2, and ST into one coherent “realistic space first-person bug hunter” experience.
