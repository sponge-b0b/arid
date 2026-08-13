use thiserror::Error;

use crate::corpus::CorpusToken;
use crate::model::CorpusPos;

#[derive(Debug, Error)]
pub enum SuffixError {
    #[error("suffix array exceeds u32 indexing capacity")]
    TooLarge,

    #[error("suffix array length {actual} does not match token length {expected}")]
    InvalidLength { expected: usize, actual: usize },

    #[error("suffix array position {position} is outside corpus length {length}")]
    InvalidPosition { position: CorpusPos, length: usize },

    #[error("suffix array contains duplicate position {0}")]
    DuplicatePosition(CorpusPos),
}

/// Builds the suffix array for `tokens`.
///
/// The returned positions are ordered lexicographically by the suffix
/// beginning at each position.
///
/// Construction uses prefix doubling. The initial token classes are obtained
/// with one comparison sort; subsequent doubling rounds use stable counting
/// sorts over compact integer ranks.
pub fn build_suffix_array(tokens: &[CorpusToken]) -> Result<Vec<CorpusPos>, SuffixError> {
    let length = tokens.len();

    if length == 0 {
        return Ok(Vec::new());
    }

    let length_u32 = u32::try_from(length).map_err(|_| SuffixError::TooLarge)?;

    let mut suffixes: Vec<CorpusPos> = (0..length_u32).collect();

    // Establish deterministic initial ordering. Position is included as a
    // tiebreaker so equal first tokens always begin in source order.
    suffixes.sort_unstable_by_key(|&position| (tokens[position as usize], position));

    let mut ranks = vec![0_u32; length];
    let mut class_count = assign_initial_ranks(tokens, &suffixes, &mut ranks)?;

    if class_count == length {
        return Ok(suffixes);
    }

    let mut scratch = vec![0_u32; length];
    let mut next_ranks = vec![0_u32; length];
    let mut counts = Vec::new();

    let mut width = 1_usize;

    loop {
        let key_count = class_count.checked_add(1).ok_or(SuffixError::TooLarge)?;

        // Radix-sort rank pairs:
        //
        //     (
        //         rank[position],
        //         rank[position + width]
        //     )
        //
        // Missing second ranks sort before real ranks by using key 0.
        // Real ranks are shifted by one.
        counting_sort(
            &suffixes,
            &mut scratch,
            key_count,
            &mut counts,
            |position| second_rank_key(&ranks, position as usize, width),
        );

        counting_sort(
            &scratch,
            &mut suffixes,
            key_count,
            &mut counts,
            |position| rank_key(ranks[position as usize]),
        );

        class_count = assign_doubled_ranks(&suffixes, &ranks, &mut next_ranks, width)?;

        std::mem::swap(&mut ranks, &mut next_ranks);

        if class_count == length {
            break;
        }

        width = width.checked_mul(2).ok_or(SuffixError::TooLarge)?;
    }

    Ok(suffixes)
}

/// Builds the longest-common-prefix array for `tokens` and `suffixes`.
///
/// `lcp[0]` is always zero. For every `i > 0`, `lcp[i]` is the number
/// of leading tokens shared by the suffixes at `suffixes[i - 1]` and
/// `suffixes[i]`.
///
/// Construction uses Kasai's algorithm and runs in O(n) time.
pub fn build_lcp_array(
    tokens: &[CorpusToken],
    suffixes: &[CorpusPos],
) -> Result<Vec<u32>, SuffixError> {
    let length = tokens.len();

    if suffixes.len() != length {
        return Err(SuffixError::InvalidLength {
            expected: length,
            actual: suffixes.len(),
        });
    }

    if length == 0 {
        return Ok(Vec::new());
    }

    let mut ranks = vec![usize::MAX; length];

    for (rank, &position) in suffixes.iter().enumerate() {
        let index = position as usize;

        if index >= length {
            return Err(SuffixError::InvalidPosition { position, length });
        }

        if ranks[index] != usize::MAX {
            return Err(SuffixError::DuplicatePosition(position));
        }

        ranks[index] = rank;
    }

    let mut lcp = vec![0_u32; length];
    let mut common = 0_usize;

    for position in 0..length {
        let rank = ranks[position];

        // The lexicographically first suffix has no previous suffix.
        if rank == 0 {
            common = 0;
            continue;
        }

        let previous = suffixes[rank - 1] as usize;

        while position + common < length
            && previous + common < length
            && tokens[position + common] == tokens[previous + common]
        {
            common += 1;
        }

        lcp[rank] = u32::try_from(common).map_err(|_| SuffixError::TooLarge)?;

        // If suffixes at i and j share `common` leading tokens, suffixes at
        // i + 1 and j + 1 share at least `common - 1`. Reusing that fact is
        // what keeps Kasai's algorithm linear.
        common = common.saturating_sub(1);
    }

    Ok(lcp)
}

fn assign_initial_ranks(
    tokens: &[CorpusToken],
    suffixes: &[CorpusPos],
    ranks: &mut [u32],
) -> Result<usize, SuffixError> {
    let mut class = 0_u32;

    ranks[suffixes[0] as usize] = class;

    for pair in suffixes.windows(2) {
        let previous = pair[0] as usize;
        let current = pair[1] as usize;

        if tokens[previous] != tokens[current] {
            class = class.checked_add(1).ok_or(SuffixError::TooLarge)?;
        }

        ranks[current] = class;
    }

    usize::try_from(class)
        .map_err(|_| SuffixError::TooLarge)?
        .checked_add(1)
        .ok_or(SuffixError::TooLarge)
}

fn assign_doubled_ranks(
    suffixes: &[CorpusPos],
    ranks: &[u32],
    next_ranks: &mut [u32],
    width: usize,
) -> Result<usize, SuffixError> {
    let mut class = 0_u32;

    next_ranks[suffixes[0] as usize] = class;

    for pair in suffixes.windows(2) {
        let previous = pair[0] as usize;
        let current = pair[1] as usize;

        if rank_pair(ranks, previous, width) != rank_pair(ranks, current, width) {
            class = class.checked_add(1).ok_or(SuffixError::TooLarge)?;
        }

        next_ranks[current] = class;
    }

    usize::try_from(class)
        .map_err(|_| SuffixError::TooLarge)?
        .checked_add(1)
        .ok_or(SuffixError::TooLarge)
}

fn rank_pair(ranks: &[u32], position: usize, width: usize) -> (usize, usize) {
    (
        rank_key(ranks[position]),
        second_rank_key(ranks, position, width),
    )
}

fn rank_key(rank: u32) -> usize {
    rank as usize + 1
}

fn second_rank_key(ranks: &[u32], position: usize, width: usize) -> usize {
    position
        .checked_add(width)
        .filter(|&next| next < ranks.len())
        .map_or(0, |next| rank_key(ranks[next]))
}

fn counting_sort<F>(
    input: &[CorpusPos],
    output: &mut [CorpusPos],
    key_count: usize,
    counts: &mut Vec<usize>,
    key: F,
) where
    F: Fn(CorpusPos) -> usize,
{
    counts.clear();
    counts.resize(key_count, 0);

    for &position in input {
        counts[key(position)] += 1;
    }

    let mut offset = 0_usize;

    for count in counts.iter_mut() {
        let occurrences = *count;
        *count = offset;
        offset += occurrences;
    }

    for &position in input {
        let key = key(position);
        output[counts[key]] = position;
        counts[key] += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_suffix_array(tokens: &[CorpusToken]) -> Vec<CorpusPos> {
        let mut suffixes: Vec<CorpusPos> = (0..tokens.len() as u32).collect();

        suffixes.sort_by(|&left, &right| tokens[left as usize..].cmp(&tokens[right as usize..]));

        suffixes
    }

    #[test]
    fn empty_sequence_has_empty_suffix_array() {
        assert_eq!(build_suffix_array(&[]).unwrap(), Vec::<u32>::new());
    }

    #[test]
    fn single_token_has_one_suffix() {
        assert_eq!(build_suffix_array(&[42]).unwrap(), vec![0]);
    }

    #[test]
    fn orders_distinct_tokens() {
        let tokens = [30, 10, 20];

        assert_eq!(build_suffix_array(&tokens).unwrap(), vec![1, 2, 0]);
    }

    #[test]
    fn orders_repeated_tokens_by_remaining_suffix() {
        let tokens = [1, 1, 1, 1];

        assert_eq!(build_suffix_array(&tokens).unwrap(), vec![3, 2, 1, 0]);
    }

    #[test]
    fn matches_reference_for_mixed_sequence() {
        let tokens = [2, 1, 2, 1, 0, 2, 1];

        assert_eq!(
            build_suffix_array(&tokens).unwrap(),
            reference_suffix_array(&tokens)
        );
    }

    #[test]
    fn handles_unique_segment_sentinels_as_normal_tokens() {
        let tokens = [0, 1, 4, 0, 1, 5, 0, 2, 6];

        assert_eq!(
            build_suffix_array(&tokens).unwrap(),
            reference_suffix_array(&tokens)
        );
    }

    #[test]
    fn handles_long_repetitive_sequences() {
        let tokens = vec![7; 128];

        assert_eq!(
            build_suffix_array(&tokens).unwrap(),
            reference_suffix_array(&tokens)
        );
    }

    #[test]
    fn matches_reference_exhaustively_for_small_sequences() {
        const ALPHABET_SIZE: usize = 3;

        for length in 0..=7 {
            let sequence_count = ALPHABET_SIZE.pow(length as u32);

            for encoded in 0..sequence_count {
                let mut value = encoded;
                let mut tokens = vec![0_u32; length];

                for token in &mut tokens {
                    *token = (value % ALPHABET_SIZE) as u32;
                    value /= ALPHABET_SIZE;
                }

                let expected_suffixes = reference_suffix_array(&tokens);

                let actual_suffixes = build_suffix_array(&tokens).unwrap();

                assert_eq!(
                    actual_suffixes, expected_suffixes,
                    "suffix array failed for {tokens:?}"
                );

                let expected_lcp = reference_lcp_array(&tokens, &expected_suffixes);

                let actual_lcp = build_lcp_array(&tokens, &actual_suffixes).unwrap();

                assert_eq!(actual_lcp, expected_lcp, "LCP array failed for {tokens:?}");
            }
        }
    }

    fn reference_lcp_array(tokens: &[CorpusToken], suffixes: &[CorpusPos]) -> Vec<u32> {
        let mut lcp = vec![0_u32; suffixes.len()];

        for index in 1..suffixes.len() {
            let left = suffixes[index - 1] as usize;
            let right = suffixes[index] as usize;

            let mut common = 0_usize;

            while left + common < tokens.len()
                && right + common < tokens.len()
                && tokens[left + common] == tokens[right + common]
            {
                common += 1;
            }

            lcp[index] = common as u32;
        }

        lcp
    }

    #[test]
    fn empty_sequence_has_empty_lcp_array() {
        let suffixes = build_suffix_array(&[]).unwrap();

        assert_eq!(build_lcp_array(&[], &suffixes).unwrap(), Vec::<u32>::new());
    }

    #[test]
    fn single_token_has_zero_lcp() {
        let tokens = [42];
        let suffixes = build_suffix_array(&tokens).unwrap();

        assert_eq!(build_lcp_array(&tokens, &suffixes).unwrap(), vec![0]);
    }

    #[test]
    fn computes_known_lcp_array() {
        let tokens = [1, 2, 1, 2, 1];

        let suffixes = build_suffix_array(&tokens).unwrap();
        let lcp = build_lcp_array(&tokens, &suffixes).unwrap();

        assert_eq!(suffixes, vec![4, 2, 0, 3, 1]);

        assert_eq!(lcp, vec![0, 1, 3, 0, 2]);
    }

    #[test]
    fn computes_lcp_for_repeated_tokens() {
        let tokens = [7, 7, 7, 7];

        let suffixes = build_suffix_array(&tokens).unwrap();

        assert_eq!(suffixes, vec![3, 2, 1, 0]);

        assert_eq!(
            build_lcp_array(&tokens, &suffixes).unwrap(),
            vec![0, 1, 2, 3]
        );
    }

    #[test]
    fn lcp_matches_reference_for_mixed_sequence() {
        let tokens = [2, 1, 2, 1, 0, 2, 1];

        let suffixes = build_suffix_array(&tokens).unwrap();

        assert_eq!(
            build_lcp_array(&tokens, &suffixes).unwrap(),
            reference_lcp_array(&tokens, &suffixes)
        );
    }

    #[test]
    fn rejects_lcp_suffix_array_with_wrong_length() {
        let error = build_lcp_array(&[1, 2, 3], &[0, 1]).unwrap_err();

        assert!(matches!(
            error,
            SuffixError::InvalidLength {
                expected: 3,
                actual: 2,
            }
        ));
    }

    #[test]
    fn rejects_out_of_range_suffix_position() {
        let error = build_lcp_array(&[1, 2, 3], &[0, 1, 3]).unwrap_err();

        assert!(matches!(
            error,
            SuffixError::InvalidPosition {
                position: 3,
                length: 3,
            }
        ));
    }

    #[test]
    fn rejects_duplicate_suffix_position() {
        let error = build_lcp_array(&[1, 2, 3], &[0, 1, 1]).unwrap_err();

        assert!(matches!(error, SuffixError::DuplicatePosition(1)));
    }
}
