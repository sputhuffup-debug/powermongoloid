# M4 playtest: whole runs

M4 "Rise" turns the sandbox into roguelite runs (docs/PROGRESSION_DESIGN.md, docs/M4_PLAN.md): a band and a champion wake on the run island, grow by goodwill, conquest and monuments, and win by taking the rival warlord's seat. The scripted bot already wins every fixed seed it plays (docs/m4/PROGRESS_BOT.md), so the rules hold together. What needs you is the feel: is a run worth playing, and do two seeds feel different?

Budget about an hour: two short looks at different seeds (10 minutes each), then one run played properly (about 40 minutes at 4x-8x with fast-forward).

## Before you start

```
cargo run --release -p game_app -- --seed 1      # gentle: kin and neighbours, 3 days of food, bumper harvest
cargo run --release -p game_app -- --seed 3      # hard: outlaws, 1 day of food, lean year, stronger rival
cargo run --release -p game_app                  # the new-game screen: any seed, any difficulty
```

Seeds 1 and 3 are the contrast pair: the same rules, a different band, start boons, run modifiers, island and rival. Steady is the default difficulty; chaotic and harsh (`--difficulty chaotic|harsh`) are for a second hour, not this one.

**Controls** (from README):

| Action | Control |
| --- | --- |
| Pan, zoom, rotate | WASD / arrows / screen edges (Shift faster) or right/middle drag; mouse wheel; Q / E |
| Jump between settlements | N / Shift+N |
| Pause, speed | Space; 1 / 2 / 3 / 4 for 1x / 2x / 4x / 8x |
| Fast-forward | *>> Next event* in the run bar: 8x until the next notable event, then pause |
| Inspect | Click a villager, soldier, settlement or its name; Esc clears |
| Select an army | Click one of its soldiers or its captain (the captain panel opens) |
| Move | Right-click the ground (Shift queues) |
| Village orders | Right-click a village: gather food, recruit 10, trade, equip, give food, attack, garrison (yours), move here |
| Forage | F or the panel's *Forage* button |
| Posture | Panel buttons, or P to cycle passive / neutral / aggressive |
| Return / cancel | R / Backspace |
| Appoint a captain | Select one of your followers, then *Appoint captain* (greys out when the captain slots are full) |
| Offers | Buttons in the run bar under the clock: click one, then a card; *Later* closes |
| Overlays | O cycles ownership, goodwill / loyalty, food security, monuments, off |
| Monuments | Hover one for its kind, bonus and holder |
| Event feed | Bottom left; click a line to move the camera there |
| Abandon a run | *Abandon run* in the run bar, twice; the end screen offers *New run* and *Same seed again* |

If your build has saving (the run bar or the menu), try one save and resume during the long run; if not, skip that line.

## 1. Two seeds, ten minutes each

For seed 1, then seed 3, play the first ten minutes (about two game days at 4x):

- [ ] The new-game screen shows the seed, difficulty, the three start-boon cards and the run modifiers; picking a card starts the run.
- [ ] You can tell where your band is, how much food it has, and where the rival's seat is (ownership overlay).
- [ ] The first notable event comes within a few game hours (a request, raiders, a monument). Fast-forward stops at it.
- [ ] Food matters: you forage, ask a village, or give food for a request before the band starves.
- [ ] You can take a monument by standing at it (two game hours) and see its bonus.
- [ ] A village request (drive off raiders, bring food, clear a camp) can be answered and pays out.

**Do the two seeds feel different in those ten minutes?** Band, boons, food pressure, where the villages and monuments are, how soon raiders come. Write one line per seed.

## 2. One whole run (about 40 minutes)

Pick either seed (or the new-game screen's random one) and play to a win, a loss or day 40. The bot's route, if you want a baseline: recruit from nearby villages, take weak villages with an Attack, appoint a captain to garrison one, keep everyone fed from your own villages, then march every captain on the seat together.

- [ ] Recruiting, then attacking a village: the militia rises, breaks, and the village becomes yours after the hold. The chronicle says so.
- [ ] An invitation, if goodwill gets you one, is a real choice.
- [ ] Dominion boons arrive at 25% and 50% of a region; captain slots grow with the land you hold.
- [ ] A champion rank-up offers traits.
- [ ] Owned villages feed a garrison; a starved or neglected one grows restless or revolts, and you saw it coming (loyalty overlay).
- [ ] The rival levies, grows its seat's defence, and from day 10 raids you.
- [ ] Taking the seat ends the run in a win; the end screen's summary and chronicle tell the run.
- [ ] (If saving is in your build) save mid-run, quit, resume: the run carries on where it was.

## Please tell me

- **Was a whole run worth playing?** Where did it drag (waiting for food, walking, holding a village) and where was it tense?
- **Did the seeds differ** enough in the first ten minutes to make you want another run?
- **Readability:** could you tell why a village rose, revolted or invited you, and why an attack failed?
- **Pacing:** did the director's raiders feel like pressure or noise? Did the rival feel like a clock?
- **Balance:** did you win, and on what day? Was the seat too hard, too easy, or about right at your army size?
- **Bugs:** anything stuck (a captain that never arrives, an order that silently does nothing, an offer that won't close).
