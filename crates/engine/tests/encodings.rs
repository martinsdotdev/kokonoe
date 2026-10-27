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

mod ruleset_codes {
    use super::*;

    #[test]
    fn every_ruleset_round_trips_through_its_code() {
        let all = every_ruleset();
        assert_eq!(all.len(), 1_440);
        let mut codes = std::collections::HashSet::new();
        for rules in &all {
            let written = rules.to_string();
            assert_eq!(written.parse::<Ruleset>(), Ok(*rules), "{written}");
            assert!(codes.insert(written), "one code per Ruleset");
        }
    }

    #[test]
    fn the_presets_and_the_spec_example_read_as_written() {
        let presets: Vec<String> = Preset::ALL
            .iter()
            .map(|p| p.ruleset().to_string())
            .collect();
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
        assert_eq!(example.to_string(), "cont+both+first5+most+rand4");
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
            ("first", CodeError::FirstTo.into()),
            ("first5x", CodeError::FirstTo.into()),
            ("first300", CodeError::FirstTo.into()),
            ("rand", CodeError::Random.into()),
            ("rand-1", CodeError::Random.into()),
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
            ("first0", CodeError::FirstTo.into()),
            ("first10", CodeError::FirstTo.into()),
            ("rand0", CodeError::Random.into()),
            ("rand9", CodeError::Random.into()),
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
            "the settings of a Ruleset code go in the order d3, cont, both, first, most, rand"
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

mod json {
    use super::*;
    use kokonoe_engine::{Game, OpenError, RULES_VERSION, Record, RecordFile};

    #[test]
    fn every_ruleset_round_trips_through_json() {
        for rules in every_ruleset() {
            let json = serde_json::to_string(&rules).unwrap();
            assert_eq!(
                serde_json::from_str::<Ruleset>(&json).unwrap(),
                rules,
                "{json}"
            );
        }
    }

    #[test]
    fn standard_is_the_spec_string() {
        let json = serde_json::to_string(&Preset::Standard.ruleset()).unwrap();
        assert_eq!(
            json,
            r#"{"depth":2,"continuation":"Off","drawn_owner":"Neither","objective":"Line","fallback":"Draw","opening":"Free"}"#
        );
        let counting = serde_json::to_value(Preset::TicTacKu.ruleset()).unwrap();
        assert_eq!(counting["objective"], serde_json::json!({ "FirstTo": 5 }));
    }

    #[test]
    fn json_runs_the_engine_boundary_checks() {
        let standard = serde_json::to_value(Preset::Standard.ruleset()).unwrap();
        for (field, value) in [
            ("depth", serde_json::json!(4)),
            ("objective", serde_json::json!({ "FirstTo": 10 })),
            ("opening", serde_json::json!({ "Random": 0 })),
            ("fallback", serde_json::json!("Coin")),
        ] {
            let mut bad = standard.clone();
            bad[field] = value;
            assert!(serde_json::from_value::<Ruleset>(bad).is_err(), "{field}");
        }
    }

    fn record(seed: u64) -> Record {
        let mut game = Game::new(Preset::Standard.ruleset(), Seed(seed));
        for digits in ["42", "24"] {
            let at = game.state().ply();
            game.play(at, digits_cell(digits)).unwrap();
        }
        game.record()
    }

    fn digits_cell(digits: &str) -> Cell {
        let digits: Vec<u8> = digits.bytes().map(|b| b - b'0').collect();
        Cell::from_digits(&digits).unwrap()
    }

    #[test]
    fn a_record_round_trips_with_a_seed_past_two_to_the_53() {
        for seed in [(1 << 53) + 1, u64::MAX] {
            let original = record(seed);
            let json = serde_json::to_string(&original).unwrap();
            assert_eq!(
                serde_json::from_str::<Record>(&json).unwrap(),
                original,
                "{json}"
            );
        }
    }

    #[test]
    fn a_record_is_the_spec_shape() {
        let json = serde_json::to_string(&record(0x5eed00)).unwrap();
        assert_eq!(
            json,
            format!(
                r#"{{"rules_version":{RULES_VERSION},"ruleset":{},"seed":"00000000005eed00","moves":[38,22]}}"#,
                serde_json::to_string(&Preset::Standard.ruleset()).unwrap()
            )
        );
    }

    #[test]
    fn a_record_file_carries_a_custom_variant_name() {
        let plain = RecordFile {
            record: record(1),
            name: None,
        };
        let json = serde_json::to_value(&plain).unwrap();
        assert!(json.get("name").is_none(), "absent unless named");
        assert_eq!(serde_json::from_value::<RecordFile>(json).unwrap(), plain);

        let named = RecordFile {
            name: Some("Quick fives".into()),
            ..plain
        };
        let json = serde_json::to_string(&named).unwrap();
        assert_eq!(serde_json::from_str::<RecordFile>(&json).unwrap(), named);
    }

    #[test]
    fn a_record_reads_its_moves_at_its_depth_and_open_checks_its_version() {
        let mut json = serde_json::to_value(record(1)).unwrap();
        json["moves"] = serde_json::json!([81]);
        assert!(
            serde_json::from_value::<Record>(json.clone()).is_err(),
            "past depth 2"
        );

        json["moves"] = serde_json::json!([40]);
        json["rules_version"] = serde_json::json!(RULES_VERSION + 1);
        let future: Record = serde_json::from_value(json).unwrap();
        assert_eq!(
            Game::open(&future).err(),
            Some(OpenError::RulesVersion(RULES_VERSION + 1))
        );
    }
}
