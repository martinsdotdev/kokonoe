//! The computer opponent (spec §6): plain UCT with random playouts. It needs only the legal
//! moves, [`State::play`], the outcome and the side to move, so it plays every Ruleset with no
//! evaluation per Variant.
//!
//! A [`Level`] is an iteration budget with a time cap. The engine has no clock, so [`think`] takes
//! one: `Date.now` in the browser's Worker, `Instant` natively, a fake in tests.

use alloc::vec::Vec;
use core::cmp::Reverse;

use crate::address::Cell;
use crate::error::ParseError;
use crate::play::{LegalMoves, Outcome, Player, State, Status};
use crate::ruleset::Depth;

/// One of the computer's 8 Levels, 1 to 8: an iteration budget with a time cap, and how the move
/// is picked. One ladder serves every Preset and Custom variant. Only `TryFrom<u8>` makes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Level(u8);

/// How the move is picked from the root's visit counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    /// At random, in proportion to visits^(1/τ). A higher τ plays more loosely.
    Sample { tau: f32 },
    /// The most-visited move, ties going to the lowest Cell index.
    MostVisited,
}

/// One row of the ladder.
struct Rung {
    iterations: u32,
    /// At depth 2, then at depth 3.
    caps_ms: [u32; 2],
    choice: Choice,
    tactics: bool,
}

impl Level {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 8;

    pub const fn get(self) -> u8 {
        self.0
    }

    /// UCT iterations, when the time cap allows them all.
    pub const fn iterations(self) -> u32 {
        self.rung().iterations
    }

    /// The longest the search may take, in milliseconds. A slow device plays slightly weaker
    /// instead of waiting.
    pub const fn cap_ms(self, depth: Depth) -> u32 {
        let [two, three] = self.rung().caps_ms;
        match depth {
            Depth::Two => two,
            Depth::Three => three,
        }
    }

    pub const fn choice(self) -> Choice {
        self.rung().choice
    }

    /// Whether the computer always takes an immediate win and never hands the opponent one.
    /// Below Level 3 it can miss them.
    pub const fn tactics(self) -> bool {
        self.rung().tactics
    }

    /// Spec §6's table, the one place the self-play calibration tunes.
    const fn rung(self) -> Rung {
        const fn rung(iterations: u32, caps_ms: [u32; 2], choice: Choice, tactics: bool) -> Rung {
            Rung {
                iterations,
                caps_ms,
                choice,
                tactics,
            }
        }
        const LOOSE: Choice = Choice::Sample { tau: 2.0 };
        const FAIR: Choice = Choice::Sample { tau: 1.0 };
        const TIGHT: Choice = Choice::Sample { tau: 0.5 };
        match self.0 {
            1 => rung(30, [1_000, 3_000], LOOSE, false),
            2 => rung(100, [1_000, 3_000], LOOSE, false),
            3 => rung(300, [1_000, 3_000], FAIR, true),
            4 => rung(1_000, [1_000, 3_000], FAIR, true),
            5 => rung(3_000, [2_000, 6_000], TIGHT, true),
            6 => rung(10_000, [2_000, 6_000], TIGHT, true),
            7 => rung(30_000, [3_000, 9_000], Choice::MostVisited, true),
            _ => rung(100_000, [3_000, 9_000], Choice::MostVisited, true), // 8
        }
    }
}

impl TryFrom<u8> for Level {
    type Error = ParseError;

    fn try_from(n: u8) -> Result<Level, ParseError> {
        if (Level::MIN..=Level::MAX).contains(&n) {
            Ok(Level(n))
        } else {
            Err(ParseError::Level(n))
        }
    }
}

impl From<Level> for u8 {
    fn from(level: Level) -> u8 {
        level.0
    }
}

/// How many iterations run between reads of the clock.
const CLOCK_EVERY: u32 = 16;

/// UCB1's exploration constant.
const EXPLORATION: f32 = core::f32::consts::SQRT_2;

/// The computer's move in `state` at `level`, or `None` when the game is over.
///
/// `seed` drives the search's generator, so a position, a Level and a seed give one move while
/// the clock doesn't bind. `elapsed_ms` reads the milliseconds since the search began. It is read
/// every 16 iterations, and the search stops at the Level's cap with what it has.
pub fn think(
    state: &State,
    level: Level,
    seed: u64,
    mut elapsed_ms: impl FnMut() -> u32,
) -> Option<Cell> {
    let Status::ToMove { player, moves, .. } = state.status() else {
        return None;
    };
    let moves = if level.tactics() {
        if let Some(win) = winning_move(state) {
            return Some(win);
        }
        safe_moves(state, moves)
    } else {
        moves
    };
    if moves.len() == 1 {
        return Some(moves.nth(0));
    }
    let mut rng = Rng(seed);
    let mut tree = Tree::new(*state, player, moves, level.iterations());
    let cap = level.cap_ms(state.ruleset().depth);
    for i in 0..level.iterations() {
        if i.is_multiple_of(CLOCK_EVERY) && elapsed_ms() >= cap {
            break;
        }
        tree.iterate(&mut rng);
    }
    Some(tree.choose(level.choice(), &mut rng))
}

/// A move that wins the game at once for the side to move.
fn winning_move(state: &State) -> Option<Cell> {
    let Status::ToMove { player, moves, .. } = state.status() else {
        return None;
    };
    moves.iter().find(|&cell| {
        let mut next = *state;
        matches!(next.play(cell), Ok(Status::Over(outcome)) if outcome.winner() == Some(player))
    })
}

/// The moves that don't hand the opponent the game: after them the opponent hasn't won, and has
/// no winning reply. All of `moves` when every one of them does.
fn safe_moves(state: &State, moves: LegalMoves) -> LegalMoves {
    let mut safe = moves;
    for cell in moves.iter() {
        let mut next = *state;
        let loses = match next.play(cell) {
            Ok(Status::Over(outcome)) => outcome.winner() == Some(next.ply().player()),
            Ok(Status::ToMove { .. }) => winning_move(&next).is_some(),
            Err(_) => false, // unreachable: `cell` came from `moves`
        };
        if loses {
            match safe.without(cell) {
                Some(rest) => safe = rest,
                None => return moves,
            }
        }
    }
    safe
}

/// No node: the end of a child or sibling list.
const NONE: u32 = u32::MAX;

/// A node of the search tree. It holds no State: each iteration replays its path from the root,
/// since a State is `Copy`.
struct Node {
    /// The move into this node. The root's is a placeholder.
    cell: Cell,
    visits: u32,
    /// Half-points for the player who made `cell`: 2 for each win, 1 for each draw.
    score: u32,
    children: u16,
    first_child: u32,
    next_sibling: u32,
}

impl Node {
    const fn new(cell: Cell, next_sibling: u32) -> Node {
        Node {
            cell,
            visits: 0,
            score: 0,
            children: 0,
            first_child: NONE,
            next_sibling,
        }
    }
}

struct Tree {
    root: State,
    /// The side to move at the root.
    player: Player,
    /// The root's moves: every legal one, or only the safe ones.
    moves: LegalMoves,
    /// The root is node 0.
    nodes: Vec<Node>,
    /// This iteration's path from the root, kept to reuse its allocation.
    path: Vec<u32>,
}

impl Tree {
    fn new(root: State, player: Player, moves: LegalMoves, iterations: u32) -> Tree {
        // One node per iteration, plus the root.
        let mut nodes = Vec::with_capacity((iterations as usize).saturating_add(1));
        nodes.push(Node::new(moves.nth(0), NONE));
        Tree {
            root,
            player,
            moves,
            nodes,
            path: Vec::new(),
        }
    }

    /// One UCT iteration.
    fn iterate(&mut self, rng: &mut Rng) {
        if let Some(outcome) = self.descend(rng) {
            self.backpropagate(outcome);
        }
    }

    /// Selects down the tree, adds one untried move, and plays randomly to the end of the game,
    /// leaving the nodes it passed in `path`. `None` only if a legal move failed to play, which
    /// can't happen.
    fn descend(&mut self, rng: &mut Rng) -> Option<Outcome> {
        let mut state = self.root;
        let mut node = 0;
        self.path.clear();
        self.path.push(node);
        loop {
            let moves = match state.status() {
                Status::Over(outcome) => return Some(outcome), // scores as it stands
                Status::ToMove { .. } if node == 0 => self.moves,
                Status::ToMove { moves, .. } => moves,
            };
            let expanded = self.node(node).map_or(0, |node| usize::from(node.children));
            if expanded < moves.len() {
                let cell = self.untried(node, moves, rng)?;
                state.play(cell).ok()?;
                let child = self.add(node, cell);
                self.path.push(child);
                return Some(playout(state, rng));
            }
            node = self.select(node);
            state.play(self.node(node)?.cell).ok()?;
            self.path.push(node);
        }
    }

    /// Scores the path. Turns alternate, so the movers along it do too. The root's score is
    /// never read.
    fn backpropagate(&mut self, outcome: Outcome) {
        let mut mover = self.player.opponent();
        for &index in &self.path {
            if let Some(node) = self.nodes.get_mut(index as usize) {
                node.visits = node.visits.saturating_add(1);
                node.score = node.score.saturating_add(points(outcome, mover));
            }
            mover = mover.opponent();
        }
    }

    fn node(&self, index: u32) -> Option<&Node> {
        self.nodes.get(index as usize)
    }

    fn children(&self, parent: u32) -> impl Iterator<Item = (u32, &Node)> {
        let first = self.node(parent).map_or(NONE, |node| node.first_child);
        core::iter::successors(self.node(first).map(|node| (first, node)), |(_, node)| {
            let next = node.next_sibling;
            self.node(next).map(|node| (next, node))
        })
    }

    /// A random move of `moves` that isn't a child of `parent` yet. Random, not in address
    /// order: Level 1's 30 iterations would otherwise only ever try the first Boards.
    fn untried(&self, parent: u32, moves: LegalMoves, rng: &mut Rng) -> Option<Cell> {
        let mut untried = moves;
        for (_, child) in self.children(parent) {
            untried = untried.without(child.cell)?;
        }
        Some(untried.nth(rng.below(untried.len())))
    }

    fn add(&mut self, parent: u32, cell: Cell) -> u32 {
        let index = self.nodes.len() as u32;
        let mut next_sibling = NONE;
        if let Some(parent) = self.nodes.get_mut(parent as usize) {
            next_sibling = parent.first_child;
            parent.first_child = index;
            parent.children = parent.children.saturating_add(1);
        }
        self.nodes.push(Node::new(cell, next_sibling));
        index
    }

    /// The child with the best UCB1 value. Every child has been visited: each is played out the
    /// moment it is added.
    fn select(&self, parent: u32) -> u32 {
        let log = libm::logf(self.node(parent).map_or(1, |node| node.visits) as f32);
        let mut best = (NONE, f32::NEG_INFINITY);
        for (index, child) in self.children(parent) {
            let visits = child.visits.max(1) as f32;
            let value =
                child.score as f32 / (2.0 * visits) + EXPLORATION * libm::sqrtf(log / visits);
            if value > best.1 {
                best = (index, value);
            }
        }
        best.0
    }

    /// The move to play, from the root's visit counts.
    fn choose(&self, choice: Choice, rng: &mut Rng) -> Cell {
        let picked = match choice {
            Choice::MostVisited => self
                .children(0)
                .max_by_key(|(_, child)| (child.visits, Reverse(child.cell.index())))
                .map(|(_, child)| child.cell),
            Choice::Sample { tau } => {
                let weight = |child: &Node| libm::powf(child.visits as f32, 1.0 / tau);
                let total: f32 = self.children(0).map(|(_, child)| weight(child)).sum();
                let mut left = rng.unit() * total;
                let mut last = None;
                for (_, child) in self.children(0) {
                    left -= weight(child);
                    last = Some(child.cell);
                    if left < 0.0 {
                        break;
                    }
                }
                last // the last child when rounding leaves a little over
            }
        };
        // No child when the clock ran out before the first iteration.
        picked.unwrap_or_else(|| self.moves.nth(rng.below(self.moves.len())))
    }
}

/// Uniformly random moves to the end of the game.
fn playout(mut state: State, rng: &mut Rng) -> Outcome {
    loop {
        let moves = match state.status() {
            Status::Over(outcome) => return outcome,
            Status::ToMove { moves, .. } => moves,
        };
        if state.play(moves.nth(rng.below(moves.len()))).is_err() {
            return Outcome::Draw; // unreachable: the Cell came from `moves`
        }
    }
}

/// Half-points for `player`: 2 for a win, 1 for a draw.
fn points(outcome: Outcome, player: Player) -> u32 {
    match outcome.winner() {
        Some(winner) if winner == player => 2,
        Some(_) => 0,
        None => 1,
    }
}

/// The search's generator, wyrand: fast, and plenty for picking moves. The opening's SplitMix64
/// is pinned by `RULES_VERSION`, so the search keeps its own.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0xA076_1D64_78BD_642F);
        let product = u128::from(self.0).wrapping_mul(u128::from(self.0 ^ 0xE703_7ED1_A0B4_28DB));
        (product >> 64) as u64 ^ product as u64
    }

    /// In `0..n`, 0 when `n` is 0. Multiply-shift, whose bias of under n / 2^64 can't matter
    /// when picking among at most 729 moves.
    fn below(&mut self, n: usize) -> usize {
        (u128::from(self.next()).wrapping_mul(n as u128) >> 64) as usize
    }

    /// In `[0, 1)`, from the top 24 bits.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u32 << 24) as f32
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]
mod tests {
    use super::*;
    use crate::play::{Game, Seed};
    use crate::ruleset::Preset;

    fn tree(visits: &[(u16, u32)]) -> Tree {
        let game = Game::new(Preset::Standard.ruleset(), Seed(0));
        let state = *game.state();
        let Status::ToMove { moves, .. } = state.status() else {
            unreachable!()
        };
        let mut tree = Tree::new(state, Player::X, moves, 0);
        for &(index, visits) in visits {
            let child = tree.add(0, moves.nth(usize::from(index)));
            tree.nodes[child as usize].visits = visits;
        }
        tree
    }

    #[test]
    fn the_generator_stays_in_range() {
        let mut rng = Rng(7);
        for n in 1..100 {
            assert!(rng.below(n) < n);
        }
        assert_eq!(rng.below(0), 0);
        for _ in 0..1_000 {
            assert!((0.0..1.0).contains(&rng.unit()));
        }
    }

    #[test]
    fn most_visited_takes_the_most_visits_and_the_lowest_cell_on_a_tie() {
        let mut rng = Rng(1);
        let tree = tree(&[(5, 3), (9, 7), (2, 7), (40, 1)]);
        assert_eq!(tree.choose(Choice::MostVisited, &mut rng).index(), 2);
    }

    #[test]
    fn sampling_never_picks_an_unvisited_move_and_follows_the_visits() {
        let mut rng = Rng(2);
        let tree = tree(&[(10, 0), (20, 1), (30, 9)]);
        let mut picks = [0u32; 81];
        for _ in 0..10_000 {
            picks[usize::from(tree.choose(Choice::Sample { tau: 1.0 }, &mut rng).index())] += 1;
        }
        assert_eq!(picks[10], 0);
        // Visits 1 and 9 at τ 1 give 10% and 90%.
        assert!((800..1_200).contains(&picks[20]), "{}", picks[20]);
        assert_eq!(picks[20] + picks[30], 10_000);
    }

    #[test]
    fn with_nothing_searched_it_still_picks_a_root_move() {
        let mut rng = Rng(3);
        let tree = tree(&[]);
        let cell = tree.choose(Choice::MostVisited, &mut rng);
        assert!(tree.moves.contains(cell));
    }
}
