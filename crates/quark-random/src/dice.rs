use crate::rng::Rng;

const MAX_DICE: i64 = 1000;
const MAX_SIDES: i64 = 1_000_000;

enum Keep {
    All,
    Highest(usize),
    Lowest(usize),
}

pub fn roll(rng: &mut Rng, notation: &str) -> Result<(i64, Vec<i64>), String> {
    let compact: String = notation
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    if compact.is_empty() {
        return Err("the dice notation is empty".to_owned());
    }

    let mut total: i64 = 0;
    let mut rolls = Vec::new();
    let mut rest = compact.as_str();
    let mut first = true;

    while !rest.is_empty() {
        let sign = match rest.as_bytes()[0] {
            b'+' => {
                rest = &rest[1..];
                1
            }
            b'-' => {
                rest = &rest[1..];
                -1
            }
            _ if first => 1,
            _ => return Err(format!("expected + or - in '{notation}'")),
        };
        first = false;

        let end = rest.find(['+', '-']).unwrap_or(rest.len());
        let term = &rest[..end];
        rest = &rest[end..];
        if term.is_empty() {
            return Err(format!("missing term in '{notation}'"));
        }

        let Some((count, die)) = term.split_once('d') else {
            let value: i64 = term
                .parse()
                .map_err(|_| format!("'{term}' is not a number or a dice term"))?;
            total += sign * value;
            continue;
        };

        let count: i64 = if count.is_empty() {
            1
        } else {
            count
                .parse()
                .map_err(|_| format!("invalid dice count in '{term}'"))?
        };
        let (sides, keep) = match die
            .split_once("kh")
            .map(|(sides, keep)| (sides, keep, true))
            .or_else(|| {
                die.split_once("kl")
                    .map(|(sides, keep)| (sides, keep, false))
            }) {
            Some((sides, keep, highest)) => {
                let keep: usize = keep
                    .parse()
                    .map_err(|_| format!("invalid keep count in '{term}'"))?;
                (
                    sides,
                    if highest {
                        Keep::Highest(keep)
                    } else {
                        Keep::Lowest(keep)
                    },
                )
            }
            None => (die, Keep::All),
        };
        let sides: i64 = if sides == "%" {
            100
        } else {
            sides
                .parse()
                .map_err(|_| format!("invalid number of sides in '{term}'"))?
        };

        if !(1..=MAX_DICE).contains(&count) {
            return Err(format!(
                "between 1 and {MAX_DICE} dice can be rolled at once"
            ));
        }
        if !(1..=MAX_SIDES).contains(&sides) {
            return Err(format!("dice need between 1 and {MAX_SIDES} sides"));
        }

        let mut results: Vec<i64> = (0..count).map(|_| rng.int(1, sides)).collect();
        rolls.extend(results.iter().copied());
        match keep {
            Keep::All => {}
            Keep::Highest(keep) => {
                results.sort_unstable_by(|left, right| right.cmp(left));
                results.truncate(keep);
            }
            Keep::Lowest(keep) => {
                results.sort_unstable();
                results.truncate(keep);
            }
        }
        total += sign * results.iter().sum::<i64>();
    }

    Ok((total, rolls))
}
