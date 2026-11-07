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
2. **Move on the board.** The coach token walks that many squares around a ring of
   twenty squares laid around the diamond.
3. **The square happens.**
   - Training squares (Batting, Pitching, Fielding, Running, Strength): every player
     gains practice points in that area, more when in good condition, less when tired.
     Ten points raise a stat by one.
   - Rest: stamina comes back, condition improves.
   - Event: something happens (a sponsor brings drinks, an alumnus coaches, a player
     twists an ankle, rain). Events are JSON.
   - Spirit: the whole team's condition goes up.
4. **Fatigue.** Training costs stamina; a tired player trains worse and can be hurt.
5. **The calendar moves.** Some weeks are game weeks (below); the board waits.

## The year (48 weeks, April to March)

| Weeks | What |
|---|---|
| 1 | April: first-years join (after year 1) |
| 6, 20, 36, 44 | Practice game against a nearby school |
| 12 | Summer camp |
| 13 to 17 | Summer prefectural tournament (knockout, up to 5 games) |
| 18 to 19 | Summer Stadium, the national tournament, if you won the prefecture |
| 25 to 27 | Autumn tournament (up to 3 games) |
| 40 | Exam week |
| 45 to 47 | Blossom Invitational, if the autumn invited you |
| 48 | March: third-years graduate; the year ends |

The years never run out. Reputation rises with wins and brings better first-years, and
the region names the school by its fame stars: 無名校 (Unknown), 注目校 (Noticed),
実力校 (Contender), 常連校 (Regular), 伝説校 (Legend). The names are our own.

## Players

Each player has a name, a grade (1 to 3), a position, and stats from 1 to 99, shown as
letter grades (S, A to G):

- Batting: Contact, Power, Speed, Fielding, Arm.
- Pitching (pitchers): Velocity (km/h), Control, Stamina, Breaking ball.

Plus condition (Great, Good, Normal, Poor, Awful), stamina (0 to 100), and weeks hurt.

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

## Calls in a game (round 2)

The game stops at a **chance** (we bat with a runner on second or third) or a **pinch**
(they do). At a chance: swing away, take pitches, bunt, squeeze (runner on third), steal,
or bring in a pinch hitter or pinch runner. At a pinch: go after him, walk him on purpose,
bring in the next pitcher, or make a defensive change. "Play on" lets them play this once;
"no more calls" for the rest of the game. There is still no ball-strike count: clutch
moments (a chance, an extra-base hit, a walk-off) just hold longer, with a bigger face
and a louder line.

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
strength that grows when they beat you and a little every spring. The same week next
year is a rematch.

Winning any game in the autumn tournament invites the club to the **Blossom
Invitational** (春の招待舞台) in March, weeks 45 to 47, before graduation. Winning
the autumn seeds you: the first opponent is easier. It is not a licensed event name.

## School life, thinly

- **Exams.** The old exam event, and a fixed week in January. Mood drops. One or two
  players who are not 鉄の体 get a red mark and cannot play for two weeks — the
  same gate as an injury, not a study minigame.
- **Summer camp** (week 12, before the prefectural tournament). Extra practice in every
  drill, a real chance of a knock, and sometimes a conversation on the steps.
- **Graduation.** The face card is still there. Under each third-year: プロ, 大学, or
  就職, from his numbers, the school's reputation, and his traits. No draft meeting.

## Practice and games are two parts

The week is the practice part: the board of squares round the school's practice ground
(the school building, the net, a batting cage, a bullpen, the running lane), where the club
is drawn as chibi players at their own practice menus. No stadium on the board.

A fixture is the game part, on its own screens:

1. **Game day.** The tournament ribbon, both school crests, team grades, the record between
   the two schools, our starting nine with faces. The coach can change the order, then
   presses Play Ball.
2. **The game, from behind the plate.** A broadcast picture: the stands, the outfield, the
   fielders, the pitcher on the mound (his own face), the batter from behind with his
   number, the catcher, the strike zone. Every plate appearance is shown pitch by pitch,
   with balls, strikes and outs on a TV count box and the speed of the pitch. The pitches
   are drawn from the play (a strikeout ends on a swinging third strike, a walk on ball
   four), so the picture never changes a result. When the ball is hit, the picture
   switches to the field from above with the ball's flight and the runners. Chances and
   pinches stop for the coach's call as before. A tap skips to the result; double speed
   and "to the end" are buttons.
   The coach can also call any batter: the 作戦 button asks for the next plate appearance,
   whoever bats, chance or not.
3. **The result.** Win or loss, the final line score, the player of the game (a chibi
   cheering, with his line), the game experience the club earned (practice points: batting
   for every one of the nine, more per hit; pitching for whoever pitched, more for
   strikeouts; fielding on a win), what it means for the tournament, a word from Kotori,
   then back to practice.

The game-day screen shows their ace (face, name, speed) between the crests: he is the one
who starts.

The roster and the player picker are lists that follow a finger (drag, fling, spring back
at the ends); a tap that hardly moved opens a player. Raising stats pops "+1" by the row
and the new grade ("ミート E→D!") over the page.

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
  the orange accent are the same on every card. The 3D world is only a sky colour.
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
- Still to come: a balance pass. Hit and UI sounds already use the files the other
  games ship.
