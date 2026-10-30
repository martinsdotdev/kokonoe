//! The golden Record corpus (spec §3.4, ADR-0004): games checked into `tests/corpus/v<N>/`, one
//! JSON file each, holding a Record and what it must keep producing: the dealt opening, the Sent
//! target and the legal Cells at every ply in order, and the outcome. Every build replays them, so
//! a change to any of those fails here unless `RULES_VERSION` is bumped as `RULES_VERSION.md`
//! describes. The ignored `write_the_corpus` adds games, never rewriting one.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{Xorshift, advance, cells};
use kokonoe_engine::{
    Board, BoardStatus, Child, Depth, Fallback, Game, Objective, Opening, Outcome, Player, Ply,
    Preset, RULES_VERSION, Record, Ruleset, Seed, Sent, Space, State, Status,
};
use serde_json::Value;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/corpus");

/// One corpus file.
struct Entry {
    record: Record,
    opening: Vec<u16>,
    /// `"X by objective"`, `"O by most won"`, `"draw"`, or `None` while the game goes on.
    outcome: Option<String>,
    /// Every ply with a side to move, opening plies included.
    turns: Vec<Turn>,
}

/// One ply with a side to move: where it was Sent, and the legal Cells there in order.
#[derive(PartialEq)]
struct Turn {
    sent: String,
    legal: Vec<u16>,
}

/// What `game` produces, as a corpus file records it.
fn entry(game: &Game) -> Entry {
    let last = game.state().ply().get();
    let turns = (0..=last)
        .filter_map(|ply| match game.state_at(Ply::new(ply))?.status() {
            Status::ToMove { sent, moves, .. } => Some(Turn {
                sent: sent_words(sent),
                legal: moves.iter().map(|cell| cell.index()).collect(),
            }),
            Status::Over(_) => None,
        })
        .collect();
    Entry {
        record: game.record(),
        opening: game.opening().iter().map(|cell| cell.index()).collect(),
        outcome: ended(game.state()).map(outcome_words),
        turns,
    }
}

fn outcome_words(outcome: Outcome) -> String {
    let side = |player| match player {
        Player::X => "X",
        Player::O => "O",
    };
    match outcome {
        Outcome::Objective(player) => format!("{} by objective", side(player)),
        Outcome::MostWon(player) => format!("{} by most won", side(player)),
        Outcome::Draw => "draw".to_string(),
    }
}

/// `"first move"`, `"to 4"`, or `"free: 47 within 4"`, a Board written as its Space digits.
fn sent_words(sent: Sent) -> String {
    let board = |board: Board| match board.parent() {
        None => "global".to_string(),
        Some(_) => board
            .spaces()
            .iter()
            .map(|s| s.index().to_string())
            .collect(),
    };
    match sent {
        Sent::FirstMove => "first move".to_string(),
        Sent::To(target) => format!("to {}", board(target)),
        Sent::Free { target, within } => {
            format!("free: {} within {}", board(target), board(within))
        }
    }
}

fn read(path: &Path) -> Entry {
    let text = fs::read_to_string(path).unwrap();
    let mut file: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let turns = file["turns"].as_array().map_or(&[][..], Vec::as_slice);
    let turns = turns
        .iter()
        .map(|turn| Turn {
            sent: turn["sent"].as_str().unwrap_or_default().to_string(),
            legal: serde_json::from_value(turn["legal"].clone()).unwrap(),
        })
        .collect();
    Entry {
        record: serde_json::from_value(file["record"].take())
            .unwrap_or_else(|e| panic!("{path:?}: {e}")),
        opening: serde_json::from_value(file["opening"].take()).unwrap(),
        outcome: file["outcome"].as_str().map(String::from),
        turns,
    }
}

/// The record on one line and one turn per line, so a diff reads ply by ply.
fn json(entry: &Entry) -> String {
    let list = |cells: &[u16]| {
        let cells: Vec<String> = cells.iter().map(u16::to_string).collect();
        format!("[{}]", cells.join(", "))
    };
    let turns: Vec<String> = entry
        .turns
        .iter()
        .map(|turn| {
            format!(
                "    {{\"sent\": \"{}\", \"legal\": {}}}",
                turn.sent,
                list(&turn.legal)
            )
        })
        .collect();
    let outcome = match &entry.outcome {
        Some(words) => format!("\"{words}\""),
        None => "null".to_string(),
    };
    format!(
        "{{\n  \"record\": {},\n  \"opening\": {},\n  \"outcome\": {outcome},\n  \"turns\": [\n{}\n  ]\n}}\n",
        serde_json::to_string(&entry.record).unwrap(),
        list(&entry.opening),
        turns.join(",\n"),
    )
}

/// Every corpus file by its slice's version, which must run from 1 to `RULES_VERSION`. Anything
/// but a `v<N>` folder, or a JSON file inside one, is left alone.
fn corpus() -> Vec<(u32, PathBuf)> {
    let mut versions = Vec::new();
    let mut files = Vec::new();
    for slice in fs::read_dir(CORPUS).unwrap() {
        let slice = slice.unwrap().path();
        if !slice.is_dir() {
            continue;
        }
        let name = slice.file_name().unwrap().to_str().unwrap();
        let version: u32 = name
            .strip_prefix('v')
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("{name} is not a slice like v1"));
        versions.push(version);
        for file in fs::read_dir(&slice).unwrap() {
            let file = file.unwrap().path();
            if file
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                files.push((version, file));
            }
        }
    }
    versions.sort();
    assert_eq!(
        versions,
        (1..=RULES_VERSION).collect::<Vec<_>>(),
        "a slice for every RULES_VERSION"
    );
    files.sort();
    files
}

/// Where a replay first differs from what was recorded.
fn difference(recorded: &Entry, replayed: &Entry) -> Option<String> {
    if replayed.opening != recorded.opening {
        return Some(format!(
            "the deal is {:?}, recorded {:?}",
            replayed.opening, recorded.opening
        ));
    }
    let turn = |entry: &Entry, ply: usize| match entry.turns.get(ply) {
        Some(turn) => format!("Sent {}, legal {:?}", turn.sent, turn.legal),
        None => "none, the game is over".to_string(),
    };
    let plies = recorded.turns.len().max(replayed.turns.len());
    if let Some(ply) = (0..plies).find(|&ply| recorded.turns.get(ply) != replayed.turns.get(ply)) {
        return Some(format!(
            "ply {ply}: {}; recorded {}",
            turn(replayed, ply),
            turn(recorded, ply)
        ));
    }
    let outcome = |entry: &Entry| entry.outcome.as_deref().unwrap_or("unfinished").to_string();
    (replayed.outcome != recorded.outcome).then(|| {
        format!(
            "the outcome is {}, recorded {}",
            outcome(replayed),
            outcome(recorded)
        )
    })
}

#[test]
fn every_record_replays_as_it_was_recorded() {
    let mut failures = Vec::new();
    for (version, path) in corpus() {
        let recorded = read(&path);
        let name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(recorded.record.rules_version, version, "{name}'s slice");
        match Game::open(&recorded.record) {
            Ok(game) => {
                if let Some(difference) = difference(&recorded, &entry(&game)) {
                    failures.push(format!("v{version}/{name}: {difference}"));
                }
            }
            Err(error) => failures.push(format!("v{version}/{name}: {error}")),
        }
    }
    assert!(
        failures.is_empty(),
        "a Record replays differently, so the rules changed; see RULES_VERSION.md:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_game_the_writer_knows_has_its_file() {
    let slice = Path::new(CORPUS).join(format!("v{RULES_VERSION}"));
    let missing: Vec<_> = names()
        .into_iter()
        .filter(|name| !slice.join(name).exists())
        .collect();
    assert!(
        missing.is_empty(),
        "v{RULES_VERSION} has no file for {missing:?}: a corpus file is never deleted, and a new \
         game is written with cargo test --test corpus -- --ignored"
    );
}

/// What the corpus must hold at least one game of.
#[derive(Debug, PartialEq)]
enum Case {
    Depth(Depth),
    RandomOpening(Depth),
    Unfinished,
    OverInsideItsOpening,
    Ended(Termination),
    Preset(Preset, Kind),
}

/// Spec §4.4's Termination for an Outcome the engine decides.
#[derive(Debug, PartialEq)]
enum Termination {
    Line,
    Count,
    Decided,
}

#[derive(Debug, PartialEq)]
enum Kind {
    Objective,
    MostWon,
    Draw,
}

fn termination(rules: &Ruleset, outcome: Outcome) -> Termination {
    match (outcome, rules.objective, rules.fallback) {
        (Outcome::Objective(_), Objective::Line, _) => Termination::Line,
        (Outcome::Objective(_), Objective::FirstTo(_), _) | (Outcome::MostWon(_), _, _) => {
            Termination::Count
        }
        (Outcome::Draw, _, Fallback::MostWon) => Termination::Count,
        // Every child of the Global board is decided, and the fallback is a draw.
        (Outcome::Draw, _, Fallback::Draw) => Termination::Decided,
    }
}

fn kind(outcome: Outcome) -> Kind {
    match outcome {
        Outcome::Objective(_) => Kind::Objective,
        Outcome::MostWon(_) => Kind::MostWon,
        Outcome::Draw => Kind::Draw,
    }
}

#[test]
fn the_corpus_covers_the_rules() {
    let mut covered = Vec::new();
    for (_, path) in corpus() {
        let game =
            Game::open(&read(&path).record).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        let rules = game.record().ruleset;
        covered.push(Case::Depth(rules.depth));
        if matches!(rules.opening, Opening::Random(_)) {
            covered.push(Case::RandomOpening(rules.depth));
        }
        match ended(game.state()) {
            None => covered.push(Case::Unfinished),
            Some(outcome) => {
                covered.push(Case::Ended(termination(&rules, outcome)));
                if let Some(preset) = Preset::of(&rules) {
                    covered.push(Case::Preset(preset, kind(outcome)));
                }
                if game.moves().is_empty() && !game.opening().is_empty() {
                    covered.push(Case::OverInsideItsOpening);
                }
            }
        }
    }
    let mut wanted = vec![
        Case::Unfinished,
        Case::OverInsideItsOpening,
        Case::Ended(Termination::Line),
        Case::Ended(Termination::Count),
        Case::Ended(Termination::Decided),
    ];
    for depth in [Depth::Two, Depth::Three] {
        wanted.push(Case::Depth(depth));
        wanted.push(Case::RandomOpening(depth));
    }
    for preset in Preset::ALL {
        wanted.push(Case::Preset(preset, Kind::Objective));
        wanted.push(Case::Preset(preset, Kind::Draw));
        if preset.ruleset().fallback == Fallback::MostWon {
            wanted.push(Case::Preset(preset, Kind::MostWon));
        }
    }
    let missing: Vec<_> = wanted
        .iter()
        .filter(|case| !covered.contains(case))
        .collect();
    assert!(missing.is_empty(), "the corpus has no game for {missing:?}");
}

/// A game the tests already pin, copied from the test file named above it.
struct Named {
    name: &'static str,
    /// The Ruleset code.
    rules: &'static str,
    seed: u64,
    /// The moves as digits, none for a game that is only its deal.
    moves: &'static str,
}

const NAMED: [Named; 14] = [
    // standard.rs
    Named {
        name: "top-row",
        rules: "std",
        seed: 0,
        moves: "03 30 04 40 05 51 13 31 14 41 15 52 26 62 27 72 28",
    },
    Named {
        name: "drawn",
        rules: "std",
        seed: 0,
        moves: "82 21 12 27 72 24 46 64 42 78 85 54 44 11 16 68 87 70 03 32 65 56 \
                62 01 14 67 77 71 38 83 36 63 34 07 76 66 84 04 37 74 80 52 81",
    },
    // settings.rs
    Named {
        name: "opponent-line",
        rules: "both",
        seed: 0,
        moves: "82 27 70 07 72 28 81 14 47 76 67 71 12 20 00 08 86 62 21 10 \
                01 18 80 03 32 26 63 36 65 50 05 51 60 02 04 42 06",
    },
    Named {
        name: "double-line",
        rules: "both",
        seed: 0,
        moves: "42 28 81 14 41 17 71 11 60 07 70 04 40 02 24 57 77 72 27 75 \
                50 06 62 25 54 56 61 33 37 74 22 20 82 26 23 31 32 21",
    },
    Named {
        name: "line-of-three",
        rules: "first4",
        seed: 0,
        moves: "74 41 13 35 56 64 47 77 75 57 73 31 10 01 16 67 48 84 46",
    },
    Named {
        name: "unequal",
        rules: "most",
        seed: 0,
        moves: "04 47 76 67 72 23 34 44 40 07 78 80 08 84 46 64 48 88 22 27 75 \
                51 12 26 65 57 00 54 43 30 35 17 18 14 33 11 28 61 21 24 20",
    },
    Named {
        name: "equal",
        rules: "most",
        seed: 0,
        moves: "03 30 04 47 75 55 52 23 38 80 05 57 73 34 44 46 62 26 60 84 42 \
                20 81 16 61 14 48 88 50 33 35 54 43 31 18 40 74 45 51 12 41 37",
    },
    Named {
        name: "full-and-won",
        rules: "cont",
        seed: 0,
        moves: "88 80 08 82 28 86 68 85 58 87 77 71 18 81 17 78 83 38 84 48",
    },
    // deep.rs
    Named {
        name: "line-of-middle-boards",
        rules: "d3",
        seed: 0,
        moves: LINE_OF_MIDDLE_BOARDS,
    },
    Named {
        name: "into-a-won-board",
        rules: "d3",
        seed: 0,
        moves: "257 576 765 653 537 376 763 636 367 676 764 640 407 076",
    },
    Named {
        name: "dealt",
        rules: "d3+rand2",
        seed: 22,
        moves: LINE_OF_MIDDLE_BOARDS,
    },
    // openings.rs: Seed 490's deal, under two Rulesets, and under FirstTo(1), where it ends the
    // game.
    Named {
        name: "seed-490",
        rules: "rand8",
        seed: 490,
        moves: "",
    },
    Named {
        name: "seed-490",
        rules: "cont+rand8",
        seed: 490,
        moves: "",
    },
    Named {
        name: "ends-in-opening",
        rules: "first1+rand8",
        seed: 490,
        moves: "",
    },
];

const LINE_OF_MIDDLE_BOARDS: &str = "280 803 031 313 131 315 151 515 150 504 042 427 271 714 \
    142 426 262 627 272 728 281 815 152 527 270 703 032 327 261 615 130 304 041 413 132 328 \
    282 827 260 604 040 403 030 305 052 522 050 503 051 513 140 405 141";

/// A game to find by random play: the first whose final State `wanted` keeps.
struct Searched {
    name: &'static str,
    /// The Ruleset code.
    rules: &'static str,
    wanted: fn(&State) -> bool,
}

fn ended(state: &State) -> Option<Outcome> {
    match state.status() {
        Status::Over(outcome) => Some(outcome),
        Status::ToMove { .. } => None,
    }
}

fn objective(state: &State) -> bool {
    matches!(ended(state), Some(Outcome::Objective(_)))
}

fn most_won(state: &State) -> bool {
    matches!(ended(state), Some(Outcome::MostWon(_)))
}

fn drawn(state: &State) -> bool {
    ended(state) == Some(Outcome::Draw)
}

/// The children of the Global board `player` won outright, without drawn ones counting.
fn won_outright(state: &State, player: Player) -> usize {
    let global = Board::global(state.ruleset().depth);
    Space::ALL
        .into_iter()
        .filter(|&space| match global.child(space) {
            Child::Board(child) => state.board_status(child) == BoardStatus::Won(player),
            Child::Cell(_) => false,
        })
        .count()
}

const SEARCHED: [Searched; 18] = [
    Searched {
        name: "o-wins",
        rules: "std",
        wanted: |state| ended(state) == Some(Outcome::Objective(Player::O)),
    },
    Searched {
        name: "objective",
        rules: "cont",
        wanted: objective,
    },
    Searched {
        name: "drawn",
        rules: "cont",
        wanted: drawn,
    },
    Searched {
        name: "objective",
        rules: "cont+first5+most",
        wanted: objective,
    },
    Searched {
        name: "most-won",
        rules: "cont+first5+most",
        wanted: most_won,
    },
    Searched {
        name: "equal",
        rules: "cont+first5+most",
        wanted: drawn,
    },
    Searched {
        name: "line",
        rules: "most",
        wanted: objective,
    },
    Searched {
        name: "most-won",
        rules: "both+most",
        wanted: most_won,
    },
    Searched {
        name: "objective",
        rules: "cont+both",
        wanted: objective,
    },
    // FirstTo reached only by counting a drawn child.
    Searched {
        name: "counts-a-drawn-board",
        rules: "both+first5",
        wanted: |state| match ended(state) {
            Some(Outcome::Objective(winner)) => {
                won_outright(state, winner) < usize::from(state.won_count(winner))
            }
            _ => false,
        },
    },
    Searched {
        name: "objective",
        rules: "rand4",
        wanted: objective,
    },
    Searched {
        name: "drawn",
        rules: "d3",
        wanted: drawn,
    },
    Searched {
        name: "most-won",
        rules: "d3+most",
        wanted: most_won,
    },
    Searched {
        name: "objective",
        rules: "d3+both",
        wanted: objective,
    },
    Searched {
        name: "drawn",
        rules: "d3+both",
        wanted: drawn,
    },
    Searched {
        name: "objective",
        rules: "d3+cont",
        wanted: objective,
    },
    Searched {
        name: "drawn",
        rules: "d3+cont",
        wanted: drawn,
    },
    // Drawn with Cells still empty, so a middle Board was decided by a drawn Local board.
    Searched {
        name: "drawn-before-full",
        rules: "d3+cont",
        wanted: |state| drawn(state) && state.ply().get() < 729,
    },
];

/// The first random game under `rules` that `wanted` keeps, its moves and its Seed both counting
/// up from 1.
fn search(rules: Ruleset, wanted: fn(&State) -> bool) -> Game {
    for seed in 1..100_000 {
        let mut game = Game::new(rules, Seed(seed));
        let mut rng = Xorshift(seed);
        while let Status::ToMove { moves, .. } = game.state().status() {
            advance(&mut game, moves.nth(rng.below(moves.len())));
        }
        if wanted(game.state()) {
            return game;
        }
    }
    panic!("no game under {rules} ends as wanted");
}

fn file_name(rules: &str, name: &str) -> String {
    format!("{rules}-{name}.json")
}

/// The file names of every game the writer records.
fn names() -> Vec<String> {
    let named = NAMED.iter().map(|game| file_name(game.rules, game.name));
    let searched = SEARCHED.iter().map(|game| file_name(game.rules, game.name));
    named.chain(searched).collect()
}

fn play(named: &Named) -> Game {
    let mut game = Game::new(named.rules.parse().unwrap(), Seed(named.seed));
    for cell in cells(named.moves) {
        advance(&mut game, cell);
    }
    game
}

/// Adds this version's games to its slice. A file already there is left alone: a game that
/// replays differently means the rules changed, which takes a bump, not a rewrite.
#[test]
#[ignore = "writes the corpus: cargo test --test corpus -- --ignored"]
fn write_the_corpus() {
    let slice = Path::new(CORPUS).join(format!("v{RULES_VERSION}"));
    fs::create_dir_all(&slice).unwrap();
    let mut names = names();
    names.sort();
    names.dedup();
    assert_eq!(
        names.len(),
        NAMED.len() + SEARCHED.len(),
        "every game has its own name"
    );
    let named = NAMED
        .iter()
        .map(|game| (file_name(game.rules, game.name), play(game)));
    let searched = SEARCHED.iter().map(|game| {
        let rules = game.rules.parse().unwrap();
        (file_name(game.rules, game.name), search(rules, game.wanted))
    });
    for (name, game) in named.chain(searched) {
        let path = slice.join(name);
        if !path.exists() {
            fs::write(path, json(&entry(&game))).unwrap();
        }
    }
}
