//! The computer opponent (spec §6): plain UCT with random playouts. It needs only the legal
//! moves, [`State::play`], the outcome and the side to move, so it plays every Ruleset with no
//! evaluation per Variant.
//!
//! A [`Level`] is an iteration budget with a time cap. The engine has no clock, so [`think`] takes
//! one: `Date.now` in the browser's Worker, `Instant` natively, a fake in tests.
//!
//! [`think`] is the whole move in one call. The browser splits it root-parallel from Level 5:
//! each Worker [`search`]es one of the Level's [`shares`] from the same root, and the page passes
//! every Worker's root visits to [`choose`], which sums them and picks once.

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
    /// Whether its iterations split across Workers.
    parallel: bool,
}

/// The most Workers a search splits across (spec §6).
const MOST_WORKERS: u32 = 4;

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

    /// How many Workers share its search on a device with `cores` logical cores: one below Level
    /// 5, else min(4, cores − 1), and never none.
    pub fn workers(self, cores: u32) -> u32 {
        if self.rung().parallel {
            cores.saturating_sub(1).clamp(1, MOST_WORKERS)
        } else {
            1
        }
    }

    /// Spec §6's table, the one place the self-play calibration tunes.
    const fn rung(self) -> Rung {
        const fn rung(
            iterations: u32,
            caps_ms: [u32; 2],
            choice: Choice,
            tactics: bool,
            parallel: bool,
        ) -> Rung {
            Rung {
                iterations,
                caps_ms,
                choice,
                tactics,
                parallel,
            }
        }
        const LOOSE: Choice = Choice::Sample { tau: 2.0 };
        const FAIR: Choice = Choice::Sample { tau: 1.0 };
        const TIGHT: Choice = Choice::Sample { tau: 0.5 };
        match self.0 {
            1 => rung(30, [1_000, 3_000], LOOSE, false, false),
            2 => rung(100, [1_000, 3_000], LOOSE, false, false),
            3 => rung(300, [1_000, 3_000], FAIR, true, false),
            4 => rung(1_000, [1_000, 3_000], FAIR, true, false),
            5 => rung(3_000, [2_000, 6_000], TIGHT, true, true),
            6 => rung(10_000, [2_000, 6_000], TIGHT, true, true),
            7 => rung(30_000, [3_000, 9_000], Choice::MostVisited, true, true),
            _ => rung(100_000, [3_000, 9_000], Choice::MostVisited, true, true), // 8
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

/// The computer's move in `state` at `level`, or `None` when the game is over: the whole
/// [`search`] in one call, then [`choose`].
///
/// `seed` drives the search's generator, so a position, a Level and a seed give one move while
/// the clock doesn't bind. `elapsed_ms` reads the milliseconds since the search began. It is read
/// every `CLOCK_EVERY` iterations, and the search stops at the Level's cap with what it has.
pub fn think(
    state: &State,
    level: Level,
    seed: u64,
    elapsed_ms: impl FnMut() -> u32,
) -> Option<Cell> {
    match search(state, level, level.iterations(), seed, elapsed_ms)? {
        Searched::Forced(cell) => Some(cell),
        // Its own seed, so the pick doesn't follow the search's last draws.
        Searched::Visits(visits) => choose(level, &visits, Rng(seed).next()),
    }
}

/// What one search from a root found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Searched {
    /// The tactics leave one move to play: an immediate win, or the only move that doesn't hand
    /// the opponent the game. Every search from the root finds the same one.
    Forced(Cell),
    /// The visits of every root move the search could play, in ascending Cell index, unvisited
    /// ones included.
    Visits(Vec<(Cell, u32)>),
}

/// Searches `iterations` from `state` at `level`: the Level's whole budget, or one Worker's share
/// of it. It stops early at the Level's cap, which `elapsed_ms` reads every `CLOCK_EVERY`
/// iterations, from whenever the caller started the clock. `None` when the game is over.
pub fn search(
    state: &State,
    level: Level,
    iterations: u32,
    seed: u64,
    mut elapsed_ms: impl FnMut() -> u32,
) -> Option<Searched> {
    let Status::ToMove { player, moves, .. } = state.status() else {
        return None;
    };
    let moves = if level.tactics() {
        if let Some(win) = winning_move(state) {
            return Some(Searched::Forced(win));
        }
        safe_moves(state, moves)
    } else {
        moves
    };
    if moves.len() == 1 {
        return Some(Searched::Forced(moves.nth(0)));
    }
    let mut rng = Rng(seed);
    let mut tree = Tree::new(*state, player, moves, iterations);
    let cap = level.cap_ms(state.ruleset().depth);
    for i in 0..iterations {
        if i.is_multiple_of(CLOCK_EVERY) && elapsed_ms() >= cap {
            break;
        }
        tree.iterate(&mut rng);
    }
    Some(Searched::Visits(tree.visits()))
}

/// The iterations each Worker searches at `level` on a device with `cores` logical cores: as many
/// shares as [`Level::workers`], as even as can be, summing to the Level's budget.
pub fn shares(level: Level, cores: u32) -> Vec<u32> {
    let workers = level.workers(cores);
    let total = level.iterations();
    let base = total.checked_div(workers).unwrap_or(total);
    let left = total.checked_rem(workers).unwrap_or(0);
    (0..workers)
        .map(|i| base.saturating_add(u32::from(i < left)))
        .collect()
}

/// The move `level` plays from the visits its searches found, every search's listed together in
/// any order: summed by Cell, then the most visited, or sampled in proportion to visits^(1/τ).
/// With no visits at all, a move picked uniformly. `None` only when no move is listed.
pub fn choose(level: Level, visits: &[(Cell, u32)], seed: u64) -> Option<Cell> {
    let visits = merge(visits);
    let mut rng = Rng(seed);
    if visits.iter().all(|&(_, n)| n == 0) {
        return visits.get(rng.below(visits.len())).map(|&(cell, _)| cell);
    }
    match level.choice() {
        Choice::MostVisited => visits
            .iter()
            .max_by_key(|&&(cell, n)| (n, Reverse(cell.index())))
            .map(|&(cell, _)| cell),
        Choice::Sample { tau } => {
            let weight = |n: u32| libm::powf(n as f32, 1.0 / tau);
            let total: f32 = visits.iter().map(|&(_, n)| weight(n)).sum();
            let mut left = rng.unit() * total;
            let mut last = None;
            for &(cell, n) in visits.iter().filter(|&&(_, n)| n > 0) {
                left -= weight(n);
                last = Some(cell);
                if left < 0.0 {
                    break;
                }
            }
            last // the last visited move when rounding leaves a little over
        }
    }
}

/// Visits summed by Cell, in ascending Cell index, so the pick doesn't depend on how many
/// searches there were or the order they came back in.
fn merge(visits: &[(Cell, u32)]) -> Vec<(Cell, u32)> {
    // Kept in order as it goes, not sorted after: a sort would cost the WASM module 2.5 KB.
    let mut merged: Vec<(Cell, u32)> = Vec::new();
    for &(cell, n) in visits {
        match merged.binary_search_by_key(&cell.index(), |&(cell, _)| cell.index()) {
            Ok(at) => {
                if let Some(entry) = merged.get_mut(at) {
                    entry.1 = entry.1.saturating_add(n);
                }
            }
            Err(at) => merged.insert(at, (cell, n)),
        }
    }
    merged
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

    /// The visits of every root move, in ascending Cell index, unvisited ones included.
    fn visits(&self) -> Vec<(Cell, u32)> {
        let mut visits: Vec<(Cell, u32)> = self.moves.iter().map(|cell| (cell, 0)).collect();
        for (_, child) in self.children(0) {
            let found = visits.binary_search_by_key(&child.cell.index(), |&(cell, _)| cell.index());
            if let Some(entry) = found.ok().and_then(|at| visits.get_mut(at)) {
                entry.1 = child.visits;
            }
        }
        visits
    }
}

/// Uniformly random moves to the end of the game.
fn playout(mut state: State, rng: &mut Rng) -> Outcome {
    state.playout(|n| rng.below(n))
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

    /// One search's visits, from `(the root move's position among the legal moves, visits)`.
    fn visits(listed: &[(u16, u32)]) -> Vec<(Cell, u32)> {
        let game = Game::new(Preset::Standard.ruleset(), Seed(0));
        let Status::ToMove { moves, .. } = game.state().status() else {
            unreachable!()
        };
        let mut visits: Vec<_> = listed
            .iter()
            .map(|&(index, n)| (moves.nth(usize::from(index)), n))
            .collect();
        visits.sort_by_key(|&(cell, _)| cell.index());
        visits
    }

    fn level(n: u8) -> Level {
        Level::try_from(n).unwrap()
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
        let searched = visits(&[(5, 3), (9, 7), (2, 7), (40, 1)]);
        // Level 7 plays the most visited.
        assert_eq!(choose(level(7), &searched, 1).unwrap().index(), 2);
    }

    #[test]
    fn sampling_never_picks_an_unvisited_move_and_follows_the_visits() {
        let searched = visits(&[(10, 0), (20, 1), (30, 9)]);
        let mut picks = [0u32; 81];
        for seed in 0..10_000 {
            // Level 3 samples at τ 1.
            let cell = choose(level(3), &searched, seed).unwrap();
            picks[usize::from(cell.index())] += 1;
        }
        assert_eq!(picks[10], 0);
        // Visits 1 and 9 at τ 1 give 10% and 90%.
        assert!((800..1_200).contains(&picks[20]), "{}", picks[20]);
        assert_eq!(picks[20] + picks[30], 10_000);
    }

    #[test]
    fn with_nothing_searched_it_still_picks_a_listed_move() {
        let searched = visits(&[(3, 0), (60, 0)]);
        let cell = choose(level(8), &searched, 3).unwrap();
        assert!(searched.iter().any(|&(listed, _)| listed == cell));
        assert_eq!(choose(level(8), &[], 3), None);
    }

    #[test]
    fn searches_sum_by_cell_whatever_their_number_or_order() {
        let one = visits(&[(1, 4), (7, 2)]);
        let other = visits(&[(7, 5), (1, 1), (9, 0)]);
        let merged = merge(&[one.clone(), other.clone()].concat());
        assert_eq!(merge(&[other, one].concat()), merged);
        let counts: Vec<u32> = merged.iter().map(|&(_, n)| n).collect();
        assert_eq!(counts, [5, 7, 0]);
        assert!(
            merged
                .windows(2)
                .all(|pair| pair[0].0.index() < pair[1].0.index())
        );
    }
}
