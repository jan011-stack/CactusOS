//! Deterministic linear resource quotas for a fixed set of tasks.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceRequest {
    pub minimum: u64,
    pub demand: u32,
    pub maximum: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationSummary {
    pub allocated: u64,
    pub unused: u64,
    pub next_cursor: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationError {
    MinimumExceedsMaximum,
    MinimumsExceedBudget,
    ScoreOverflow,
}

/// Allocate a resource budget by the linear score `1 + demand`.
///
/// Every task receives its minimum first. Remaining units are distributed
/// proportionally to each eligible task's score, respecting its maximum. Any
/// integer-rounding remainder uses largest-remainder order, with `cursor`
/// rotating ties between calls. No heap allocation or floating-point math is
/// used.
pub fn allocate_linear<const N: usize>(
    requests: &[ResourceRequest; N],
    budget: u64,
    cursor: usize,
    allocations: &mut [u64; N],
) -> Result<AllocationSummary, AllocationError> {
    let mut minimum_total = 0u64;
    let mut total_score = 0u64;

    for request in requests {
        if request.minimum > request.maximum {
            return Err(AllocationError::MinimumExceedsMaximum);
        }
        minimum_total = minimum_total
            .checked_add(request.minimum)
            .ok_or(AllocationError::MinimumsExceedBudget)?;
        total_score = total_score
            .checked_add(u64::from(request.demand) + 1)
            .ok_or(AllocationError::ScoreOverflow)?;
    }
    if minimum_total > budget {
        return Err(AllocationError::MinimumsExceedBudget);
    }

    for (allocation, request) in allocations.iter_mut().zip(requests) {
        *allocation = request.minimum;
    }

    let mut remaining = budget - minimum_total;
    let mut next_cursor = if N == 0 { 0 } else { cursor % N };
    let mut remainders = [0u64; N];
    let mut remainder_awarded = [false; N];

    while remaining > 0 {
        let mut active_score = 0u64;
        for (index, request) in requests.iter().enumerate() {
            if allocations[index] < request.maximum {
                active_score += u64::from(request.demand) + 1;
            }
        }
        if active_score == 0 {
            break;
        }

        let round_budget = remaining;
        let mut saturated_any = false;
        for (index, request) in requests.iter().enumerate() {
            let capacity = request.maximum - allocations[index];
            if capacity == 0 {
                continue;
            }
            let score = u64::from(request.demand) + 1;
            let ideal_share = ((u128::from(round_budget) * u128::from(score))
                / u128::from(active_score)) as u64;
            if ideal_share >= capacity {
                allocations[index] += capacity;
                remaining -= capacity;
                saturated_any = true;
            }
        }
        if saturated_any {
            continue;
        }

        let mut distributed = 0u64;
        for (index, request) in requests.iter().enumerate() {
            if allocations[index] == request.maximum {
                continue;
            }
            let score = u64::from(request.demand) + 1;
            let numerator = u128::from(round_budget) * u128::from(score);
            let share = (numerator / u128::from(active_score)) as u64;
            allocations[index] += share;
            distributed += share;
            remainders[index] = (numerator % u128::from(active_score)) as u64;
        }
        remaining -= distributed;
        remainder_awarded.fill(false);

        while remaining > 0 {
            let mut best_index = None;
            let mut best_remainder = 0u64;
            for offset in 0..N {
                let index = (next_cursor + offset) % N;
                if remainder_awarded[index]
                    || allocations[index] >= requests[index].maximum
                {
                    continue;
                }
                if best_index.is_none() || remainders[index] > best_remainder {
                    best_index = Some(index);
                    best_remainder = remainders[index];
                }
            }
            let Some(index) = best_index else {
                break;
            };
            allocations[index] += 1;
            remaining -= 1;
            remainder_awarded[index] = true;
            next_cursor = (index + 1) % N;
        }
        break;
    }

    let allocated = allocations.iter().copied().sum();
    Ok(AllocationSummary {
        allocated,
        unused: budget - allocated,
        next_cursor,
    })
}

#[cfg(test)]
mod tests {
    use super::{allocate_linear, AllocationError, ResourceRequest};

    #[test]
    fn linear_scores_split_budget_and_round_deterministically() {
        let requests = [
            ResourceRequest { minimum: 0, demand: 0, maximum: 100 },
            ResourceRequest { minimum: 0, demand: 1, maximum: 100 },
            ResourceRequest { minimum: 0, demand: 3, maximum: 100 },
        ];
        let mut allocations = [0; 3];
        let summary = allocate_linear(&requests, 28, 0, &mut allocations).unwrap();

        assert_eq!(allocations, [4, 8, 16]);
        assert_eq!(summary.allocated, 28);
        assert_eq!(summary.unused, 0);
    }

    #[test]
    fn equal_scores_rotate_rounding_remainder() {
        let requests = [
            ResourceRequest { minimum: 0, demand: 0, maximum: 10 },
            ResourceRequest { minimum: 0, demand: 0, maximum: 10 },
            ResourceRequest { minimum: 0, demand: 0, maximum: 10 },
        ];
        let mut first = [0; 3];
        let first_summary = allocate_linear(&requests, 10, 0, &mut first).unwrap();
        let mut second = [0; 3];
        allocate_linear(&requests, 10, first_summary.next_cursor, &mut second).unwrap();

        assert_eq!(first, [4, 3, 3]);
        assert_eq!(second, [3, 4, 3]);
    }

    #[test]
    fn capped_task_releases_its_share_to_remaining_tasks() {
        let requests = [
            ResourceRequest { minimum: 0, demand: 0, maximum: 5 },
            ResourceRequest { minimum: 0, demand: 1, maximum: 64 },
            ResourceRequest { minimum: 0, demand: 3, maximum: 64 },
        ];
        let mut allocations = [0; 3];
        let summary = allocate_linear(&requests, 64, 0, &mut allocations).unwrap();

        assert_eq!(allocations, [5, 20, 39]);
        assert_eq!(summary.allocated, 64);
        assert_eq!(summary.unused, 0);
    }

    #[test]
    fn minimums_and_maximums_are_respected_and_unused_budget_is_reported() {
        let requests = [
            ResourceRequest { minimum: 2, demand: 1, maximum: 4 },
            ResourceRequest { minimum: 1, demand: 1, maximum: 3 },
        ];
        let mut allocations = [0; 2];
        let summary = allocate_linear(&requests, 12, 0, &mut allocations).unwrap();

        assert_eq!(allocations, [4, 3]);
        assert_eq!(summary.unused, 5);
    }

    #[test]
    fn invalid_minimums_are_rejected() {
        let requests = [ResourceRequest { minimum: 4, demand: 1, maximum: 3 }];
        let mut allocations = [0; 1];
        assert_eq!(
            allocate_linear(&requests, 10, 0, &mut allocations),
            Err(AllocationError::MinimumExceedsMaximum)
        );

        let requests = [ResourceRequest { minimum: 4, demand: 1, maximum: 8 }];
        assert_eq!(
            allocate_linear(&requests, 3, 0, &mut allocations),
            Err(AllocationError::MinimumsExceedBudget)
        );
    }
}