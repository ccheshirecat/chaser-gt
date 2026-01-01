//! Gobang (Five-in-a-row) puzzle solver for Geetest captcha.
//!
//! The puzzle presents a board where the user needs to find a row/column/diagonal
//! with n-1 matching elements and one empty cell (0), then move a matching piece
//! from elsewhere to fill the gap.

use std::collections::HashMap;

/// Solver for Gobang/Five-in-a-row captcha puzzles.
pub struct GobangSolver {
    board: Vec<Vec<i32>>,
    n: usize,
}

impl GobangSolver {
    /// Create a new Gobang solver with the given board.
    ///
    /// # Arguments
    /// * `board` - 2D array representing the puzzle board
    pub fn new(board: Vec<Vec<i32>>) -> Self {
        let n = board.len();
        Self { board, n }
    }

    /// Find the solution: a piece to remove and position to fill.
    ///
    /// # Returns
    /// `Some([[remove_row, remove_col], [fill_row, fill_col]])` if found, `None` otherwise.
    pub fn find_four_in_line(&self) -> Option<[[i32; 2]; 2]> {
        for line in self.iterate_lines() {
            if line.len() < self.n {
                continue;
            }

            let elements: Vec<i32> = line.iter().map(|&(r, c)| self.board[r][c]).collect();
            let freq = Self::count_freq(&elements);

            // Look for a line with n-1 matching elements and one 0
            if !freq.values().any(|&count| count == self.n - 1) {
                continue;
            }

            // Skip if n-1 zeros (can't fill)
            if freq.get(&0) == Some(&(self.n - 1)) {
                continue;
            }

            // Find the number that appears n-1 times (not 0)
            let correct_num = freq
                .iter()
                .find(|(&num, &count)| count == self.n - 1 && num != 0)
                .map(|(&num, _)| num);

            if let Some(correct_num) = correct_num {
                // Find the position of 0 in this line
                let zero_idx = elements.iter().position(|&x| x == 0);

                if let Some(zero_idx) = zero_idx {
                    let fill_pos = line[zero_idx];

                    // Find a piece to remove (same value, not in this line)
                    if let Some(remove_pos) = self.find_remove_candidate(correct_num, &line) {
                        return Some([
                            [remove_pos.0 as i32, remove_pos.1 as i32],
                            [fill_pos.0 as i32, fill_pos.1 as i32],
                        ]);
                    }
                }
            }
        }

        None
    }

    /// Iterate over all possible lines (rows, columns, diagonals).
    fn iterate_lines(&self) -> impl Iterator<Item = Vec<(usize, usize)>> + '_ {
        let n = self.n;

        // Rows
        let rows = (0..n).map(move |row| (0..n).map(|col| (row, col)).collect::<Vec<_>>());

        // Columns
        let cols = (0..n).map(move |col| (0..n).map(|row| (row, col)).collect::<Vec<_>>());

        // Main diagonals (top-left to bottom-right)
        let main_diag_1 = (0..n).map(move |start_row| {
            (0..(n - start_row))
                .map(|i| (start_row + i, i))
                .collect::<Vec<_>>()
        });

        let main_diag_2 = (1..n).map(move |start_col| {
            (0..(n - start_col))
                .map(|i| (i, start_col + i))
                .collect::<Vec<_>>()
        });

        // Anti-diagonals (top-right to bottom-left)
        let anti_diag_1 = (0..n).map(move |start_row| {
            (0..=start_row)
                .map(|i| (start_row - i, i))
                .collect::<Vec<_>>()
        });

        let anti_diag_2 = (1..n).map(move |start_col| {
            (0..(n - start_col))
                .map(|i| (n - 1 - i, start_col + i))
                .collect::<Vec<_>>()
        });

        rows.chain(cols)
            .chain(main_diag_1)
            .chain(main_diag_2)
            .chain(anti_diag_1)
            .chain(anti_diag_2)
    }

    /// Count frequency of each value in the elements.
    fn count_freq(elements: &[i32]) -> HashMap<i32, usize> {
        let mut freq = HashMap::new();
        for &num in elements {
            *freq.entry(num).or_insert(0) += 1;
        }
        freq
    }

    /// Find a position with the target value that's not in the excluded line.
    fn find_remove_candidate(
        &self,
        target: i32,
        exclude: &[(usize, usize)],
    ) -> Option<(usize, usize)> {
        let exclude_set: std::collections::HashSet<_> = exclude.iter().cloned().collect();

        for r in 0..self.n {
            for c in 0..self.n {
                if !exclude_set.contains(&(r, c)) && self.board[r][c] == target {
                    return Some((r, c));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gobang_solver_basic() {
        // Create a board where row 0 has [1, 1, 1, 0, 1] (needs to fill position 3)
        // and there's a 1 elsewhere to remove
        let board = vec![
            vec![1, 1, 1, 0, 1],
            vec![2, 2, 2, 2, 1], // Extra 1 at (1, 4) to remove
            vec![3, 3, 3, 3, 3],
            vec![4, 4, 4, 4, 4],
            vec![5, 5, 5, 5, 5],
        ];

        let solver = GobangSolver::new(board);
        let result = solver.find_four_in_line();

        assert!(result.is_some());
        let [[remove_r, remove_c], [fill_r, fill_c]] = result.unwrap();

        // Should fill position (0, 3) which has 0
        assert_eq!(fill_r, 0);
        assert_eq!(fill_c, 3);
    }

    #[test]
    fn test_count_freq() {
        let elements = vec![1, 1, 1, 0, 2];
        let freq = GobangSolver::count_freq(&elements);

        assert_eq!(freq.get(&1), Some(&3));
        assert_eq!(freq.get(&0), Some(&1));
        assert_eq!(freq.get(&2), Some(&1));
    }

    #[test]
    fn test_empty_board() {
        let board = vec![vec![0; 5]; 5];
        let solver = GobangSolver::new(board);
        let result = solver.find_four_in_line();

        // All zeros, can't solve
        assert!(result.is_none());
    }
}
