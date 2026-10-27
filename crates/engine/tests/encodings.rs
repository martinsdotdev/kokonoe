//! The text encodings of spec §4.1 to §4.3: every value round-trips, the spellings the spec
//! writes are pinned, and each way a code or a Seed can be malformed is refused for its reason.

use kokonoe_engine::{
    Board, Cell, CodeError, Continuation, Depth, DrawnOwner, Fallback, Goal, NotatedMove,
    Objective, Opening, ParseError, Preset, RandomPlies, Ruleset, Seed,
};

/// All 1,440 Rulesets: 2 depths × 2 continuations × 2 drawn owners × 10 objectives × 2
/// fallbacks × 9 openings.
fn every_ruleset() -> Vec<Ruleset> {
    let mut all = Vec::new();
    let objectives = [Objective::Line]
        .into_iter()
        .chain((1..=9).map(|n| Objective::FirstTo(Goal::try_from(n).unwrap())));
    let openings: Vec<Opening> = [Opening::Free]
        .into_iter()
        .chain((1..=8).map(|k| Opening::Random(RandomPlies::try_from(k).unwrap())))
        .collect();
    for objective in objectives {
        for depth in [Depth::Two, Depth::Three] {
            for continuation in [Continuation::Off, Continuation::On] {
                for drawn_owner in [DrawnOwner::Neither, DrawnOwner::Both] {
                    for fallback in [Fallback::Draw, Fallback::MostWon] {
                        for &opening in &openings {
                            all.push(Ruleset {
                                depth,
                                continuation,
                                drawn_owner,
                                objective,
                                fallback,
                                opening,
                            });
                        }
                    }
                }
            }
        }
    }
    all
}

fn code(rules: &Ruleset) -> String {
    rules.to_string()
}

mod ruleset_codes {
    use super::*;

    #[test]
    fn every_ruleset_round_trips_through_its_code() {
        let all = every_ruleset();
        assert_eq!(all.len(), 1_440);
        let mut codes = std::collections::HashSet::new();
        for rules in &all {
            let written = code(rules);
            assert_eq!(written.parse::<Ruleset>(), Ok(*rules), "{written}");
            assert!(codes.insert(written), "one code per Ruleset");
        }
    }

    #[test]
    fn the_presets_and_the_spec_example_read_as_written() {
        let presets: Vec<String> = Preset::ALL.iter().map(|p| code(&p.ruleset())).collect();
        assert_eq!(presets, ["std", "cont", "cont+first5+most", "most", "d3"]);
        let example: Ruleset = "cont+both+first5+most+rand4".parse().unwrap();
        assert_eq!(
            example,
            Ruleset {
                continuation: Continuation::On,
                drawn_owner: DrawnOwner::Both,
                objective: Objective::FirstTo(Goal::try_from(5).unwrap()),
                fallback: Fallback::MostWon,
                opening: Opening::Random(RandomPlies::try_from(4).unwrap()),
                ..Preset::Standard.ruleset()
            }
        );
        assert_eq!(code(&example), "cont+both+first5+most+rand4");
    }

    #[test]
    fn a_malformed_code_is_refused_for_its_reason() {
        let refused = [
            ("", CodeError::Empty.into()),
            ("cont+", CodeError::Empty.into()),
            ("+cont", CodeError::Empty.into()),
            ("cont++most", CodeError::Empty.into()),
            ("standard", CodeError::Unknown.into()),
            ("STD", CodeError::Unknown.into()),
            ("Cont", CodeError::Unknown.into()),
            ("d2", CodeError::Unknown.into()),
            ("first", CodeError::Unknown.into()),
            ("first5x", CodeError::Unknown.into()),
            ("rand-1", CodeError::Unknown.into()),
            ("cont first5", CodeError::Unknown.into()),
            ("std+cont", CodeError::StdCombined.into()),
            ("cont+std", CodeError::StdCombined.into()),
            ("most+cont", CodeError::OutOfOrder.into()),
            ("rand4+d3", CodeError::OutOfOrder.into()),
            ("first5+both", CodeError::OutOfOrder.into()),
            ("cont+cont", CodeError::Duplicate.into()),
            ("first3+first5", CodeError::Duplicate.into()),
            ("first05", CodeError::LeadingZero.into()),
            ("rand08", CodeError::LeadingZero.into()),
            ("first0", ParseError::Goal(0)),
            ("first10", ParseError::Goal(10)),
            ("rand0", ParseError::RandomPlies(0)),
            ("rand9", ParseError::RandomPlies(9)),
        ];
        for (code, error) in refused {
            assert_eq!(code.parse::<Ruleset>(), Err(error), "{code:?}");
        }
    }

    #[test]
    fn a_refusal_says_why() {
        let error = "most+cont".parse::<Ruleset>().unwrap_err();
        assert_eq!(
            error.to_string(),
            "a Ruleset code's settings go in the order d3, cont, both, first, most, rand"
        );
    }
}

mod notation {
    use super::*;

    fn every_cell(depth: Depth) -> impl Iterator<Item = Cell> {
        let count = if depth == Depth::Two { 81 } else { 729 };
        (0..count).map(move |index| Cell::from_index(depth, index).unwrap())
    }

    #[test]
    fn every_cell_round_trips_at_both_depths() {
        for depth in [Depth::Two, Depth::Three] {
            for cell in every_cell(depth) {
                let written = cell.to_string();
                assert_eq!(written.parse::<Cell>(), Ok(cell), "{written}");
                assert_eq!(written.replace('·', ".").parse::<Cell>(), Ok(cell));
            }
        }
    }

    #[test]
    fn it_reads_the_path_from_the_global_board_down_with_spaces_from_one() {
        assert_eq!(Cell::from_index(Depth::Two, 0).unwrap().to_string(), "1·1");
        assert_eq!(Cell::from_index(Depth::Two, 40).unwrap().to_string(), "5·5");
        assert_eq!(Cell::from_index(Depth::Two, 80).unwrap().to_string(), "9·9");
        // 4·0·7 as Spaces is index 4·81 + 0·9 + 7.
        assert_eq!(
            Cell::from_index(Depth::Three, 331).unwrap().to_string(),
            "5·1·8"
        );
    }

    #[test]
    fn a_dealt_move_is_in_parentheses() {
        let cell: Cell = "5·5".parse().unwrap();
        let dealt = NotatedMove { cell, dealt: true };
        assert_eq!(dealt.to_string(), "(5·5)");
        assert_eq!("(5·5)".parse(), Ok(dealt));
        assert_eq!("5.5".parse(), Ok(NotatedMove { cell, dealt: false }));
    }

    #[test]
    fn malformed_notation_is_refused() {
        for text in [
            "",
            "5",
            "55",
            "0·5",
            "5·0",
            "5·",
            "·5",
            "5··5",
            "a·b",
            "10·1",
            "5·5·5·5",
        ] {
            assert!(text.parse::<Cell>().is_err(), "{text:?}");
        }
        assert_eq!("5".parse::<Cell>(), Err(ParseError::AddressLength(1)));
        assert_eq!("5·0".parse::<Cell>(), Err(ParseError::Notation));
        for text in ["(5·5", "5·5)", "((5·5))", "()"] {
            assert!(text.parse::<NotatedMove>().is_err(), "{text:?}");
        }
    }
}

mod seeds_and_boards {
    use super::*;

    #[test]
    fn a_seed_is_sixteen_lowercase_hex_digits() {
        for seed in [0, 0x5eed00, (1 << 53) + 1, u64::MAX] {
            let written = Seed(seed).to_string();
            assert_eq!(written.len(), 16);
            assert_eq!(written.parse(), Ok(Seed(seed)));
        }
        assert_eq!(Seed(0x5eed00).to_string(), "00000000005eed00");
        for text in [
            "5eed00",
            "00000000005EED00",
            "+0000000005eed00",
            "00000000005eed000",
            "",
        ] {
            assert_eq!(text.parse::<Seed>(), Err(ParseError::Seed), "{text:?}");
        }
    }

    #[test]
    fn a_board_path_runs_from_the_global_board_down() {
        assert_eq!(
            Board::from_path(Depth::Two, &[]),
            Ok(Board::global(Depth::Two))
        );
        let local = Board::from_path(Depth::Three, &[1, 7]).unwrap();
        assert!(local.is_local());
        assert_eq!(
            local.spaces().iter().map(|s| s.index()).collect::<Vec<_>>(),
            [1, 7]
        );
        assert_eq!(
            Board::from_path(Depth::Two, &[4, 2]),
            Err(ParseError::BoardPath(2))
        );
        assert_eq!(
            Board::from_path(Depth::Two, &[9]),
            Err(ParseError::Space(9))
        );
    }
}
