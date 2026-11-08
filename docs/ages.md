# Twenty ages

Alex's decision (lead): progression is **twenty emergent ages**, with much more for agents to do. The ladder is data, in [`data/ages.json`](../data/ages.json). The Rust civilisation sim (#50), the galaxy's planet sim (#68) and later the campaign (#22) all read the same file, so there is one ladder rather than three.

This is **not a tech tree**: there's no research screen and no player-chosen upgrades. Each civilisation's own simulation earns an age by meeting its conditions, and can lose it again (README §4).

## The ladder

| # | Age | Agents start doing | New buildings | New systems | Typical crisis |
|---|---|---|---|---|---|
| 1 | Wanderers | forage, hunt | campfire | hunger, fear, fire | famine, predators |
| 2 | Hearth Clans | woodcut, cook | hut | shelter, families, night and sleep | cold snap |
| 3 | Toolmakers | knap stone, fish | workshop, raft | tools, coastal exploration | lost at sea |
| 4 | First Farmers | farm, herd goats | field, goat pen | seasons, harvest | drought, locusts |
| 5 | Villages | build, carry, settle | granary, well | storage, settlers, territory | granary fire, flood |
| 6 | Trade Routes | trade, guard | market, dock | trade between civs, goat currency | bandits |
| 7 | Chiefdoms | chief, warrior, raid | longhouse, palisade | leadership, tribute, raids | succession split |
| 8 | Bronze Age | mine, smith | mine, forge | metal tools and weapons, ore scarcity | ore war |
| 9 | Kingdoms | scribe, tax | palace, road | law, taxes, inequality, unrest | tax revolt |
| 10 | Temples & Faith | priest, pilgrim | temple, shrine | religion, conversion, holy sites | schism |
| 11 | Empires | general, engineer, soldier | barracks, aqueduct, fort | conquest, vassals, roads, overextension | province revolt |
| 12 | Medieval Realms | knight, monk, miller | castle, monastery, mill | lords, guilds, disease along trade | plague, peasant revolt |
| 13 | Age of Sail | sailor, explorer, colonist | shipyard, port | overseas colonies, naval war | shipwreck storms |
| 14 | Renaissance | scholar, artist, banker | university, bank, theatre | faster knowledge, credit, culture spread | censorship |
| 15 | Industrial Revolution | factory, rail, miner | factory, railway | goat exchange, strikes, smog | market crash |
| 16 | Revolutions | agitator, reformer | parliament | government change, civil war, splits | civil war |
| 17 | Electric Age | electrician, clerk | power plant, street lights | cities, night lights, alliances | blackout, world war |
| 18 | Information Age | programmer, broadcaster | tower, data centre | networks, instant ideas, misinformation | panic, cyber war |
| 19 | Space Age | engineer, astronaut | launch pad, satellite | rocket project | launch failure |
| 20 | Interstellar | colonist, terraformer | starport | galaxy colonisation | colony collapse |

Entry conditions are in `data/ages.json` (e.g. Kingdoms: population 80, 5 settlements, knowledge 20). They are built from simulation **metrics** defined at the top of that file: population, settlements, food days, stability, territory, knowledge, wealth, faith, production, links, contact, coastal, vassals, colonies and inequality. Jobs and buildings are cumulative: a Kingdoms civilisation still hunts, farms and trades.

## What makes it complex

- **Different paths for different personalities.** The calibration traits (friendliness, aggressiveness, violence; #54) shape how a civilisation meets each age's conditions:
  - friendly civs reach Empires through vassal treaties and trade;
  - aggressive ones get there through conquest;
  - violent ones hit more crises and civil wars.

  Some ages have alternative conditions (`"any"`) for exactly this.
- **Collapse.** Stability below 0.15 for 30 s, or population below 60% of the age's entry level, drops a civilisation back one age: a Dark Age. Its buildings go dormant and its jobs stop until it climbs back.
- **Crises are emergent.** Plague spreads along trade routes, ore wars start over hills, and revolutions come from inequality. The player's signals tilt them but don't script them.
- **Daily life.** Agents sleep at night, eat at the hearth or market, hold festivals when the player is joyful, mourn their dead, form families, make friends and rivals across civilisations, and migrate when hungry or at war.

## Conflict
Fights are **agent against agent** and come out of the simulation. They also happen **inside a single civilisation**: a civilisation alone on its planet still has clans, rebels and lords who fight. The full model is under `conflict` in `data/ages.json`.

**Escalation ladder.** Each level can grow into the next, or calm back down:

| Level | Who fights | Started by | Result |
|---|---|---|---|
| Scuffle | two agents | an insult, a rivalry, the same food or tile | wounds; a grudge between families |
| Feud | two families or clans | repeated scuffles or a death | ambushes, revenge, stolen goats |
| Raid | a war band against a settlement | hunger, greed, aggressive traits | stolen food, burned buildings, captives |
| Battle | two armies | a raid answered, a border dispute | a won or lost field; morale swings |
| War | two civilisations or realms (age 7+) | several battles, or a declaration | territory changes hands; truce or conquest |
| Civil war | factions inside one civilisation | low stability plus inequality, schisms, succession, revolution | the civilisation splits, or the rebels are crushed |

**Fighting inside one civilisation** grows with its age:
- clan feuds (ages 1–6);
- succession splits (7+);
- province revolts (11+);
- peasant revolts and plague riots (12+);
- civil wars and revolutions (16+);
- misinformation riots (18+).

**Battles:**
- **Morale:** every fighter has morale. Nearby losses, being outnumbered and a wounded leader lower it. At zero a fighter flees, and a rout can spread down the line.
- **Terrain changes the odds:** hills ×1.3, forest ×1.2, walls ×1.8, attacking across a river ×0.7.
- **Formations:** from age 11, armies hold a shield wall, flank and keep reserves. Earlier fights are brawls.
- **Wounds:** fighters are wounded before they die; healers (age 12+) and rest bring them back.
- **Aftermath:** surrender and prisoners, ransom or forced labour, truces, vassalage. Grudges restart feuds later.

**Weapons change by age:** clubs → stone spears → slings → bronze swords → legions → knights and crossbows → cannon and muskets → rifles → artillery → drones → orbital defences. Each age's `weapon` in the data sets power and range.

**The player on the battlefield:**
- anger or stress pushes conflicts up the ladder;
- calm drifts them down toward a truce;
- lightning breaks a charge, and the struck side loses morale.

The peace, fireball and disaster spells went with the other hand-gesture casts in #261. The charged fireball that came back in #291 is a burst at the cursor; it does not strike armies.

## Where the player's brain comes in
Every age has a "brain moment" (the `brain` field in the data). Examples:
- calm brings rain to first farmers and keeps the lords loyal;
- focus lets the scribes write law and fuels Renaissance breakthroughs;
- stress crashes the goat market and spreads misinformation;
- anger drives conquest and bloody revolutions;
- blinks are lightning, miracles or storms at sea.

## Pacing
- **Sandbox:** the full ladder takes roughly 25–30 minutes at normal speed. A time-speed control (×1, ×4, ×16) lets a demo show 6–8 ages in three minutes.
- **Galaxy:** worlds run the same ladder at an abstract level (#68).

## How today's game maps onto it
- **The campaign's three eras** are ages 1, 15 and 19.
- **The two cutscenes** (Farming → Kingdoms, Information) stand in for ages 4–9 and 18. They become real stages as the campaign moves onto the shared ladder (#22).
- **The galaxy worlds' four-era bar** becomes this ladder (#68).

## Implementation notes
- **Rust (`moc-civ`, #50 follow-up):** generate a Rust table from `data/ages.json` (a small script or `build.rs`), so the wasm doesn't carry a JSON parser. The planet-level metrics are computed per civilisation each step.
- **JS (`src/cosmos/planet-sim.js`, #68):** import the JSON and keep the logic DOM-free.
- **Naming:** don't use names the coherence loop treats as a tech tree (`tech_tree`, `research_tree`, `evolution_system`, `evolve(`). Ages are reached, not researched.
