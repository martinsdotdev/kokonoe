//! Random openings (spec §2.1). The deal is part of the replay format: a Record keeps only its
//! Seed, and every client deals the same plies from it. These vectors pin today's deal, so
//! changing one means bumping `RULES_VERSION` (ADR-0004), since every stored Record with a Random
//! opening would replay differently.

mod model;

use kokonoe_engine::{
    Cell, Game, Goal, Objective, Opening, Outcome, Played, Player, Ply, Preset, RandomPlies,
    Ruleset, Seed, Status,
};
use model::{Model, depth_two_rulesets};

fn random(k: u8) -> Opening {
    Opening::Random(RandomPlies::try_from(k).unwrap())
}

fn with_opening(preset: Preset, k: u8) -> Ruleset {
    Ruleset {
        opening: random(k),
        ..preset.ruleset()
    }
}

/// The opening as digit pairs: `"42"` is `4·2`.
fn dealt(game: &Game) -> String {
    let digits = |cell: &Cell| -> String {
        cell.spaces()
            .iter()
            .map(|space| space.index().to_string())
            .collect()
    };
    game.opening()
        .iter()
        .map(digits)
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn a_seed_always_deals_the_same_opening() {
    let vectors = [
        (1, 0, "78"),
        (1, 1, "50"),
        (1, 0x5eed00, "10"),
        (1, u64::MAX, "80"),
        (4, 0, "78 83 30 08"),
        (4, 1, "50 06 68 83"),
        (4, 0x5eed00, "10 08 82 20"),
        (4, u64::MAX, "80 08 82 23"),
        (8, 0, "78 83 30 08 80 02 21 16"),
        (8, 1, "50 06 68 83 33 37 77 74"),
        (8, 0x5eed00, "10 08 82 20 00 03 35 52"),
        (8, u64::MAX, "80 08 82 23 36 67 78 83"),
    ];
    for (k, seed, expected) in vectors {
        let rules = with_opening(Preset::Standard, k);
        let game = Game::new(rules, Seed(seed));
        assert_eq!(dealt(&game), expected, "Random({k}), Seed {seed:#x}");
        assert_eq!(
            dealt(&Game::new(rules, Seed(seed))),
            expected,
            "dealt again"
        );
        let opened = Game::open(&game.record()).unwrap();
        assert_eq!(dealt(&opened), expected, "dealt again by open");
    }
    let next_seed = Game::new(with_opening(Preset::Standard, 8), Seed(0x5eed01));
    assert_ne!(
        dealt(&next_seed),
        "10 08 82 20 00 03 35 52",
        "a neighbouring Seed"
    );
}

/// Seed 490 deals O the middle Local board on the sixth ply: 4·0, 4·4 and 4·8.
const SEED_490: &str = "54 48 84 40 04 44";

#[test]
fn the_deal_follows_the_rules_it_deals_under() {
    // With continuation off the won middle board takes no move, so the seventh ply goes
    // elsewhere; with it on, the middle board still takes moves.
    let standard = Game::new(with_opening(Preset::Standard, 8), Seed(490));
    assert_eq!(dealt(&standard), format!("{SEED_490} 51 13"));
    let continuation = Game::new(with_opening(Preset::ForcedContinuation, 8), Seed(490));
    assert_eq!(dealt(&continuation), format!("{SEED_490} 45 53"));
}

#[test]
fn first_to_one_can_end_inside_the_opening() {
    let rules = Ruleset {
        objective: Objective::FirstTo(Goal::try_from(1).unwrap()),
        ..with_opening(Preset::Standard, 8)
    };
    let game = Game::new(rules, Seed(490));
    assert_eq!(dealt(&game), SEED_490, "six of eight plies, then it's over");
    assert_eq!(
        game.state().status(),
        Status::Over(Outcome::Objective(Player::O))
    );
    assert_eq!(game.record().moves, []);
}

#[test]
fn the_dealt_plies_count_but_belong_to_no_one() {
    let mut game = Game::new(with_opening(Preset::Standard, 4), Seed(0x5eed00));
    let opening = game.opening().to_vec();
    assert_eq!(game.state().ply(), Ply::new(4));
    assert_eq!(
        game.record().moves,
        [],
        "the Record keeps the Seed, not the deal"
    );
    assert_eq!(game.undo(), None, "a dealt ply is setup, not a move");
    for (ply, &cell) in (0..).zip(&opening) {
        assert_eq!(game.play(Ply::new(ply), cell), Ok(Played::Already));
    }
    assert_eq!(game.state().ply(), Ply::new(4));
}

/// SplitMix64 with Lemire's bounded pick, written again from their definitions, so the deal is
/// checked against something other than itself.
struct Reference(u64);

impl Reference {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        let threshold = n.wrapping_neg() % n;
        loop {
            let product = u128::from(self.next()) * u128::from(n);
            if (product as u64) >= threshold {
                return (product >> 64) as u64;
            }
        }
    }
}

#[test]
fn each_ply_is_the_drawn_position_in_the_ascending_legal_moves() {
    // The deal picks by position, so the order of the legal moves is part of it: a seam
    // invariant every representation must keep (see `BoardRepr::legal_in`). The legal moves
    // come from the reference model here, listed in ascending Cell index.
    for base in depth_two_rulesets() {
        for seed in [0, 7, 490, 0x5eed00] {
            let game = Game::new(
                Ruleset {
                    opening: random(8),
                    ..base
                },
                Seed(seed),
            );
            let mut model = Model::new(base);
            let mut rng = Reference(seed);
            for &cell in game.opening() {
                let (_, legal) = model.turn().expect("the opening ends with the game");
                assert!(legal.windows(2).all(|pair| pair[0] < pair[1]));
                let pick = rng.below(legal.len() as u64) as usize;
                assert_eq!(
                    usize::from(cell.index()),
                    legal[pick],
                    "{base:?}, Seed {seed}"
                );
                model.play(legal[pick]);
            }
            let ended = model.turn().is_none();
            assert!(
                game.opening().len() == 8 || ended,
                "short only when the game ended"
            );
        }
    }
}
