# 白球デイズ (Diamond Days): design notes

An original high-school baseball manager for phones. You are the new coach of a small
school's baseball club. Year after year (for as long as you like) you train the team
week by week, play practice games and tournaments, and aim for the national championship
at the "Summer Stadium". Players graduate every March and first-years join in April, so
the club keeps changing while your record grows.

Everything here is original: names, rules, events, pictures. It uses only bundled CC0
assets (Kenney mini characters, platformer tiles) and the shared engine parts.

## The loop (one turn = one week)

1. **Pick a card.** You hold five cards, each with a number of steps (1 to 6) and
   sometimes a theme ("Batting focus": batting squares give double this week).
2. **Move on the board.** The board is a road of squares running to the right under
   the school's practice ground (a loop of twenty, drawn as a straight road). The coach
   stands near the left; the six squares ahead carry their step numbers, lit where a
   card in hand lands, and as he walks the road slides by.
3. **The square happens.**
   - Training squares (Batting, Pitching, Fielding, Running, Strength): every player
     gains practice points in that area, more when in good condition, less when tired.
     Ten points raise a stat by one.
   - Rest: stamina comes back, condition improves.
   - Event: something happens (a sponsor brings drinks, an alumnus coaches, a player
     twists an ankle, rain). Events are JSON.
   - Spirit: the whole team's condition goes up.
   - Scout: a middle-schooler promises to join next April.
   - Film: game film together — fielding and batting points, and nobody tires.
   - Grounds: the club keeps the field tidy — a little rest, and reputation rises.
   - Game: a practice game against a nearby school this week (in winter, red against
     white for points). Practice games come only from these squares, so the coach decides
     how often the club plays: Emma, "無駄な試合が多い", asked for practice first.
4. **Fatigue.** Training costs stamina; a tired player trains worse and can be hurt.
5. **The calendar moves.** Some weeks are game weeks (below); the board waits.

## The year (48 weeks, April to March)

The year follows a real high-school club (the scenario and why: `diamond_days_scenario.ja.md`).

| Weeks | What |
|---|---|
| 1 | April: first-years join (after year 1) |
| any week but winter | A practice game when the coach lands on a **Game** square on the road (two per lap). In winter the same square is a game among ourselves, for points |
| 11 | Training camp |
| 12 | Summer numbers (the coach gives number 1 to a pitcher) and the draw |
| 13 | Summer prefectural (6 rounds; a seed, last autumn's last four, starts in round 2). Lose once and the third-years' summer is over |
| 17 | Summer Stadium, the national tournament (5 rounds), if we won the prefecture |
| the day our summer ends | The third-years retire (a word from each), then the new team names a captain |
| 24 | Autumn tournament of the new team (4 rounds). The last four are seeded next summer |
| 27 | The pro draft: a retired third-year may be picked |
| 33 to 44 | Winter: no games with other schools; training counts for 25% more |
| 37 | Selection day: a deep autumn run waits by the phone for the March invitation |
| 40 | Exam week |
| 45 | Blossom Invitational (3 rounds), if selected |
| 48 | March: the retired third-years graduate; the year ends |

A tournament is played as one stretch of game days from its first week: the board
waits, the draw shows our path from the first round up to the final (the scores so far,
the next school lit), and the day before each game is rest, tune up (mood), or study the
next school (it plays a little weaker). A win stays on the same week; when the tournament
is over the calendar moves past it. Tiredness carries from game to game, so the ace's arm
is a real worry in a long run. Number 1 starts when his arm is rested (stamina 45 or
more). Extra innings are a tiebreak: from the 10th every half starts with men on first and
second.

The years never run out. Reputation rises with wins and brings better first-years, and
the region names the school by its fame stars: 無名校 (Unknown), 注目校 (Noticed),
実力校 (Contender), 常連校 (Regular), 伝説校 (Legend). The names are our own.

## Players

Each player has a name, a grade (1 to 3), a position, and stats from 1 to 99, shown as
letter grades (S, A to G):

- Batting: Contact, Power, Speed, Fielding, Catching, Arm.
- Pitching (pitchers): Velocity (km/h), Control, Stamina, Breaking ball, and **pitches**
  besides the fastball (slider, curve, cutter, splitter, two-seam, sinker, changeup), each
  at a level 1 to 7.
- **Hands**: right or left (the same for throwing and batting), shown on his page
  (右投右打 / 左投左打) and in the ballpark.

Plus condition (Great, Good, Normal, Poor, Awful), stamina (0 to 100), and weeks hurt.

### Left and right, pitches, errors (round 7)

- A batter facing a pitcher from his own side loses contact and some power; from the
  other side he sees the ball better.
- A slider, curve or cutter breaks away from a batter on the pitcher's side; a two-seam,
  sinker or changeup away from the other side; a splitter gets swings and misses from
  anyone. Three pitches or more keep every batter guessing.
- The breaking ball grows into pitches: a second at 40, a third at 65, and every eight
  points of breaking ball a pitch moves more.
- **Catching** decides errors: a ball that would be an out can be dropped (ERROR!), the
  batter reaches and runners move up a base. A team of shaky hands drops several a week;
  sure hands almost none.

## Games

A game is simulated plate appearance by plate appearance from the players' stats. The
pitcher tires; when his stamina runs out the next pitcher comes in. You watch the
play-by-play line by line (tap to hurry) on a scoreboard, and the game stops for your
call at chances and pinches (below). "Skip" plays the rest at once.

## Growth (round 2)

- Training puts **points** into five pools (batting, pitching, fielding, running,
  strength). The coach spends them on a player's page: each stat costs more the higher it
  is, and each pool pays for its own stats (fielding pays for fielding and arm, strength
  for power and stamina, pitching for velocity, control and the breaking ball). "Auto
  spend" raises the lowest stats first.
- Every player has his own **practice menu** (one of the five drills) and earns a third of
  a square's points in it every training week, whatever square the coach lands on.
- A **card** can carry a player's name: that week he trains twice as hard.
- **Scout** squares find a middle-schooler (position, a rating) who joins next April,
  up to four a year.

## Traits: good, gold and bad (round 7)

37 traits instead of 8, each with a real effect in the game or in practice, all names
original:

- **Good (21).** The first eight (big moment, fireman, late grit, burner, iron body, spark,
  good eye, heater) and thirteen more: pull power, all fields, table setter (leading off),
  bunt artist, smart feet, soft hands, rocket arm; for pitchers bat misser (strikeouts),
  keeps it low (fewer home runs), paint brush (few walks), long haul (stamina), quick move
  (fewer steals), warms up (better later).
- **Gold (8).** The step above a good trait, replacing it and still counting as it: big
  stage, moonshot, gale, iron wall, roaring heat, K machine, ice heart, unbreakable. A good
  trait turns gold when its stat reaches the top (power 88, speed 88, fielding 88, velocity
  150, breaking 86, control 86, power 92 for the big moment).
- **Bad (8).** Habits to train away: tight grip (at chances), glass body (hurt more
  often), streaky, wild, long ball, wild throws, slow start, rattled. Some players are born
  with one; each goes when the stat behind it reaches a level (control 55 for wild, fielding
  60 for wild throws, and so on).
- A player holds up to four good or gold traits; bad habits take no slot. He learns a good
  one when a stat crosses its threshold, from some events, and from talks.

## The month's practice (practice instruction)

Emma: 「月に1回練習指示で能力向上も基本は自動にしたい」.

- Every month the staff sets each player's individual drill to what he lacks most for his
  job (a pitcher's velocity, control, stamina or breaking ball; a fielder's bat, power,
  legs, glove or arm), and the points saved up turn into growth on their own. A short
  "A new month" card says what moved.
- **Club → Practice** (練習指示) shows the whole team in two columns, one chip per player.
  A tap moves a player to the next drill (a fielder never gets pitching) and marks it the
  coach's own: the monthly automatic menus leave it alone. **Auto** turns the monthly
  menus off; **All to auto** clears every hand-set menu and sets them again.
- A new player starts on the drill he needs most.

## The extra scout (April gacha)

April has one more door: after the usual intake, the coach may pull the **extra scout**
— a gacha over this spring's hopefuls, with the pull told on a notice card and the boy
joining at once as a first-year (he does not count in the walk-in numbers and is not a
scouted promise).

- **Pulls, not money.** The year turn deals free pulls (2, or 3 at reputation 60+);
  they are April-only and vanish when the month turns. **Tickets** keep across years
  (at most 9): one from a scout square that finds the year's book already full, one
  from a prefectural title, one from a first game at the Summer Stadium.
- **Five tiers** stand on the walk-in level (`33 + reputation × 0.25`): Normal (the
  level itself), Fine (+8), Promising (+16, a trait guaranteed), Star (+26, two traits
  with a signature — a heavy ball for a pitcher, a batting trait for a fielder) and
  Legend (a flat 78–86, two traits, signature included). Chances grow with reputation:
  Legend 0.3% → 1.5%, Star 2% → 5%, Promising 8% → 10%, Fine a flat 30%, the rest
  Normal. Ten pulls without a Promising-or-better make the next one Promising-or-better
  (drawn at the usual relative rates; no pity aims at a Legend itself).
- **A Legend is a reborn alumnus.** He wears the name of one of the school's own pros
  (from the `pros` record; a fictional legend's name until the school has one), is
  flagged `legend`, and is never renamed by the unique-names pass — the club renames
  around him. The card says which year he went pro.

## Calls in a game (round 2)

The game stops at a **chance** (we bat with a runner on second or third) or a **pinch**
(they do). At a chance: swing away, take pitches, hit and run, bunt, squeeze (runner on
third), steal, or bring in a pinch hitter or pinch runner. At a pinch: go after him, work
him inside, stay away, keep it low, bring the infield in (runner on third), walk him on
purpose, bring in the next pitcher, or make a defensive change. "Play on" lets them play
this once; "no more calls" for the rest of the game. The 作戦 button asks for any batter.

Every call trades one risk for another (the button says which, under its name), and one
button is tagged おすすめ from the matchup and the bases:

| Call | Better | Worse |
|---|---|---|
| 勝負 (go after him) | more stuff | the arm tires sooner |
| 内角攻め (inside) | more strikeouts, jammed swings | a slugger turns on a mistake; a pitch can hit him (more with poor control) |
| 外角中心 (away) | far fewer home runs and doubles; a strikeout is a called third strike | more walks |
| 低めに集めろ (low) | ground balls, more double plays, fewer home runs | a few more walks and singles |
| 前進守備 (infield in) | a ground ball never scores the runner from third | more balls get through |
| エンドラン (hit and run) | no double play; first to third on a single; more contact | a strikeout can hang the runner |

The other bench calls too, from the same reading of the game, in the spots that matter
and not every time: their battery works our sluggers away and gets low with a man on
first; their bench hits and runs, squeezes and steals. The coach calls only for his own
side (batting calls when we bat, fielding calls when we field), so both calls can be in
force for one batter, and chips over the picture show them (作戦：… and 相手：…).

A ground ball out moves runners (the man on third scores about half the time against a
normal infield). A hit batsman takes first and is not an at bat. The pitches on screen
follow the call: in on the hands, on the outer corner, or low in the zone, and the one
that hits him is right at him.

## Special abilities

Each player has zero, one, or two traits. They change the plate appearance they are
already part of, and the field shouts the name when ours fires (「ここ一番！」). All names
are original (none are taken from another game).

| Trait | When it matters |
|---|---|
| ここ一番 / Big Moment | We bat with a runner in scoring position |
| 火消し / Fireman | Our pitcher, and they have a runner in scoring position |
| 終盤の粘り / Late Grit | The 7th inning or later, within two runs |
| 韋駄天 / Burner | A stolen base |
| 鉄の体 / Iron Body | Training, camp, and random injuries; also shrugs off a red mark |
| 元気印 / Spark | A small lift for the rest of the order, named at first pitch |
| 見極め / Good Eye | More walks |
| 剛球 / Heater | Pitching stuff |

The founding ace throws a heavy ball and the founding captain is the spark; both faces
are drawn by hand. Everyone else is a restrained random face. Traits are also learned:
a conversation can grant one, and a stat crossing a threshold can teach one (speed 70
is 韋駄天, and so on). A player never holds more than two.

## The order

`Team::lineup` is still the automatic nine: best at each position, speed leading off,
the strongest third and fourth, the pitcher ninth. That is 「おまかせ」. From the roster
the coach can reorder, change who stands where, and swap in someone who can play. The
card is stored by family name, so an old save still loads; if it is no longer legal
(a red mark, an injury, no pitcher on the mound) the automatic nine comes back for
that game. The coach's card returns when the player can play again.

## The circuit, and March

Ten named schools stay for the whole run (北浜, 南野, 桜ヶ丘, 東山, 西川, 旭台,
緑ヶ丘, 川辺, and the academies 白石 and 星見). Each has a colour, a face, and a
strength that grows when they beat you and a little every spring. A practice game in the
same week next year is a rematch; a tournament draws our path, the strongest school on the
circuit waiting near the end. The national tournament and the March invitation bring
schools from other prefectures (made-up names).

Three autumn wins put the club on the list for the **Blossom Invitational** (春の招待舞台)
in March, two make it a coin toss; the answer comes on selection day in January. It is
not a licensed event name.

## The story of the first chapter

Original characters, told in the notes as the year goes:

- **Kotori** (七瀬ことり), the manager, starts as a first-year the day the coach arrives
  and grows a year with the club. Her own summer ends with the third year's: a letter
  to the coach, and she stays on as an old girl to keep the scorebook.
- **The founding captain**, a third-year: his last summer is the coach's first. The goal
  that year is one summer win, after ten years without one.
- **The rival ace** (神崎 at 星見学園; later chapters bring a new name): a first-year
  monster in year 1, a year older and faster every year (141, 148, 155 km/h). The draw
  puts his school in our half, in the semifinal; the summer opening names where he
  waits, and beating him (or losing to him) gets its line.

## School life, thinly

- **Exams.** The old exam event, and a fixed week in January. Mood drops. One or two
  players who are not 鉄の体 get a red mark and cannot play for two weeks — the
  same gate as an injury, not a study minigame.
- **Summer camp** (week 12, before the prefectural tournament). Extra practice in every
  drill, a real chance of a knock, and sometimes a conversation on the steps.
- **Retirement and graduation.** The third-years leave the team when our summer ends
  (a word from each, Kotori in tears); a club left with fewer than ten takes in players
  from other clubs. They graduate in March, the face card showing プロ (only if the
  October draft picked them), 大学, or 就職.
- **The captain.** The new team names one of its oldest players: the best player
  (leads by example, ここ一番), the loudest (leads with his voice, 元気印), or the
  steadiest glove (holds the field together, 終盤の粘り). He calls the huddle before every
  tournament.

## Practice and games are two parts

The week is the practice part: the board of squares round the school's practice ground
(the school building, the net, a batting cage, a bullpen, the running lane), where the club
is drawn as chibi players at their own practice menus. No stadium on the board.

A fixture is the game part, on its own screens:

Going into a game the screen opens on a red curtain with 試合の日！, Play Ball opens an iris
on the broadcast, and leaving the result opens a green curtain with 練習に戻ろう.

1. **Game day.** The tournament ribbon, both school crests, team grades, the record between
   the two schools, our starting nine with faces. The coach can change the order, then
   presses Play Ball.
2. **The game, in 3D.** A high-school ballpark (`hud/stadium.rs`): a dark dirt infield and
   dirt foul ground, grass mown in stripes, chalk, a green fence with a yellow cap, stands
   packed with fans, foul poles, a scoreboard, light towers lit for autumn and winter games,
   the sky plates standing round it. Both nines are 3D chibi players (`game_core::chibi3d`,
   toon primitives with outlines; home whites and road greys in the school colour, batting
   helmets, the catcher's gear, an umpire). The camera sits high behind the catcher: the
   pitcher's whole motion on the mound, the ball's flight to the plate, the batter's swing
   (about one in four bats and throws left, for looks). Every plate appearance is shown
   pitch by pitch (0.7 s a pitch), with balls, strikes and outs on a TV count box, the speed
   of the pitch, and a small strike zone with a numbered dot where each pitch crossed. The
   pitches are drawn from the play (a strikeout ends on a swinging third strike, a walk on
   ball four), so the picture never changes a result. When the ball is in play the camera
   cuts to a high view behind home that follows the ball: the nearest fielder runs for it
   (and gets there on an out), the batter runs it out round the bases, the runners go.
   The HUD paints only above and below the picture (`View::in_band`). Chances and
   pinches stop for the coach's call as before. A tap skips to the result; double speed
   and "to the end" are buttons.
   The coach can also call any batter: the 作戦 button asks for the next plate appearance,
   whoever bats, chance or not.
   The big moments get their own picture. A home run holds about three times as long as
   a single: light rays turn behind ホームラン！, and when it is ours paper bursts over the
   picture and the caption gives his number here (「三浦、高校通算12号！」, his career
   home runs before the game plus today's). The last line of a game is a scene of its own,
   held about four seconds: a walk-off brings the whole bench out of the first-base
   dugout to ring the batter who won it, jumping, while the other nine stand with their
   heads down (サヨナラ！); any other end lines both nines up in two rows in front of the
   plate, the umpire at the plate, and they bow together, then the winners jump and the
   losers hang their heads (ゲームセット). Paper falls on our wins.
   Tournament ceremonies, at the last line of a tournament game: after the bow the
   winners turn to the field for their school song (ours with words of our own, a line
   at a time: 「朝日に映える 学び舎の」…), and the losers run to their own stands and
   bow (「ありがとうございました！」). Winning a final (not by a walk-off) is the ring at
   the mound: the whole nine sprint in, jump, fingers to the sky, 優勝！ with paper
   falling. With a runner in scoring position for us, the brass band plays the chance
   theme (♪ チャンステーマ ♪) and the stands sway.
3. **The result.** Win or loss, the final line score, the player of the game (a chibi
   cheering, with his line), the game experience the club earned (practice points: batting
   for every one of the nine, more per hit; pitching for whoever pitched, more for
   strikeouts; fielding on a win), what it means for the tournament, a word from Kotori,
   then back to practice.

Below our order, a radar puts both nines on one chart (contact, power, speed, fielding,
the starting pitcher) on a square-root scale, so a young club still shows its shape.

The game-day screen shows their ace (face, name, speed) between the crests: he is the one
who starts.

The roster and the player picker are lists that follow a finger (drag, fling, spring back
at the ends); a tap that hardly moved opens a player. Raising stats pops "+1" by the row
and the new grade ("ミート E→D!") over the page.

Every game adds to each player's numbers, this season's and his whole time at the school:
games, at bats, hits, home runs, runs batted in, and for pitchers batters faced,
strikeouts and runs. A player's page shows this season's line. At the year's end a notice
names the season's best bat (average, with at least 8 at bats) and best arm
(strikeouts); then the season numbers start again. Graduates who go pro are remembered,
and the career card counts them (プロ入りOB).

## Endless play

A career does not end on its own. The year count runs on (1年目, 2年目, ... 30年目) and
every finished year goes into the career record: the year, the school, the best result,
and that year's wins and losses. The record is part of the save, so it survives reloads.

Every three years is a milestone (ひと区切り), not an ending. The card shows the last
three years, the career wins and losses and national titles, and offers: keep going at
the same school (the default; reputation and the grounds carry, the first-years arrive),
move to one of the weaker circuit schools (your old school stays on the schedule), or
retire. Retiring shows the career card (the latest years with medals, the totals) and
the next visit starts a new career. Old fixtures are dropped at each milestone so a long
career keeps a small save.

Grounds are a small training bonus (up to three levels) that rises with reputation
and is kept if you stay.

## Opening the game again

A run is kept on the device. Opening the page again, the title says つづきから (the
manifest's `page.resume`), with a smaller はじめから that erases the run after asking.
Continuing never drops the coach straight into a game: Kotori first says where the run
stands (year, month, week, record) and what this week holds, and the week (a game day too)
starts on the tap. The `?week=` link switch is for testing only: it starts a new run at
that week and never moves a kept one. Links for the owner do not carry it.

## Rounds

- **Round 1 (this PR):** the week loop (cards, board, training, rest, events, fatigue,
  injuries), the team (12 to 16 players, generated names), the calendar with practice
  games, the summer tournament and the autumn tournament, the game sim with orders,
  graduation and first-years, reputation, save on the device.
- Round 2 (this PR too): points and the coach spending them, practice menus, cards with a
  player, scouting, calls at chances and pinches.
- Round 3: events with choices (dialogue — the coach picks an answer, and some answers
  teach a trait), the coach's own growth, rival schools with names, records and an
  end-of-three-years card. Choice events and the three-year result are in the game.
- Round 4 (the picture): every screen is an illustration drawn by the HUD on the
  engine's 2D parts (`hud_art`, `vector`, `game_core::portrait`), phone first on a
  400-wide stage. The board: twenty squares round a little ballpark full of people, the
  club in their places as faces, the coach's token hopping. Under it the manager (an
  original character, 七瀬ことり / Kotori) says a word for the week; the cards show the
  starred player's face. A game: a field under the lights with little fielders (cap,
  arms, a glove), runners on their bases, the ball's flight for each play (plays carry
  a kind and a direction), the scoreboard, the pitcher's and the batter's faces, the
  play-by-play, the calls in a popup, and the trait shout when one fires. The roster
  and a player's page show faces, grades, mood, stamina, and traits. Paper, ink, and
  the orange accent are the same on every card.
  Look at every screen without a GPU:
  `KAGRA_SHOTS=dir cargo test -p kagra-shared --lib diamond_days::tests::screens_to_png`.
- Also in: the editable order, pinch hit / pinch run / a defensive change, the ten
  persistent schools, the Blossom Invitational, exams, camp, graduation paths, and
  endless play: the career year count, the career record, fame titles, and a milestone
  every three years where the coach stays, moves, or retires.
- Free pictures now sit behind that drawing (CC0, in `assets/diamond_days/` and the
  ledger), drawn as real engine textures (`image2d`, one picture atlas, drawn in order
  with the vector art): Segel's cartoon countryside behind the week and the title,
  colour-graded Poly Haven sky plates behind a match, the looneybits ball icon on the
  title, Kenney paper (nine-slice, tiled grain) and a hanging banner on notices, Kenney
  green/gold buttons on the roster and the milestone, Kenney stars for reputation and
  medals on the career card. The pixel stadium is not used; it would sit
  unmodified against the illustrated field. No paid art.
- Round 6 (3D): the game itself moved into 3D on new engine parts: toon primitives with
  outlines, 3D chibi players, pictures baked at run time on 3D props (the field, the
  stands), a camera framed for a band of the screen. The 2D broadcast picture is gone.
  Look at it with a GPU or lavapipe:
  `KAGRA_SHOTS3D=dir cargo test --features render --lib match_in_3d -- --nocapture`
  (`KAGRA_NIGHT=1` for a game under the lights). The big moments (a home run of ours,
  a walk-off, the bow): `KAGRA_SHOTS3D=dir cargo test --features render --lib big_moments_in_3d`.
- Still to come: a balance pass. Hit and UI sounds already use the files the other
  games ship.
