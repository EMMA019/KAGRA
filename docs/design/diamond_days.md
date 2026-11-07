# 白球デイズ (Diamond Days): design notes

An original high-school baseball manager for phones. You are the new coach of a small
school's baseball club. Over three years you train the team week by week, play practice
games and tournaments, and aim for the national championship at the "Summer Stadium".
Players graduate every March and first-years join in April, so the club keeps changing.

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
| 13 to 16 | Summer prefectural tournament (knockout, up to 5 games) |
| 17 to 18 | Summer Stadium, the national tournament, if you won the prefecture |
| 25 to 27 | Autumn tournament (up to 3 games) |
| 48 | March: third-years graduate; the year ends |

Three years make a full run. Reputation rises with wins and brings better first-years.

## Players

Each player has a name, a grade (1 to 3), a position, and stats from 1 to 99, shown as
letter grades (S, A to G):

- Batting: Contact, Power, Speed, Fielding, Arm.
- Pitching (pitchers): Velocity (km/h), Control, Stamina, Breaking ball.

Plus condition (Great, Good, Normal, Poor, Awful), stamina (0 to 100), and weeks hurt.

## Games

A game is simulated plate appearance by plate appearance from the players' stats. Before
each of your batting innings you pick an order: **Swing away** (more power, more
strikeouts), **Normal**, or **Small ball** (bunts and steals move runners). The pitcher
tires; when stamina runs low the next pitcher comes in. You watch the play-by-play line
by line (tap to hurry) on a scoreboard.

## Rounds

- **Round 1 (this PR):** the week loop (cards, board, training, rest, events, fatigue,
  injuries), the team (12 to 16 players, generated names), the calendar with practice
  games, the summer tournament and the autumn tournament, the game sim with orders,
  graduation and first-years, reputation, save on the device.
- Round 2: the national tournament, better events with choices (dialogue), the coach's
  own growth, rival schools with names, records and an end-of-three-years card.
- Round 3: the picture (players on the field during a game, a ball), sound, balance.
