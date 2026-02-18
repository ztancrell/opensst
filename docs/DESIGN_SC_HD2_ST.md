# OpenSST: Star Citizen × Helldivers 2 × Starship Troopers

**Vision:** A **full universe sim** like Star Citizen — persistent galaxy, 100 star systems, travel and contracts — with **bug killing** at its core: Starship Troopers × Helldivers 2. First-person on foot and aboard ship; drop, fight, extract; galactic war and liberation persist.

---

## Pillars

| Pillar | Source | In OpenSST |
|--------|--------|----------------|
| **Full universe sim** | Star Citizen | 100-system procedural galaxy; main menu Universe Map (pick system, Enter = travel & board); M in ship = galaxy map / warp. |
| **Space first-person** | Star Citizen | FPS on ground; ship phase aboard Roger Young; cockpit approach; EVA to drop pod. |
| **Orbital deployment** | All three | Drop pods from orbit, retrieval boat extraction, fleet in space. |
| **Stratagems / call-ins** | Helldivers 2 | Tac Fighter CAS, extraction, supply drop; orbital strike, reinforce. |
| **Mission / contracts** | HD2 + SC | War table = mission board: CONTRACT (type — planet). Reward: Liberation. Keys 1–5; select planet → deploy. |
| **MI vs bugs** | Starship Troopers | Trooper classes, squad, Roger Young, Federation Bulletin, cinematic military tone. |
| **Realistic space** | Star Citizen | Space sky, orbits, fleet, orbit/atmosphere blend, cinematic renderer. |

---

## Current Loop

1. **Main menu** – Continue (saved system) / Universe Map (pick any of 100 systems, Enter = travel & board) / Quit.
2. **In Ship** – Aboard Roger Young; Federation Bulletin (sector liberation, major order); war table: CONTRACT (mission — planet), keys 1–5, pick planet; walk to drop bay.
3. **Approach** – First-person cockpit view; SPACE to begin EVA.
4. **EVA** – Zero-G float to drop pod; [E] or 6s to enter pod.
5. **Drop** – Pod descent to planet (streaming terrain).
6. **Playing** – FPS bug hunt, stratagems [B/N/R], extraction [V], tac fighter, squad.
7. **Extract** – Retrieval boat → orbit → Roger Young; space + fleet.
8. **Back to Ship** – Stats; galactic war progress saved. Ready for next drop.

---

## Roadmap (High Level)

- **Full universe** – Done. 100 star systems; main menu Universe Map (select system, Enter = travel & board); M in ship = galaxy map / warp.
- **Mission / contract board** – Done. War table shows CONTRACT: [type] — [planet]. Reward: Liberation. Typed missions (Extermination, Bug Hunt, Hold the Line, Defense, Hive Destruction) with objectives and “Mission complete – extract when ready”. War table keys 1–5.
- **Federation Bulletin** – Done. On entering ship: sector liberation %, major order.
- **Stratagems** – Done. Orbital Strike [B], Supply Drop [N], Reinforce [R], Extraction [V]; key-bound with cooldowns and smoke.
- **First-person piloting** – Done. Approach phase: cockpit view toward planet; SPACE to begin EVA.
- **Galactic war** – Done. Liberation, kills, extractions, major orders. **Persistent save**: `opensst_save.ron` (seed + current system + war state); load on startup, save on extraction.
- **EVA / zero-G** – Done. EVA phase: zero-G float from ship to drop pod (WASD thrust, SPACE/Ctrl up/down); [E] or timer to enter pod → drop sequence.
- **Co-op (networked)** – Deferred.

---

## Design Principles

- **First-person focus** – Combat, ship interior, and (future) piloting in FP.
- **One life per drop** – No respawn on planet; extract or die (Helldivers 2 style).
- **Fleet matters** – CAS, extraction, and supply feel like the fleet supporting the trooper.
- **Cinematic tone** – 97 movie / Heinlein: military, propaganda-adjacent, gritty.

This doc is the north star for combining SC, HD2, and ST into one coherent “realistic space first-person bug hunter” experience.
