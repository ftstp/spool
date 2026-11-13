//! Python's `difflib.SequenceMatcher` (no junk function, autojunk on), on characters: the
//! song grouping depends on its exact ratios, so it is reproduced step for step.
use std::collections::HashMap;

pub struct SequenceMatcher {
    a: Vec<char>,
    b: Vec<char>,
    b2j: HashMap<char, Vec<usize>>,
}

impl SequenceMatcher {
    pub fn new(a: &str, b: &str) -> SequenceMatcher {
        let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
        let mut b2j: HashMap<char, Vec<usize>> = HashMap::new();
        for (i, &c) in b.iter().enumerate() {
            b2j.entry(c).or_default().push(i);
        }
        // autojunk: in sequences of 200+ items, items making more than 1% of them are "popular"
        let n = b.len();
        if n >= 200 {
            let ntest = n / 100 + 1;
            b2j.retain(|_, idx| idx.len() <= ntest);
        }
        SequenceMatcher { a, b, b2j }
    }

    fn find_longest_match(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> (usize, usize, usize) {
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut newj2len: HashMap<usize, usize> = HashMap::new();
            if let Some(js) = self.b2j.get(&self.a[i]) {
                for &j in js {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let k = j.checked_sub(1).and_then(|p| j2len.get(&p)).copied().unwrap_or(0) + 1;
                    newj2len.insert(j, k);
                    if k > bestsize {
                        besti = i + 1 - k;
                        bestj = j + 1 - k;
                        bestsize = k;
                    }
                }
            }
            j2len = newj2len;
        }
        // No junk function, so nothing is junk: the match extends over any equal items on
        // either side (popular ones included, which b2j left out).
        while besti > alo && bestj > blo && self.a[besti - 1] == self.b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi && bestj + bestsize < bhi && self.a[besti + bestsize] == self.b[bestj + bestsize] {
            bestsize += 1;
        }
        (besti, bestj, bestsize)
    }

    fn matching_blocks(&self) -> Vec<(usize, usize, usize)> {
        let (la, lb) = (self.a.len(), self.b.len());
        let mut queue = vec![(0, la, 0, lb)];
        let mut blocks = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let (i, j, k) = self.find_longest_match(alo, ahi, blo, bhi);
            if k > 0 {
                blocks.push((i, j, k));
                if alo < i && blo < j {
                    queue.push((alo, i, blo, j));
                }
                if i + k < ahi && j + k < bhi {
                    queue.push((i + k, ahi, j + k, bhi));
                }
            }
        }
        blocks.sort();
        // collapse adjacent blocks
        let mut out: Vec<(usize, usize, usize)> = Vec::new();
        for (i, j, k) in blocks {
            if let Some(last) = out.last_mut()
                && last.0 + last.2 == i
                && last.1 + last.2 == j
            {
                last.2 += k;
                continue;
            }
            out.push((i, j, k));
        }
        out
    }

    fn total(&self) -> usize {
        self.a.len() + self.b.len()
    }

    fn calc(matches: usize, length: usize) -> f64 {
        if length > 0 { 2.0 * matches as f64 / length as f64 } else { 1.0 }
    }

    pub fn ratio(&self) -> f64 {
        Self::calc(self.matching_blocks().iter().map(|b| b.2).sum(), self.total())
    }

    pub fn quick_ratio(&self) -> f64 {
        let mut full: HashMap<char, i64> = HashMap::new();
        for &c in &self.b {
            *full.entry(c).or_default() += 1;
        }
        let mut avail: HashMap<char, i64> = HashMap::new();
        let mut matches = 0;
        for &c in &self.a {
            let n = *avail.get(&c).unwrap_or_else(|| full.get(&c).unwrap_or(&0));
            avail.insert(c, n - 1);
            if n > 0 {
                matches += 1;
            }
        }
        Self::calc(matches, self.total())
    }

    pub fn real_quick_ratio(&self) -> f64 {
        Self::calc(self.a.len().min(self.b.len()), self.total())
    }
}

#[cfg(test)]
mod tests {
    use super::SequenceMatcher as S;

    #[test]
    fn ratios_match_python() {
        // values printed by CPython's difflib
        assert_eq!(S::new("abcd", "bcde").ratio(), 0.75);
        assert_eq!(S::new("noisia style", "noisia style 2").ratio(), 2.0 * 12.0 / 26.0);
        assert_eq!(S::new("fit", "fume").ratio(), 2.0 * 1.0 / 7.0);
    }
}
