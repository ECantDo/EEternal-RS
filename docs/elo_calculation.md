# Computing the Engine Elo

The ELO has been mostly calculated based on engines that played in
[Sebastian Lague's Chess Challenge](https://github.com/SebLague/Chess-Challenge),
[Ratings List](https://github.com/SebLague/Tiny-Chess-Bot-Challenge-Results/blob/main/RatingsList.txt)
of the Swiss and Knockout games played. The ELOs are based on the
[Boychesser](https://github.com/analog-hors/Boychesser) engine (ELO: 2772 +/- 11, at the time).

Or I have found other engines that are similarly rated based
on [CCRL](https://computerchess.org.uk/404/cgi/compare_engines.cgi?class=Single-CPU+engines&only_best_in_class=on)
ratings, for example, because I thought it would be fun, I decided to pit it
against [Reckless](https://github.com/codedeliveryservice/Reckless), but in its earlier forms.

Games are played with a time control of `10+0.1s`, and with an opening book of 8 moves, each engine gets the chance to
play as both white and black, from the same position.

_As of V0.0.4_

> The ELO is calculated based on assuming that the *guessed* Elo ratings are the *true* ratings. That leads into the elo
> ratings are still approximate, but can still be wildly off. The more games that are played, the better approximation
> that we can get, relative to the assumed true ELO ratings of the played bots.

However, even though most of the Elo ratings are assumed to be true, the higher the engine Elo in the tournament, the
lesser of the margin of error of the assumed. Namely, most of the engines `< 2400` Elo, the margin of error is ~
`+/- 80`. Compared to the top where it is closer to `+/- 20` (or better), or using external engines to the tournament,
like Reckless.

Importantly don't take the Elo of this engine for **absolute** truth, since there is a lot of error baked in, but in my
opinion, it is still decently close to what it should be. Based on playing some 1000 games per opposing engine.

This below script is used to actually calculate the Elo difference between my engine, and the known Elo of the competing
engines. The critical information that this script needs is the Elo of the faced engines, and the WLD (Win - Loss -
Draw) numbers.

Since the engine is tested against multiple opponents, each matchup produces an independent Elo estimate. These are
combined using inverse-variance weighting so that matchups with smaller uncertainty contribute more heavily to the final
estimate.

```py
import math


def calc_error(win_rate: tuple[int, int, int], p: float):
	N = sum(win_rate)
	p_w = win_rate[0] / N
	p_d = win_rate[2] / N
	p_l = win_rate[1] / N

	variance = p_w * ((1 - p) ** 2) + p_d * ((0.5 - p) ** 2) + p_l * ((0 - p) ** 2)
	std_err = (variance / N) ** 0.5

	p_low = p - 1.95996 * std_err
	p_high = p + 1.95996 * std_err

	elo_low = 400 * math.log10(p_low / (1 - p_low))
	elo_high = 400 * math.log10(p_high / (1 - p_high))

	width = elo_high - elo_low
	pm = width / 2

	return pm


def calculate_elo(input_engine_elo, wld):
	score_percentage: list[float] = [(x[0] + 0.5 * x[2]) / sum(x) for x in wld]

	delta_elo: list[float] = [400 * math.log10(p / (1 - p)) for p in score_percentage]

	estimated_elo = [sum(x) for x in zip(input_engine_elo, delta_elo)]

	elo_error = [calc_error(wld[idx], p) for idx, p in enumerate(score_percentage)]

	# Calc elo POOLED

	weights: list[float] = [1 / (err ** 2) for err in elo_error]

	pooled_elo = sum(val[0] * val[1] for val in zip(weights, estimated_elo)) / sum(weights)
	pooled_error = 1 / (sum(weights) ** 0.5)

	print(f"{round(pooled_elo)} +/- {round(pooled_error)}")


def main():
	input_engine_elo: list[int] = [...]

	wld: list[tuple[int, int, int]] = [...]

	calculate_elo(input_engine_elo, wld)
	pass


if __name__ == "__main__":
	main()
```

And as a more recent addition, with changing how I run the tests, using the CuteChess gauntlet tournament mode. This
script takes in the CuteChess output as input, and counts the number of Wins, Losses, and Draws, relative to my engine.
As an example, an input of:

```terminaloutput
Player: Reckless 0.3.0
   "Draw by 3-fold repetition": 91
   "Draw by fifty moves rule": 47
   "Draw by insufficient mating material": 52
   "Draw by stalemate": 2
   "Loss: Black mates": 158
   "Loss: White mates": 294
   "Win: Black mates": 127
   "Win: White mates": 229
```

Would count the `356` wins it had, as losses vs my engine. Which is the number I care about, since I want to plug in
that my engine lost to Reckless 0.3.0 356 times.

The program also assumes that your engine that you're testing is the first engine in the list, and ignores it when
handing it off to the above Elo calculation script.

```python
from enum import IntEnum
import elo_calc

games_input = """
<Game Report Here>
"""
engine_elos = [...]


class NextType(IntEnum):
	Nothing = 0
	Player = 1
	Draw = 2
	Loss = 3
	Win = 4
	pass


def new_player(player_name: str):
	return {
		"name": player_name,
		"wins": 0,
		"losses": 0,
		"draws": 0
	}


def build_wld_tuple(player: dict) -> tuple[int, int, int]:
	return player['losses'], player['wins'], player['draws']


def print_player(player: dict):
	print(f"{player['name']}\n\tLWD: {build_wld_tuple(player)}")


def parse_input():
	global games_input
	players: list[dict] = []
	next_expect: NextType = NextType.Nothing

	games_input = games_input.replace('"', "")
	games_input = games_input.replace(":", "")

	for part in games_input.split():
		if not isinstance(part, str):
			continue

		if part == "Player":
			next_expect = NextType.Player
			continue
		if part == "Draw":
			next_expect = NextType.Draw
			continue
		if part == "Loss":
			next_expect = NextType.Loss
			continue
		if part == "Win":
			next_expect = NextType.Win
			continue

		if next_expect == NextType.Player:
			players.append(new_player(part))
			next_expect = NextType.Nothing
			continue

		if not part.isnumeric():
			continue

		num = int(part)

		if next_expect == NextType.Draw:
			players[-1]["draws"] += num
		elif next_expect == NextType.Loss:
			players[-1]["losses"] += num
		elif next_expect == NextType.Win:
			players[-1]["wins"] += num

		next_expect = NextType.Nothing
		pass  # End of `for` loop

	for player in players:
		print_player(player)
	return players
	pass  # End of `parse_input`


def main():
	players = parse_input()[1:]
	wlds = [build_wld_tuple(player) for player in players]
	elo_calc.calculate_elo(engine_elos, wlds)
	pass


if __name__ == "__main__":
	main()
```

And lastly, this is how I am running my engine tests, not much to say, but here it is.

```shell
#!/bin/bash

date=$(date '+%Y-%m-%d-%H-%M-%S')

CUTE="cutechess-cli"
BOT_NAME="EEternalRS"
BOT_VERSION="0_0_10"

# Add opponents here
OPPONENTS=(
    "Drip leab:./drip_leab-2602"
)

ENGINE_ARGS=(-engine name="$BOT_NAME-$BOT_VERSION" cmd="./$eeternal_rs_v$BOT_VERSION")

for entry in "${OPPONENTS[@]}"; do
    name="${entry%%:*}"
    cmd="${entry#*:}"
    ENGINE_ARGS+=(-engine name="$name" cmd="$cmd")
done

$CUTE \
    "${ENGINE_ARGS[@]}" \
    -each proto=uci tc=inf/10+0.1 \
    -openings file=UHO_2024_8mvs_big_+085_+104.epd format=epd order=random \
    -tournament gauntlet \
    -rounds 500 \
    -games 2 \
    -repeat \
    -ratinginterval 16 \
    -concurrency 20 \
    -pgnout "EloTesting-$date.pgn" \
    -event "Elo Gauntlet | $BOT_NAME-$BOT_VERSION"

echo ""
echo "Gauntlet finished."
```