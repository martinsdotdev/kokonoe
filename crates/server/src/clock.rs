//! A timed game's clocks (spec §11) as arithmetic on durations, with no I/O and no clock of its
//! own: the Room says how long each turn took, so tests can run it on any time they like.
//!
//! A player's clock runs only on turns after their first move, and that first move adds no
//! increment. Dealt plies are nobody's move: the played moves alternate from `first`. Until a
//! player has moved, their turn has the abort window instead.

use std::time::Duration;

use kokonoe_engine::Player;
use kokonoe_protocol::TimeControl;

/// A timed game's clocks: its control, and each played move's time left for its mover after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clock {
    control: TimeControl,
    bank: Duration,
    increment: Duration,
    /// Who makes the first played move, after any dealt plies.
    first: Player,
    after: Vec<Duration>,
    /// The player who lost on time, whose clock reads 0.
    flagged: Option<Player>,
}

/// When a turn runs out, counted from its start, and what happens then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deadline {
    /// The player to move hasn't made their first move: the game aborts.
    Abort(Duration),
    /// Their clock runs: they lose on time.
    Flag(Duration),
}

impl Deadline {
    pub fn after(self) -> Duration {
        match self {
            Deadline::Abort(after) | Deadline::Flag(after) => after,
        }
    }
}

/// The clocks at a moment: each player's time left, and whose is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reading {
    pub x: Duration,
    pub o: Duration,
    pub running: Option<Player>,
}

impl Clock {
    /// The clocks of a game under `control`, whose units are `minute` and `second`, its played
    /// moves starting with `first`'s, after which each mover had the ms in `after` left.
    pub fn new(
        control: TimeControl,
        minute: Duration,
        second: Duration,
        first: Player,
        after: &[u32],
    ) -> Clock {
        Clock {
            control,
            bank: minute * u32::from(control.minutes()),
            increment: second * u32::from(control.increment()),
            first,
            after: after
                .iter()
                .map(|&ms| Duration::from_millis(u64::from(ms)))
                .collect(),
            flagged: None,
        }
    }

    pub fn control(&self) -> TimeControl {
        self.control
    }

    /// Who makes the played move at `index`.
    fn mover(&self, index: usize) -> Player {
        if index.is_multiple_of(2) {
            self.first
        } else {
            self.first.opponent()
        }
    }

    /// Who makes the next move.
    pub fn to_move(&self) -> Player {
        self.mover(self.after.len())
    }

    /// The time `player` had left after their last move, if they've made one.
    fn last(&self, player: Player) -> Option<Duration> {
        let mut moves = self.after.iter().enumerate().rev();
        moves
            .find(|&(index, _)| self.mover(index) == player)
            .map(|(_, &left)| left)
    }

    /// Whether `player` has made a move of their own, which starts their clock.
    pub fn started(&self, player: Player) -> bool {
        self.last(player).is_some()
    }

    /// `player`'s time left when their turn last ended: their whole bank before their first move,
    /// and none once they've lost on time.
    pub fn left(&self, player: Player) -> Duration {
        if self.flagged == Some(player) {
            return Duration::ZERO;
        }
        self.last(player).unwrap_or(self.bank)
    }

    /// How long the turn now running may last, with `abort` the window for a first move.
    pub fn deadline(&self, abort: Duration) -> Deadline {
        let player = self.to_move();
        if self.started(player) {
            Deadline::Flag(self.left(player))
        } else {
            Deadline::Abort(abort)
        }
    }

    /// The next move, `elapsed` into its turn, and the time its mover has after it. A first move
    /// costs nothing and earns nothing; any other spends `elapsed` and earns the increment. The
    /// Room settles a turn past its deadline before a move can come, so this never flags.
    pub fn spend(&mut self, elapsed: Duration) -> Duration {
        let mover = self.to_move();
        let left = if self.started(mover) {
            self.left(mover).saturating_sub(elapsed) + self.increment
        } else {
            self.bank
        };
        self.after.push(left);
        left
    }

    /// The player to move ran out of time.
    pub fn flag(&mut self, player: Player) {
        self.flagged = Some(player);
    }

    /// Keeps the clocks after the first `keep` played moves: an accepted takeback.
    pub fn take_back(&mut self, keep: usize) {
        self.after.truncate(keep);
    }

    /// The clocks `elapsed` into the turn now running, if one is: the player to move's runs once
    /// they've made their first move.
    pub fn read(&self, turn: bool, elapsed: Duration) -> Reading {
        let to_move = self.to_move();
        let running = (turn && self.started(to_move)).then_some(to_move);
        let left = |player: Player| {
            let left = self.left(player);
            if running == Some(player) {
                left.saturating_sub(elapsed)
            } else {
                left
            }
        };
        Reading {
            x: left(Player::X),
            o: left(Player::O),
            running,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);
    const MINUTE: Duration = Duration::from_secs(60);
    const ABORT: Duration = Duration::from_secs(30);

    fn clock(control: TimeControl, first: Player) -> Clock {
        Clock::new(control, MINUTE, SECOND, first, &[])
    }

    #[test]
    fn a_clock_starts_after_its_players_first_move() {
        let mut clock = clock(TimeControl::ThreePlusTwo, Player::X);
        let bank = SECOND * 180;
        assert_eq!(clock.deadline(ABORT), Deadline::Abort(ABORT));
        assert_eq!(clock.spend(SECOND * 20), bank, "a first move costs nothing");
        assert_eq!(clock.deadline(ABORT), Deadline::Abort(ABORT));
        assert_eq!(clock.spend(SECOND * 29), bank, "nor earns the increment");
        assert_eq!(clock.deadline(ABORT), Deadline::Flag(bank));
        let reading = clock.read(true, SECOND * 5);
        assert_eq!(
            (reading.x, reading.o, reading.running),
            (SECOND * 175, bank, Some(Player::X))
        );
        assert_eq!(clock.read(false, SECOND * 5).running, None, "no turn runs");
    }

    #[test]
    fn a_move_spends_its_time_and_earns_the_increment() {
        let mut clock = clock(TimeControl::FivePlusThree, Player::X);
        clock.spend(SECOND);
        clock.spend(SECOND);
        let left = clock.spend(SECOND * 10);
        assert_eq!(left, SECOND * (300 - 10 + 3));
        assert_eq!(clock.left(Player::X), left);
        assert_eq!(
            clock.left(Player::O),
            SECOND * 300,
            "the other clock didn't run"
        );
        assert_eq!(clock.deadline(ABORT), Deadline::Flag(SECOND * 300));
    }

    #[test]
    fn a_turn_flags_when_its_time_runs_out() {
        let mut clock = clock(TimeControl::ThreePlusTwo, Player::X);
        clock.spend(SECOND);
        clock.spend(SECOND);
        clock.spend(SECOND * 100);
        clock.spend(SECOND * 179);
        // X had 82 s; O had 180, spent 179 and earned 2.
        assert_eq!(clock.deadline(ABORT), Deadline::Flag(SECOND * 82));
        assert_eq!(clock.left(Player::O), SECOND * 3);
        assert_eq!(
            clock.read(true, SECOND * 90).x,
            Duration::ZERO,
            "it reads 0, never less"
        );
        clock.flag(Player::X);
        let reading = clock.read(false, SECOND * 90);
        assert_eq!((reading.x, reading.o), (Duration::ZERO, SECOND * 3));
    }

    #[test]
    fn a_takeback_restores_both_clocks() {
        let mut clock = clock(TimeControl::TenPlusFive, Player::X);
        clock.spend(SECOND);
        clock.spend(SECOND);
        clock.spend(SECOND * 40);
        let before = (clock.left(Player::X), clock.left(Player::O));
        clock.spend(SECOND * 70);
        clock.spend(SECOND * 20);
        clock.take_back(3);
        assert_eq!((clock.left(Player::X), clock.left(Player::O)), before);
        clock.take_back(1);
        assert_eq!(clock.to_move(), Player::O);
        assert!(
            !clock.started(Player::O),
            "O's first move taken back: O's turn aborts again"
        );
        assert_eq!(clock.deadline(ABORT), Deadline::Abort(ABORT));
    }

    #[test]
    fn a_dealt_opening_starts_no_clock() {
        // Three dealt plies, X, O and X, are nobody's moves: O makes the first played move, and
        // only each player's own first move starts their clock.
        let mut clock = clock(TimeControl::FivePlusThree, Player::O);
        assert_eq!(clock.to_move(), Player::O);
        assert_eq!(clock.deadline(ABORT), Deadline::Abort(ABORT));
        clock.spend(SECOND * 10);
        assert_eq!(
            (clock.to_move(), clock.started(Player::X)),
            (Player::X, false)
        );
        assert_eq!(clock.deadline(ABORT), Deadline::Abort(ABORT));
        clock.spend(SECOND * 10);
        let reading = clock.read(true, SECOND * 10);
        assert_eq!(
            (reading.x, reading.o, reading.running),
            (SECOND * 300, SECOND * 290, Some(Player::O))
        );
    }

    #[test]
    fn a_stored_clock_reads_as_it_was_played() {
        let stored = [180_000, 180_000, 171_250];
        let clock = Clock::new(
            TimeControl::ThreePlusTwo,
            MINUTE,
            SECOND,
            Player::O,
            &stored,
        );
        assert_eq!(clock.left(Player::O), Duration::from_millis(171_250));
        assert_eq!(clock.left(Player::X), Duration::from_millis(180_000));
        assert_eq!(clock.to_move(), Player::X);
    }

    #[test]
    fn time_units_shorten_for_tests() {
        let (minute, second) = (Duration::from_millis(200), Duration::from_millis(40));
        let mut clock = Clock::new(TimeControl::ThreePlusTwo, minute, second, Player::X, &[]);
        assert_eq!(clock.left(Player::X), Duration::from_millis(600));
        clock.spend(SECOND);
        clock.spend(SECOND);
        let left = clock.spend(Duration::from_millis(100));
        assert_eq!(left, Duration::from_millis(600 - 100 + 80));
    }
}
