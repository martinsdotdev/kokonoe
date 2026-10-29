//! The Level ladder's calibration (spec §6): adjacent Levels play each other until each one's
//! score against the one below is clearly inside or outside 65–75%.
//!
//! ```text
//! cargo run --release --example ladder -- [--preset std|d3] [--pairs 1-8] [--max 400]
//!     [--batch 50] [--threads N] [--set 3:tau=0.8] [--set 5:iterations=4000] [--set 6:most]
//!     [--write]
//! ```
//!
//! Every Level plays as the browser plays it: below Level 5 one search of its whole budget, from
//! Level 5 four searches of a quarter each, as on a device with five or more cores, their visits
//! summed for one choice. The clock never moves, so only the budgets bind and a run doesn't
//! depend on the machine. Seeds come from the pair and the game, so a run is reproducible.
//!
//! A pair plays in batches, the higher Level taking X in even games and O in odd ones, a draw
//! scoring half. After each batch the score's 95% Wilson interval decides: wholly inside 65–75%
//! is in band, wholly below too close, wholly above too far apart, and otherwise another batch,
//! up to `--max` games. A pair still undecided there is named by where its score sits ("in band,
//! unsettled"). Checking after every batch makes the 95% approximate.
//!
//! `--set` tries another rung, a τ, a budget or most visited, without changing the code.
//! `--write` records the Preset's results, and the ladder they were played on, in
//! `crates/engine/LADDER.md`; it refuses `--set` and anything but the default `--pairs`, `--max`
//! and `--batch`, so the file matches the code and the command it names.

use std::fs;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Instant;

use kokonoe_engine::{
    Cell, Choice, Game, Level, Outcome, Player, Preset, Searched, Seed, State, Status, search,
    split,
};

/// The band each Level's score against the one below must sit in.
const BAND: (f64, f64) = (0.65, 0.75);
/// 95%.
const Z: f64 = 1.96;
/// A device with this many cores runs every split Level at its most Workers.
const CORES: u32 = 5;

/// A Level as it plays here: its tactics, with its budget and choice, or ones being tried.
#[derive(Clone, Copy)]
struct Bot {
    level: Level,
    iterations: u32,
    choice: Choice,
}

impl Bot {
    fn new(level: Level) -> Bot {
        Bot {
            level,
            iterations: level.iterations(),
            choice: level.choice(),
        }
    }

    /// Its move in `state`, the side to move, as the browser's Workers and page would find it.
    fn play(&self, state: &State, seed: u64) -> Cell {
        let mut visits = Vec::new();
        let shares = split(self.iterations, self.level.workers(CORES));
        for (share, iterations) in (0..).zip(shares) {
            match search(state, self.level, iterations, mix(seed, share), || 0) {
                Some(Searched::Forced(cell)) => return cell,
                Some(Searched::Visits(found)) => visits.extend(found),
                None => panic!("the game is over"),
            }
        }
        self.choice
            .pick(&visits, mix(seed, u64::MAX))
            .expect("a search lists the moves")
    }
}

/// SplitMix64's finaliser, mixing a seed with a number into another seed.
fn mix(seed: u64, n: u64) -> u64 {
    let mut z = seed ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// How the higher Level fared in one game, and how many plies it took.
enum Result {
    Won,
    Drawn,
    Lost,
}

fn game(preset: Preset, higher: Bot, lower: Bot, seed: u64) -> (Result, u32) {
    let mut game = Game::new(preset.ruleset(), Seed(seed));
    let higher_plays = if seed.is_multiple_of(2) {
        Player::X
    } else {
        Player::O
    };
    loop {
        let state = *game.state();
        let player = match state.status() {
            Status::ToMove { player, .. } => player,
            Status::Over(outcome) => {
                let plies = u32::from(state.ply().get());
                let result = match outcome {
                    Outcome::Draw => Result::Drawn,
                    Outcome::Objective(p) | Outcome::MostWon(p) if p == higher_plays => Result::Won,
                    _ => Result::Lost,
                };
                return (result, plies);
            }
        };
        let bot = if player == higher_plays {
            higher
        } else {
            lower
        };
        let cell = bot.play(&state, mix(seed, u64::from(state.ply().get())));
        game.play(state.ply(), cell)
            .expect("the search plays legal moves");
    }
}

#[derive(Default, Clone, Copy)]
struct Tally {
    won: u32,
    drawn: u32,
    lost: u32,
    plies: u64,
}

impl Tally {
    fn games(&self) -> u32 {
        self.won + self.drawn + self.lost
    }

    /// The higher Level's score, a draw counting half.
    fn score(&self) -> f64 {
        (f64::from(self.won) + f64::from(self.drawn) / 2.0) / f64::from(self.games())
    }

    /// The score's 95% Wilson interval.
    fn interval(&self) -> (f64, f64) {
        let n = f64::from(self.games());
        let p = self.score();
        let centre = (p + Z * Z / (2.0 * n)) / (1.0 + Z * Z / n);
        let half = Z * (p * (1.0 - p) / n + Z * Z / (4.0 * n * n)).sqrt() / (1.0 + Z * Z / n);
        (centre - half, centre + half)
    }

    /// Where the score itself sits, for a pair whose interval still crosses a band edge at the
    /// limit.
    fn leaning(&self) -> &'static str {
        let score = self.score();
        if score < BAND.0 {
            "too close, unsettled"
        } else if score > BAND.1 {
            "too far apart, unsettled"
        } else {
            "in band, unsettled"
        }
    }

    fn verdict(&self) -> Option<&'static str> {
        let (low, high) = self.interval();
        if low >= BAND.0 && high <= BAND.1 {
            Some("in band")
        } else if high < BAND.0 {
            Some("too close")
        } else if low > BAND.1 {
            Some("too far apart")
        } else {
            None
        }
    }
}

struct Options {
    preset: Preset,
    pairs: (u8, u8),
    max: u32,
    batch: u32,
    threads: u32,
    bots: Vec<Bot>,
    overridden: bool,
    write: bool,
}

fn options() -> Options {
    let mut options = Options {
        preset: Preset::Standard,
        pairs: (Level::MIN, Level::MAX),
        max: 400,
        batch: 50,
        threads: thread::available_parallelism().map_or(1, |n| n.get() as u32),
        bots: (Level::MIN..=Level::MAX)
            .map(|n| Bot::new(Level::try_from(n).expect("a Level")))
            .collect(),
        overridden: false,
        write: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{arg} wants a value"));
        match arg.as_str() {
            "--preset" => {
                options.preset = match value().as_str() {
                    "std" => Preset::Standard,
                    "d3" => Preset::Deep,
                    other => panic!("--preset is std or d3, not {other}"),
                }
            }
            "--pairs" => {
                let range = value();
                let (from, to) = range.split_once('-').expect("--pairs is like 1-8");
                options.pairs = (from.parse().expect("a Level"), to.parse().expect("a Level"));
            }
            "--max" => options.max = value().parse().expect("a number of games"),
            "--batch" => options.batch = value().parse().expect("a number of games"),
            "--threads" => options.threads = value().parse().expect("a number of threads"),
            "--set" => {
                let set = value();
                let (level, change) = set.split_once(':').expect("--set is like 3:tau=0.8");
                let level: usize = level.parse().expect("a Level");
                let bot = &mut options.bots[level - 1];
                match change.split_once('=') {
                    Some(("tau", tau)) => {
                        bot.choice = Choice::Sample {
                            tau: tau.parse().expect("a τ"),
                        }
                    }
                    Some(("iterations", n)) => bot.iterations = n.parse().expect("iterations"),
                    None if change == "most" => bot.choice = Choice::MostVisited,
                    _ => panic!("--set takes tau=…, iterations=… or most"),
                }
                options.overridden = true;
            }
            "--write" => options.write = true,
            other => panic!("unknown option {other}"),
        }
    }
    assert!(
        !(options.write && options.overridden),
        "--write records the code's ladder, so it refuses --set"
    );
    assert!(
        !options.write || options.pairs == (Level::MIN, Level::MAX),
        "--write records every pair, so it plays Levels 1-8"
    );
    assert!(
        !options.write || (options.max, options.batch) == (400, 50),
        "--write records the command it was played by, so it keeps --max 400 and --batch 50"
    );
    options
}

/// Plays `higher` against `lower` in batches until the verdict is clear or `max` games are played.
fn pair(options: &Options, higher: Bot, lower: Bot) -> Tally {
    let tally = Mutex::new(Tally::default());
    let pair_seed = mix(
        u64::from(lower.level.get()),
        u64::from(options.preset as u8),
    );
    let mut played = 0;
    while played < options.max {
        let end = (played + options.batch).min(options.max);
        let next = AtomicU32::new(played);
        thread::scope(|scope| {
            for _ in 0..options.threads {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= end {
                            break;
                        }
                        // Even games give the higher Level X, odd ones O.
                        let seed = (mix(pair_seed, u64::from(index)) & !1) | u64::from(index % 2);
                        let (result, plies) = game(options.preset, higher, lower, seed);
                        let mut tally = tally.lock().expect("no thread panicked");
                        match result {
                            Result::Won => tally.won += 1,
                            Result::Drawn => tally.drawn += 1,
                            Result::Lost => tally.lost += 1,
                        }
                        tally.plies += u64::from(plies);
                    }
                });
            }
        });
        played = end;
        if tally
            .lock()
            .expect("no thread panicked")
            .verdict()
            .is_some()
        {
            break;
        }
    }
    tally.into_inner().expect("no thread panicked")
}

fn describe(bot: &Bot) -> String {
    match bot.choice {
        Choice::Sample { tau } => format!("τ {tau}"),
        Choice::MostVisited => "most visited".to_string(),
    }
}

/// `1234567` as `1,234,567`.
fn grouped(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn percent(p: f64) -> String {
    format!("{:.1}%", p * 100.0)
}

/// The ladder table, as `LADDER.md` and the test that holds it to the code read it.
fn ladder_table(bots: &[Bot]) -> String {
    let mut table = String::from(
        "| Level | Iterations | Choice | Tactics | Workers |\n| --: | --: | :-- | :-- | :-- |\n",
    );
    for bot in bots {
        let workers = if bot.level.workers(CORES) > 1 {
            "up to 4"
        } else {
            "1"
        };
        let tactics = if bot.level.tactics() { "yes" } else { "no" };
        table.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            bot.level.get(),
            grouped(bot.iterations),
            describe(bot),
            tactics,
            workers
        ));
    }
    table
}

/// Replaces the body of `## {heading}` in `text`, up to the next `## ` heading.
fn replace_section(text: &str, heading: &str, body: &str) -> String {
    let marker = format!("## {heading}\n");
    let start = text.find(&marker).expect("LADDER.md has the section") + marker.len();
    let end = text[start..]
        .find("\n## ")
        .map_or(text.len(), |at| start + at);
    format!("{}\n{}\n{}", &text[..start], body.trim_end(), &text[end..])
}

fn main() {
    let options = options();
    let name = match options.preset {
        Preset::Deep => "Deep",
        _ => "Standard",
    };
    println!(
        "{name}: Levels {}–{}, up to {} games a pair in batches of {}, {} threads",
        options.pairs.0, options.pairs.1, options.max, options.batch, options.threads
    );
    let mut rows = String::from(
        "| Pair | Games | Won | Drawn | Lost | Score | 95% interval | Verdict | Plies a game |\n\
         | :-- | --: | --: | --: | --: | --: | :-- | :-- | --: |\n",
    );
    for upper in (options.pairs.0 + 1)..=options.pairs.1 {
        let (higher, lower) = (
            options.bots[usize::from(upper) - 1],
            options.bots[usize::from(upper) - 2],
        );
        let start = Instant::now();
        let tally = pair(&options, higher, lower);
        let (low, high) = tally.interval();
        let verdict = tally.verdict().unwrap_or_else(|| tally.leaning());
        let plies = tally.plies as f64 / f64::from(tally.games());
        let row = format!(
            "| {upper} over {} | {} | {} | {} | {} | {} | {}–{} | {verdict} | {plies:.0} |\n",
            upper - 1,
            tally.games(),
            tally.won,
            tally.drawn,
            tally.lost,
            percent(tally.score()),
            percent(low),
            percent(high),
        );
        print!("{row}");
        println!(
            "    {:.1} s, {:.2} s a game",
            start.elapsed().as_secs_f64(),
            start.elapsed().as_secs_f64() / f64::from(tally.games())
        );
        rows.push_str(&row);
    }
    if options.write {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/LADDER.md");
        let text = fs::read_to_string(path).expect("LADDER.md reads");
        let text = replace_section(&text, "The ladder", &ladder_table(&options.bots));
        let command = format!(
            "Played by `cargo run --release --example ladder -- --preset {} --pairs {}-{} --write`.\n\n",
            if name == "Deep" { "d3" } else { "std" },
            options.pairs.0,
            options.pairs.1
        );
        let text = replace_section(&text, name, &format!("{command}{rows}"));
        fs::write(path, text).expect("LADDER.md writes");
        println!("Wrote {path}");
    }
}
