use std::ops::{Add, Sub};

/// A pair a stage may take: `row` and `target` index the stage's rows and targets. A `sure` pair
/// is worth taking whatever it costs, as long as only other sure pairs move for it.
pub(super) struct Edge<C> {
    pub row: usize,
    pub target: usize,
    pub cost: C,
    pub sure: bool,
}

/// The edges to take. Each row and each target is in at most one taken pair. With no `worth`, as
/// many pairs as the rows and targets allow, and of those sets the one whose summed cost is
/// least. With a `worth`, a pair is worth taking only while it costs less than leaving its row
/// and its target unpaired, each at half the `worth`: the cheapest set of any size, counting
/// those. Then, among the rows and targets no other pair holds, sure pairs grow the set to as many
/// as they allow, the cheapest such set. Between equally cheap sets the edges' order decides, so
/// the caller lists them by its tie-breaks.
///
/// Successive shortest paths: each round finds the cheapest way to pair one more row, letting
/// rows already paired move to another target, until no free row reaches a free target, or the
/// cheapest way costs `worth` or more. Each round costs at least as much as the one before, so
/// stopping there leaves the cheapest set, and the set stays the cheapest of its size among the
/// sure pairs, where the rounds go on.
pub(super) fn cheapest_pairs<C>(
    edges: &[Edge<C>],
    rows: usize,
    targets: usize,
    worth: Option<C>,
) -> Vec<usize>
where
    C: Copy + Ord + Default + Add<Output = C> + Sub<Output = C>,
{
    let mut row_edge: Vec<Option<usize>> = vec![None; rows];
    let mut target_edge: Vec<Option<usize>> = vec![None; targets];
    grow(edges, |_| true, worth, &mut row_edge, &mut target_edge);
    if worth.is_some() {
        let mut held_row = vec![false; rows];
        let mut held_target = vec![false; targets];
        for edge in row_edge.iter().flatten().map(|&taken| &edges[taken]) {
            if !edge.sure {
                held_row[edge.row] = true;
                held_target[edge.target] = true;
            }
        }
        let among_sure_pairs =
            |edge: &Edge<C>| edge.sure && !held_row[edge.row] && !held_target[edge.target];
        grow(
            edges,
            among_sure_pairs,
            None,
            &mut row_edge,
            &mut target_edge,
        );
    }
    let mut taken: Vec<usize> = row_edge.into_iter().flatten().collect();
    taken.sort_unstable();
    taken
}

/// Pairs one more row a round over the `usable` edges, until none is left to pair or pairing one
/// costs `worth` or more.
fn grow<C>(
    edges: &[Edge<C>],
    usable: impl Fn(&Edge<C>) -> bool,
    worth: Option<C>,
    row_edge: &mut [Option<usize>],
    target_edge: &mut [Option<usize>],
) where
    C: Copy + Ord + Default + Add<Output = C> + Sub<Output = C>,
{
    while let Some((target, cost, reached_by)) =
        cheapest_free_target(edges, &usable, row_edge, target_edge)
    {
        if worth.is_some_and(|worth| cost >= worth) {
            break;
        }
        let mut target = target;
        loop {
            let edge = reached_by[target].expect("a reached target has an edge");
            let row = edges[edge].row;
            let moved_from = row_edge[row].replace(edge);
            target_edge[target] = Some(edge);
            match moved_from {
                None => break,
                Some(previous) => target = edges[previous].target,
            }
        }
    }
}

/// Bellman-Ford over the residual graph of the `usable` edges: a free row starts at zero, an edge
/// not taken leads from its row to its target at its cost, and a taken edge leads back from its
/// target to its row at minus its cost. Returns the closest free target, what reaching it costs
/// and, per target, the edge that reached it.
fn cheapest_free_target<C>(
    edges: &[Edge<C>],
    usable: impl Fn(&Edge<C>) -> bool,
    row_edge: &[Option<usize>],
    target_edge: &[Option<usize>],
) -> Option<(usize, C, Vec<Option<usize>>)>
where
    C: Copy + Ord + Default + Add<Output = C> + Sub<Output = C>,
{
    let mut row_cost: Vec<Option<C>> = row_edge
        .iter()
        .map(|taken| taken.is_none().then(C::default))
        .collect();
    let mut target_cost: Vec<Option<C>> = vec![None; target_edge.len()];
    let mut reached_by: Vec<Option<usize>> = vec![None; target_edge.len()];
    for _ in 0..=row_edge.len() + target_edge.len() {
        let mut improved = false;
        for (index, edge) in edges.iter().enumerate() {
            if !usable(edge) {
                continue;
            }
            if row_edge[edge.row] == Some(index) {
                if let Some(cost) = target_cost[edge.target].map(|cost| cost - edge.cost) {
                    if row_cost[edge.row].is_none_or(|known| cost < known) {
                        row_cost[edge.row] = Some(cost);
                        improved = true;
                    }
                }
            } else if let Some(cost) = row_cost[edge.row].map(|cost| cost + edge.cost) {
                if target_cost[edge.target].is_none_or(|known| cost < known) {
                    target_cost[edge.target] = Some(cost);
                    reached_by[edge.target] = Some(index);
                    improved = true;
                }
            }
        }
        if !improved {
            break;
        }
    }
    let (cost, target) = (0..target_edge.len())
        .filter(|&target| target_edge[target].is_none())
        .filter_map(|target| Some((target_cost[target]?, target)))
        .min()?;
    Some((target, cost, reached_by))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken_with(
        edges: &[(usize, usize, i64, bool)],
        rows: usize,
        targets: usize,
        worth: Option<i64>,
    ) -> Vec<(usize, usize)> {
        let edges: Vec<Edge<i64>> = edges
            .iter()
            .map(|&(row, target, cost, sure)| Edge {
                row,
                target,
                cost,
                sure,
            })
            .collect();
        cheapest_pairs(&edges, rows, targets, worth)
            .into_iter()
            .map(|index| (edges[index].row, edges[index].target))
            .collect()
    }

    fn taken_worth(
        edges: &[(usize, usize, i64)],
        rows: usize,
        targets: usize,
        worth: Option<i64>,
    ) -> Vec<(usize, usize)> {
        let edges: Vec<_> = edges
            .iter()
            .map(|&(row, target, cost)| (row, target, cost, false))
            .collect();
        taken_with(&edges, rows, targets, worth)
    }

    fn taken(edges: &[(usize, usize, i64)], rows: usize, targets: usize) -> Vec<(usize, usize)> {
        taken_worth(edges, rows, targets, None)
    }

    #[test]
    fn a_pair_is_taken_only_while_it_costs_less_than_its_worth() {
        assert_eq!(taken_worth(&[(0, 0, 10)], 1, 1, Some(10)), []);
        assert_eq!(taken_worth(&[(0, 0, 9)], 1, 1, Some(10)), [(0, 0)]);
        assert_eq!(
            taken_worth(&[(0, 0, 1), (0, 1, 9), (1, 0, 3)], 2, 2, Some(10)),
            [(0, 0)],
            "a second pair would cost 11 more"
        );
        assert_eq!(
            taken_worth(&[(0, 0, 1), (0, 1, 9), (1, 0, 3)], 2, 2, Some(12)),
            [(0, 1), (1, 0)]
        );
    }

    #[test]
    fn sure_pairs_pair_whole_chains_whatever_the_worth() {
        let chain = [
            (0, 0, 5, true),
            (1, 0, 4, true),
            (1, 1, 5, true),
            (2, 0, 3, true),
            (2, 1, 4, true),
            (2, 2, 5, true),
        ];
        assert_eq!(taken_with(&chain, 3, 3, Some(6)), [(0, 0), (1, 1), (2, 2)]);
    }

    #[test]
    fn sure_pairs_never_move_a_pair_that_is_not_sure() {
        assert_eq!(
            taken_with(
                &[(0, 0, 1, false), (0, 1, 9, true), (1, 0, 3, true)],
                2,
                2,
                Some(10)
            ),
            [(0, 0)]
        );
        assert_eq!(
            taken_with(
                &[(0, 0, 1, true), (0, 1, 9, true), (1, 0, 3, true)],
                2,
                2,
                Some(10)
            ),
            [(0, 1), (1, 0)]
        );
    }

    #[test]
    fn more_pairs_win_over_a_cheaper_pair() {
        assert_eq!(
            taken(&[(0, 0, 1), (0, 1, 5), (1, 0, 5)], 2, 2),
            [(0, 1), (1, 0)]
        );
    }

    #[test]
    fn of_the_largest_sets_the_cheapest_is_taken() {
        assert_eq!(
            taken(&[(0, 0, 1), (0, 1, 2), (1, 0, 2), (1, 1, 5)], 2, 2),
            [(0, 1), (1, 0)]
        );
        assert_eq!(taken(&[(0, 0, 3), (1, 0, 2)], 2, 1), [(1, 0)]);
    }

    #[test]
    fn equally_cheap_sets_follow_the_edges_order() {
        assert_eq!(
            taken(&[(0, 0, 1), (0, 1, 1), (1, 0, 1), (1, 1, 1)], 2, 2),
            [(0, 0), (1, 1)]
        );
        assert_eq!(taken(&[(0, 0, 1), (1, 0, 1)], 2, 1), [(0, 0)]);
        assert_eq!(taken(&[(0, 0, 1), (0, 1, 1)], 1, 2), [(0, 0)]);
    }

    #[test]
    fn a_matched_row_moves_along_a_longer_path() {
        assert_eq!(
            taken(
                &[(0, 0, 0), (0, 2, 9), (1, 0, 0), (1, 1, 0), (2, 1, 0)],
                3,
                3
            ),
            [(0, 2), (1, 0), (2, 1)]
        );
    }

    #[test]
    fn nothing_to_take() {
        assert_eq!(taken(&[], 3, 2), []);
    }
}
