//! 骰子：规则套件掷骰都走这里。种子定下以后每一颗的点数就定了，状态随存档走，读档后接着掷的和没读档一样。
//! 模型和页面碰不到它，只看得到掷出来的结果。
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dice {
    state: u64,
}

impl Dice {
    pub fn new(seed: u32) -> Self {
        Self {
            state: u64::from(seed.max(1)).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// splitmix64
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 掷一颗 sides 面的骰子：1 到 sides，每一面一样可能（落在凑不成整倍数的那一小段就重掷）
    pub fn roll(&mut self, sides: u32) -> u32 {
        let sides = u64::from(sides.max(1));
        let limit = u64::MAX - u64::MAX % sides;
        loop {
            let x = self.next();
            if x < limit {
                return (x % sides) as u32 + 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 同一个种子掷出同一串；存档读档后接着掷的不变；二十面骰每一面出现的次数差不多。
    #[test]
    fn dice_are_reproducible_and_fair() {
        let (mut a, mut b) = (Dice::new(7), Dice::new(7));
        let first: Vec<u32> = (0..50).map(|_| a.roll(20)).collect();
        assert_eq!(first, (0..50).map(|_| b.roll(20)).collect::<Vec<_>>());
        assert_ne!(
            first,
            (0..50).map(|_| Dice::new(8).roll(20)).collect::<Vec<_>>()
        );
        let saved = serde_json::to_string(&a).unwrap();
        let mut loaded: Dice = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            (0..20).map(|_| a.roll(6)).collect::<Vec<_>>(),
            (0..20).map(|_| loaded.roll(6)).collect::<Vec<_>>()
        );
        let mut counts = [0u32; 20];
        let mut dice = Dice::new(2026);
        for _ in 0..200_000 {
            counts[dice.roll(20) as usize - 1] += 1;
        }
        // 每一面该有一万次上下，差不出 5%
        assert!(
            counts.iter().all(|c| (9_500..=10_500).contains(c)),
            "{counts:?}"
        );
    }
}
