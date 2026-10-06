# M3 go/no-go playtest

M3 is the go/no-go point (SEED.md): is marching an army through living villages interesting? Two things need you and the reference machine (8 cores, RTX 3060 class).

## 1. Stress run (RND-04)

```
cargo run --release -p game_app -- --stress 20000
```

This stages about 20,000 agents: the 5,000 villagers plus two armies meeting in battle, with the camera on them. Every 5 seconds the log prints a line like:

```
stress: 20035 agents | 60.0 fps, frame 16.6 ms | sim tick 7.9 ms (avg 8.1) | bands ...
```

Please send me a few of those lines from before, during and after the armies meet. Also zoom in and out (mouse wheel) while it runs. The targets are 60 fps (RND-04) and a sim tick of 8 ms or less (NFR-03).

## 2. Playtest (about 30 minutes)

```
cargo run --release -p game_app
```

**Your start:**
- Your champion with 50 men (blue) beside the populated region, with about 2.5 days of food.
- A raiding band (red) on the far side. It plunders villages when short of food, and otherwise hunts the nearest enemy army: you.

**Controls:**
- Left-click a soldier to select the army, and the captain panel opens.
- Right-click the ground to move; Shift queues.
- Right-click a village for: gather food, recruit 10, trade, equip, attack, move here.
- Captain panel buttons: posture, Return, Cancel, Appoint captain (select one of your followers first).
- Keys:
  - R returns to camp; Backspace cancels orders; P cycles posture.
  - Space pauses; 1–4 set the speed.

**Try to:**
1. Feed the army before it runs out: gather food from a village, once each posture, and watch the villagers.
2. Recruit, then equip at a village with a smithy (weapons from the M2 economy).
3. Meet the raiders. Does the fight read clearly? Does routing make sense?
4. Attack a village as aggressive and watch the militia rise. Then pass a village as passive.
5. Appoint a second captain and run two armies.

**Please tell me:**
- **Was it interesting?** Did your choices (posture, where to get food, when to fight) feel like they mattered?
- **Readability:** could you tell what your army was doing and why, from the panel, the inspector and the event lines?
- **What felt wrong or boring:** marching, waiting, food pressure, combat, villagers' reactions.
- **Go, or no-go:** should M4 (conquest, loyalty, faith and heroes) build on this, or should the core loop change first?
