use enumoid::{EnumMap, Enumoid};
use rand::prelude::*;
use sliding_tree::{Node, NodeMut, SlidingTree};
use smallvec::SmallVec;
use std::{f32, slice};

pub type Piles = [u8; 3];

#[derive(Clone, Copy, Debug, Enumoid, PartialEq)]
pub enum Player {
    Player1,
    Player2,
}

impl Player {
    fn opposite(&self) -> Player {
        match self {
            Player::Player1 => Player::Player2,
            Player::Player2 => Player::Player1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Goal {
    TakeLast,
    #[allow(dead_code)]
    NotTakeLast,
}

#[derive(Clone)]
pub struct NimState {
    piles: Piles,
    next_player: Player,
    wins: EnumMap<Player, usize>,
    visits: usize,
}

#[derive(Clone, Copy)]
struct NimMove {
    pile_index: usize,
    take: u8,
}

impl NimState {
    pub fn new(piles: Piles, starting_player: Player) -> Self {
        NimState {
            piles,
            next_player: starting_player,
            wins: EnumMap::default(),
            visits: 0,
        }
    }

    fn is_terminal(&self) -> bool {
        self.piles.iter().all(|&pile| pile == 0)
    }

    fn possible_moves(&self) -> SmallVec<[NimMove; 128]> {
        self.piles
            .iter()
            .enumerate()
            .flat_map(|(i, &pile)| {
                (1..=pile).map(move |take| NimMove {
                    pile_index: i,
                    take,
                })
            })
            .collect()
    }

    fn apply_move(&mut self, mv: NimMove) {
        self.piles[mv.pile_index] -= mv.take;
        self.next_player = self.next_player.opposite();
    }

    fn winning_player(&self, goal: Goal) -> Option<Player> {
        self.is_terminal().then(|| match goal {
            Goal::TakeLast => self.next_player.opposite(),
            Goal::NotTakeLast => self.next_player,
        })
    }

    fn update_stats(&mut self, winner: Player) {
        self.visits += 1;
        *self.wins.get_mut(winner) += 1;
    }

    fn win_rate(&self, player: Player) -> f32 {
        let wins = self.wins[player] as f32;
        let visits = self.visits as f32;
        if visits > 0.0 {
            wins / visits
        } else {
            0.0
        }
    }
}

pub fn play_mcts(
    mut root_state: NimState,
    iterations: usize,
    goal: Goal,
    rng: &mut impl Rng,
) -> Player {
    let mut tree = SlidingTree::new();
    tree.set_roots(root_state.possible_moves().iter().map(|&mv| {
        let mut new_state = root_state.clone();
        new_state.apply_move(mv);
        new_state
    }));

    while !root_state.is_terminal() {
        // Perform MCTS iterations
        for _ in 0..iterations {
            let index = select_with_ucb(
                tree.iter(),
                root_state.visits,
                root_state.next_player,
            );
            let mut node = tree.at_mut(index);
            select_and_backpropagate(&mut node, goal, rng);
            root_state.visits += 1;
        }

        // Select the best move based on win rate
        let best_child_index = find_best_child(&tree, root_state.next_player);

        // Apply the best move and update the tree
        let mut best_child = tree.at_mut(best_child_index);
        /*println!(
            "best child win rate: {:?}={}",
            Player::Player1,
            best_child.user().win_rate(Player::Player1)
        );*/
        root_state = best_child.get().clone();
        best_child.set_pending_roots();
        tree.update_roots();
    }

    root_state.winning_player(goal).unwrap()
}

// Helper function to find the best child based on win rate
fn find_best_child(tree: &SlidingTree<NimState>, player: Player) -> usize {
    tree.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.get()
                .win_rate(player)
                .partial_cmp(&b.get().win_rate(player))
                .unwrap()
        })
        .unwrap()
        .0
}

fn select_and_backpropagate(
    node: &mut NodeMut<NimState>,
    goal: Goal,
    rng: &mut impl Rng,
) -> Player {
    let state = node.get();
    let winner = if !state.is_terminal() && !node.is_empty() {
        let index =
            select_with_ucb(node.iter(), state.visits, state.next_player);
        let mut child_node = node.at_mut(index);
        select_and_backpropagate(&mut child_node, goal, rng)
    } else if !state.is_terminal() {
        expand_and_simulate(node, goal, rng)
    } else {
        state.winning_player(goal).unwrap()
    };
    node.get_mut().update_stats(winner);
    winner
}

fn expand_and_simulate(
    node: &mut NodeMut<NimState>,
    goal: Goal,
    rng: &mut impl Rng,
) -> Player {
    let state = node.get().clone();
    // Expansion
    let moves = state.possible_moves();
    node.set_children(moves.iter().map(|&mv| {
        let mut new_state = state.clone();
        new_state.apply_move(mv);
        new_state
    }));

    // Simulation
    let mut sim_state = state.clone();
    while !sim_state.is_terminal() {
        let moves = sim_state.possible_moves();
        let mv = moves[rng.gen_range(0..moves.len())];
        sim_state.apply_move(mv);
    }

    sim_state.winning_player(goal).unwrap()
}

fn select_with_ucb(
    iter: slice::Iter<'_, Node<'_, NimState>>,
    total_visits: usize,
    player: Player,
) -> usize {
    let total_visits = total_visits as f32;
    let exploration_constant = std::f32::consts::SQRT_2;
    let ln_total_visits = total_visits.ln();

    iter.enumerate()
        .map(|(i, child)| {
            let child_state = child.get();
            let wins = child_state.wins[player] as f32;
            let visits = child_state.visits as f32;
            let ucb = if visits > 0.0 {
                (wins / visits)
                    + exploration_constant * (ln_total_visits / visits).sqrt()
            } else {
                f32::INFINITY
            };
            (i, ucb)
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap()
        .0
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use rand_pcg::Pcg64Mcg;

    #[allow(dead_code)]
    fn run_mcts_test(
        initial_piles: [u8; 3],
        goal: Goal,
        expected_winner: Player,
    ) {
        let initial_state = NimState::new(initial_piles, Player::Player1);
        let mut rng = Pcg64Mcg::seed_from_u64(12345);
        let winner = play_mcts(initial_state, 70000, goal, &mut rng);
        assert_eq!(expected_winner, winner);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_first_player_wins_take_last() {
        run_mcts_test([3, 4, 5], Goal::TakeLast, Player::Player1);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_first_player_wins_not_take_last() {
        run_mcts_test([3, 4, 5], Goal::NotTakeLast, Player::Player1);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_second_player_wins_take_last() {
        run_mcts_test([2, 4, 6], Goal::TakeLast, Player::Player2);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_second_player_wins_not_take_last() {
        run_mcts_test([2, 4, 6], Goal::NotTakeLast, Player::Player2);
    }

    #[test]
    fn test_nim_state() {
        let mut state = NimState::new([3, 4, 5], Player::Player1);
        assert!(!state.is_terminal());

        let moves = state.possible_moves();
        assert!(!moves.is_empty());

        state.apply_move(moves[0]);
        assert_eq!(state.next_player, Player::Player2);

        // Play until terminal state
        while !state.is_terminal() {
            let moves = state.possible_moves();
            state.apply_move(moves[0]);
        }

        assert!(state.is_terminal());
        assert!(state.winning_player(Goal::TakeLast).is_some());
    }
}
