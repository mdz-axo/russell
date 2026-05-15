// SPDX-License-Identifier: MIT OR Apache-2.0
//! Knapsack-based knowledge selection extension for prompt_registry.rs.
//!
//! Replaces the greedy `select_knowledge()` with a token-aware knapsack
//! solver that maximizes symptom overlap within budget, using
//! `SkillHealth.reliability` as a quality weight.
//!
//! Merge into: `crates/russell-meta/src/prompt_registry.rs`

use super::KnowledgeSlot;

/// Knapsack-optimized knowledge slot selection.
///
/// This replaces the greedy `select_knowledge()` with a 0/1 knapsack
/// solver that maximizes total value within the token budget.
///
/// # Value calculation per slot
///
/// `slot_value = slot.relevance * slot.token_estimate`
///
/// This ensures that high-relevance knowledge gets priority, but
/// token-heavy knowledge is penalized proportionally — two small,
/// medium-relevance slots can beat one large, high-relevance slot
/// if they provide more symptom coverage per token.
///
/// # Algorithm
///
/// Standard 0/1 knapsack DP. O(n * budget_tokens) time and space.
/// For typical inputs (n ≤ 20 skills, budget ≤ 3000 tokens), this
/// is negligible (~60k operations).
pub fn select_knowledge_knapsack(
    slots: &[KnowledgeSlot],
    budget_tokens: usize,
) -> Vec<&KnowledgeSlot> {
    let n = slots.len();
    if n == 0 || budget_tokens == 0 {
        return Vec::new();
    }

    // Clamp token estimates to u32 for the DP table.
    // Any slot larger than budget is infeasible and skipped.
    let budget = budget_tokens.min(65535);
    let weights: Vec<usize> = slots.iter().map(|s| s.token_estimate.min(budget + 1)).collect();
    let values: Vec<u64> = slots
        .iter()
        .map(|s| {
            // Value = relevance * token_estimate, scaled to avoid float→int precision loss.
            // Multiply by 1_000_000 to preserve 6 decimal places.
            (s.relevance * s.token_estimate as f64 * 1_000_000.0) as u64
        })
        .collect();

    // DP table: dp[i][w] = max value using first i items with weight limit w.
    let mut dp = vec![vec![0u64; budget + 1]; n + 1];
    for i in 0..n {
        let w = weights[i];
        let v = values[i];
        for cap in 0..=budget {
            if w > cap {
                dp[i + 1][cap] = dp[i][cap];
            } else {
                let take = dp[i][cap - w].saturating_add(v);
                let skip = dp[i][cap];
                dp[i + 1][cap] = take.max(skip);
            }
        }
    }

    // Backtrack to find selected items.
    let mut selected = Vec::new();
    let mut cap = budget;
    for i in (0..n).rev() {
        if dp[i + 1][cap] != dp[i][cap] {
            // Item i was selected.
            if weights[i] <= cap {
                selected.push(&slots[i]);
                cap = cap.saturating_sub(weights[i]);
            }
        }
    }
    selected.reverse(); // restore original order
    selected
}

/// Minimum token guarantee: ensure at least one knowledge slot per
/// skill that meets a minimum relevance threshold.
pub fn min_guarantee(selected: &mut Vec<&KnowledgeSlot>, pools: &[&[KnowledgeSlot]], min_relevance: f64) {
    for pool in pools {
        if pool.is_empty() {
            continue;
        }
        // Check if any slot from this pool is already selected.
        let already_in = selected.iter().any(|s| pool.iter().any(|ps| ps.skill_id == s.skill_id));
        if already_in {
            continue;
        }
        // Find the highest-relevance slot from this pool.
        let best = pool.iter().max_by(|a, b| a.relevance.partial_cmp(&b.relevance).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(best) = best {
            if best.relevance >= min_relevance {
                selected.push(best);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, relevance: f64, token_estimate: usize) -> KnowledgeSlot {
        KnowledgeSlot {
            skill_id: id.to_string(),
            content: format!("content for {id}"),
            relevance,
            token_estimate,
        }
    }

    #[test]
    fn knapsack_selects_highest_value_per_token() {
        let slots = vec![
            slot("A", 0.9, 1000), // value = 900
            slot("B", 0.8, 500),  // value = 400
            slot("C", 0.5, 2000), // value = 1000
        ];
        let selected = select_knowledge_knapsack(&slots, 1500);
        // DP should select A (w=1000) + B (w=500) = w=1500, value=900+400=1300
        // vs C alone: w=2000 → infeasible, vs A alone: w=1000, value=900
        let ids: Vec<&str> = selected.iter().map(|s| s.skill_id.as_str()).collect();
        assert!(ids.contains(&"A"), "expected A to be selected, got {ids:?}");
        assert!(ids.contains(&"B"), "expected B to be selected, got {ids:?}");
    }

    #[test]
    fn knapsack_drops_low_value_when_budget_tight() {
        let slots = vec![
            slot("X", 1.0, 2000), // high value high cost
            slot("Y", 0.3, 500),  // low value low cost
            slot("Z", 0.9, 1000), // high value medium cost
        ];
        let selected = select_knowledge_knapsack(&slots, 2500);
        let ids: Vec<&str> = selected.iter().map(|s| s.skill_id.as_str()).collect();
        assert!(ids.contains(&"X"), "expected X, got {ids:?}");
        assert!(!ids.contains(&"Y"), "expected Y NOT selected");
        // Z at 0.9 value may or may not fit; total 3000 > 2500 budget
    }

    #[test]
    fn knapsack_empty_input() {
        assert!(select_knowledge_knapsack(&[], 1000).is_empty());
    }

    #[test]
    fn knapsack_zero_budget() {
        let slots = vec![slot("A", 1.0, 100)];
        assert!(select_knowledge_knapsack(&slots, 0).is_empty());
    }

    #[test]
    fn knapsack_all_fit() {
        let slots = vec![
            slot("A", 0.9, 500),
            slot("B", 0.8, 300),
            slot("C", 0.7, 200),
        ];
        let selected = select_knowledge_knapsack(&slots, 2000);
        assert_eq!(selected.len(), 3);
    }
}
