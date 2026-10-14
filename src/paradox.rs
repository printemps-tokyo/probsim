//! The paradoxes: what each one asks, its exact answer, and one random trial.

use crate::rng::Rng;
use crate::stats::harmonic;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Ja,
    En,
}

impl Lang {
    /// Japanese when the first set of LC_ALL, LC_MESSAGES and LANG starts with "ja".
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Lang {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|k| get(k))
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        if locale.starts_with("ja") {
            Lang::Ja
        } else {
            Lang::En
        }
    }
}

/// What a trial produces and how the estimate is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// Each trial is 1 (it happened) or 0; the estimate is a probability.
    Probability,
    /// Each trial is a number; the estimate is its mean.
    Mean,
}

/// The words around a result.
pub struct Texts {
    pub title: String,
    pub question: String,
    pub intuition: String,
    pub truth: String,
    pub why: String,
    /// What one trial is called, plural ("games", "groups").
    pub unit: String,
    /// What the estimate is ("switching wins").
    pub quantity: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paradox {
    MontyHall { doors: u32, host_knows: bool },
    Birthday { people: u32, days: u32 },
    Coupon { items: u32 },
    Secretary { candidates: u32, skip: u32 },
    Buffon { length: f64, spacing: f64 },
}

/// Success probability of the secretary rule "reject the first `skip`, then
/// take the first who beats them all" with `n` candidates:
/// (k/n) * sum_{j=k}^{n-1} 1/j for k >= 1, and 1/n for k = 0.
pub fn secretary_success(n: u32, skip: u32) -> f64 {
    let n_f = n as f64;
    if skip == 0 {
        return 1.0 / n_f;
    }
    if skip >= n {
        return 0.0;
    }
    let tail: f64 = (skip..n).rev().map(|j| 1.0 / j as f64).sum();
    skip as f64 / n_f * tail
}

/// The cutoff that maximizes `secretary_success`.
pub fn secretary_best_skip(n: u32) -> u32 {
    (0..n)
        .max_by(|&a, &b| {
            secretary_success(n, a)
                .partial_cmp(&secretary_success(n, b))
                .unwrap()
        })
        .unwrap_or(0)
}

/// P(at least two of `people` share a birthday) with `days` equally likely days.
pub fn birthday_probability(people: u32, days: u32) -> f64 {
    if people > days {
        return 1.0;
    }
    let mut all_different = 1.0;
    for k in 0..people {
        all_different *= (days - k) as f64 / days as f64;
    }
    1.0 - all_different
}

/// "another door, a goat" or "N other doors, all goats".
fn others(k: u32) -> String {
    if k == 1 {
        "another door, a goat".to_string()
    } else {
        format!("{k} other doors, all goats")
    }
}

fn pct(p: f64) -> String {
    format!("{:.1}%", p * 100.0)
}

impl Paradox {
    pub fn name(&self) -> &'static str {
        match self {
            Paradox::MontyHall { .. } => "monty-hall",
            Paradox::Birthday { .. } => "birthday",
            Paradox::Coupon { .. } => "coupon",
            Paradox::Secretary { .. } => "secretary",
            Paradox::Buffon { .. } => "buffon",
        }
    }

    pub fn measure(&self) -> Measure {
        match self {
            Paradox::Coupon { .. } => Measure::Mean,
            _ => Measure::Probability,
        }
    }

    /// The exact answer.
    pub fn theory(&self) -> f64 {
        match *self {
            Paradox::MontyHall { doors, host_knows } => {
                if host_knows {
                    (doors - 1) as f64 / doors as f64
                } else {
                    // Given that a random host revealed only goats: 1/2 for any n.
                    0.5
                }
            }
            Paradox::Birthday { people, days } => birthday_probability(people, days),
            Paradox::Coupon { items } => items as f64 * harmonic(items as u64),
            Paradox::Secretary { candidates, skip } => secretary_success(candidates, skip),
            Paradox::Buffon { length, spacing } => 2.0 * length / (std::f64::consts::PI * spacing),
        }
    }

    /// The exact answer as a fraction or formula, when that reads better.
    pub fn exact(&self) -> Option<String> {
        match *self {
            Paradox::MontyHall { doors, host_knows } => Some(if host_knows {
                format!("{}/{}", doors - 1, doors)
            } else {
                "1/2".to_string()
            }),
            Paradox::Coupon { items } => Some(format!("{items} x H_{items}")),
            Paradox::Buffon { .. } => Some("2l/(pi t)".to_string()),
            _ => None,
        }
    }

    /// Parameters, for display and JSON.
    pub fn params(&self) -> Vec<(&'static str, String)> {
        match *self {
            Paradox::MontyHall { doors, host_knows } => vec![
                ("doors", doors.to_string()),
                (
                    "host",
                    if host_knows { "knows" } else { "random" }.to_string(),
                ),
            ],
            Paradox::Birthday { people, days } => {
                vec![("people", people.to_string()), ("days", days.to_string())]
            }
            Paradox::Coupon { items } => vec![("items", items.to_string())],
            Paradox::Secretary { candidates, skip } => vec![
                ("candidates", candidates.to_string()),
                ("skip", skip.to_string()),
            ],
            Paradox::Buffon { length, spacing } => vec![
                ("length", length.to_string()),
                ("spacing", spacing.to_string()),
            ],
        }
    }

    /// One trial, or None when the trial does not count (a random Monty Hall
    /// host who revealed the car).
    pub fn trial(&self, rng: &mut Rng, scratch: &mut Vec<u32>) -> Option<f64> {
        match *self {
            Paradox::MontyHall { doors, host_knows } => {
                let n = doors as u64;
                let car = rng.below(n);
                let pick = rng.below(n);
                // The host opens every door except your pick and one other:
                // `kept` is the one he leaves closed for you to switch to.
                let kept = if host_knows {
                    if car != pick {
                        car
                    } else {
                        // Any other door; all of them hide goats.
                        let k = rng.below(n - 1);
                        if k >= pick {
                            k + 1
                        } else {
                            k
                        }
                    }
                } else {
                    let k = rng.below(n - 1);
                    let kept = if k >= pick { k + 1 } else { k };
                    // He opened every door but `pick` and `kept`; if the car
                    // was among them, this game is not the situation asked about.
                    if car != pick && car != kept {
                        return None;
                    }
                    kept
                };
                Some(if kept == car { 1.0 } else { 0.0 })
            }
            Paradox::Birthday { people, days } => {
                scratch.clear();
                scratch.resize(days as usize, 0);
                for _ in 0..people {
                    let d = rng.below(days as u64) as usize;
                    if scratch[d] == 1 {
                        return Some(1.0);
                    }
                    scratch[d] = 1;
                }
                Some(0.0)
            }
            Paradox::Coupon { items } => {
                scratch.clear();
                scratch.resize(items as usize, 0);
                let (mut missing, mut draws) = (items, 0u64);
                while missing > 0 {
                    draws += 1;
                    let i = rng.below(items as u64) as usize;
                    if scratch[i] == 0 {
                        scratch[i] = 1;
                        missing -= 1;
                    }
                }
                Some(draws as f64)
            }
            Paradox::Secretary { candidates, skip } => {
                // Scores 0..n in random order; n-1 is the best.
                scratch.clear();
                scratch.extend(0..candidates);
                rng.shuffle(scratch);
                let k = skip as usize;
                let bar = scratch[..k].iter().max().copied();
                let hired = scratch[k..].iter().find(|&&s| bar.is_none_or(|b| s > b));
                Some(if hired == Some(&(candidates - 1)) {
                    1.0
                } else {
                    0.0
                })
            }
            Paradox::Buffon { length, spacing } => {
                // Distance from the needle's center to the nearest line.
                let y = rng.f64() * spacing / 2.0;
                // A uniform angle without using pi: a uniform point in the
                // quarter disk has a uniform angle.
                let sin = loop {
                    let (a, b) = (rng.f64(), rng.f64());
                    let r2 = a * a + b * b;
                    if r2 > 0.0 && r2 <= 1.0 {
                        break b / r2.sqrt();
                    }
                };
                Some(if y <= length / 2.0 * sin { 1.0 } else { 0.0 })
            }
        }
    }

    pub fn texts(&self, lang: Lang) -> Texts {
        let p = self.theory();
        match (*self, lang) {
            (Paradox::MontyHall { doors, host_knows: true }, Lang::En) => Texts {
                title: format!("Monty Hall problem ({doors} doors, the host knows)"),
                question: format!(
                    "There are {doors} doors: a car behind one, goats behind the rest. You pick a door. \
                     The host, who knows where the car is, opens {}, and offers you the one door left. \
                     Should you switch?",
                    others(doors - 2)
                ),
                intuition: "Two doors are left, so it is 50/50 and switching makes no difference.".into(),
                truth: format!("Switching wins {}/{doors} of the time ({}).", doors - 1, pct(p)),
                why: format!(
                    "Your first pick is right only 1/{doors} of the time. The host never opens the car, \
                     so in every other case the car is behind the door he leaves closed."
                ),
                unit: "games".into(),
                quantity: "switching wins".into(),
            },
            (Paradox::MontyHall { doors, host_knows: true }, Lang::Ja) => Texts {
                title: format!("モンティ・ホール問題(ドア{doors}枚、司会者は正解を知っている)"),
                question: format!(
                    "ドアが{doors}枚あり、1枚の後ろに車、残りはヤギです。あなたが1枚選ぶと、正解を知っている\
                     司会者が残りから{}枚のヤギのドアを開け、閉じたまま残った1枚に変えてもよいと言います。変えるべき?",
                    doors - 2
                ),
                intuition: "残りは2枚なので五分五分。変えても変えなくても同じ。".into(),
                truth: format!("変えると{}/{doors}の確率({})で当たります。", doors - 1, pct(p)),
                why: format!(
                    "最初の選択が当たっているのは1/{doors}だけ。司会者は車のドアを開けないので、それ以外の\
                     場合はすべて、司会者が閉じたまま残したドアに車があります。"
                ),
                unit: "回".into(),
                quantity: "変えて当たる確率".into(),
            },
            (Paradox::MontyHall { doors, host_knows: false }, Lang::En) => Texts {
                title: format!("Monty Hall, careless host ({doors} doors, the host does not know)"),
                question: format!(
                    "The same game, but the host does not know where the car is: he opens {} at random, \
                     and this time {} a goat. Should you switch?",
                    if doors == 3 { "another door".to_string() } else { format!("{} other doors", doors - 2) },
                    if doors == 3 { "it happens to hide" } else { "every one happens to hide" }
                ),
                intuition: format!(
                    "It looks just like the classic puzzle, so switching should win {}/{doors}.",
                    doors - 1
                ),
                truth: "Switching wins only 1/2 of the games where the host happened to reveal no car.".into(),
                why: "A random host reveals the car in many games. Keeping only the games where he did not \
                      throws away most of the cases where switching would have won. Games where the car \
                      was revealed are left out of the count."
                    .into(),
                unit: "games".into(),
                quantity: "switching wins".into(),
            },
            (Paradox::MontyHall { doors, host_knows: false }, Lang::Ja) => Texts {
                title: format!("うっかり司会者のモンティ・ホール(ドア{doors}枚、司会者は正解を知らない)"),
                question: format!(
                    "同じゲームですが、司会者は車の場所を知らず、残りから{}枚をでたらめに開けます。今回は\
                     たまたま全部ヤギでした。変えるべき?",
                    doors - 2
                ),
                intuition: format!("元の問題と同じに見えるので、変えれば{}/{doors}で当たるはず。", doors - 1),
                truth: "司会者がたまたま車を開けなかったゲームに限ると、変えて当たるのは1/2です。".into(),
                why: "でたらめに開ける司会者は、多くのゲームで車を開けてしまいます。車が出なかったゲームだけを\
                      残すと、変えれば当たっていたはずのケースの多くが除かれます。車が開いたゲームは数えません。"
                    .into(),
                unit: "回".into(),
                quantity: "変えて当たる確率".into(),
            },
            (Paradox::Birthday { people, days }, Lang::En) => Texts {
                title: format!("Birthday paradox ({people} people, {days} days)"),
                question: format!(
                    "In a group of {people} people, how likely is it that at least two share a birthday? \
                     (Each of {days} days equally likely.)"
                ),
                intuition: format!(
                    "{people} people out of {days} days is only {}, so a shared birthday should be rare.",
                    pct(people as f64 / days as f64)
                ),
                truth: format!("At least two share a birthday with probability {}.", pct(p)),
                why: format!(
                    "What matters is pairs, not people: {people} people make {} pairs, and any pair can match.",
                    people as u64 * (people as u64).saturating_sub(1) / 2
                ),
                unit: "groups".into(),
                quantity: "a shared birthday".into(),
            },
            (Paradox::Birthday { people, days }, Lang::Ja) => Texts {
                title: format!("誕生日のパラドックス({people}人、{days}日)"),
                question: format!(
                    "{people}人のグループで、誕生日が同じ人が少なくとも1組いる確率は?(誕生日は{days}日のどれも同じ確率)"
                ),
                intuition: format!(
                    "{days}日に対して{people}人は{}にすぎないので、同じ誕生日はめったにないはず。",
                    pct(people as f64 / days as f64)
                ),
                truth: format!("少なくとも1組が同じ誕生日になる確率は{}です。", pct(p)),
                why: format!(
                    "効くのは人数ではなく組の数です。{people}人なら{}組あり、どの組でも一致しえます。",
                    people as u64 * (people as u64).saturating_sub(1) / 2
                ),
                unit: "グループ".into(),
                quantity: "同じ誕生日がいる確率".into(),
            },
            (Paradox::Coupon { items }, Lang::En) => Texts {
                title: format!("Coupon collector, or completing a gacha ({items} items)"),
                question: format!(
                    "A gacha has {items} items, each equally likely on every draw. How many draws does it \
                     take, on average, to collect them all?"
                ),
                intuition: format!("About {items} draws, maybe a few more."),
                truth: format!("{:.1} draws on average ({items} x H_{items}).", p),
                why: format!(
                    "The last items are the hard part: with one item missing, each draw finds it with \
                     probability 1/{items}, so that last item alone takes {items} draws on average."
                ),
                unit: "collectors".into(),
                quantity: "draws to complete".into(),
            },
            (Paradox::Coupon { items }, Lang::Ja) => Texts {
                title: format!("コンプリートガチャ(クーポン収集問題、全{items}種)"),
                question: format!(
                    "全{items}種のガチャで、毎回どれも同じ確率で出ます。全種そろえるまでに平均何回引く?"
                ),
                intuition: format!("{items}回くらい、多くてもそれより少し多い程度。"),
                truth: format!("平均{:.1}回です({items} x H_{items})。", p),
                why: format!(
                    "大変なのは最後の数種です。残り1種のときは1回あたり1/{items}でしか出ないので、最後の\
                     1種だけで平均{items}回かかります。"
                ),
                unit: "人".into(),
                quantity: "そろうまでの回数".into(),
            },
            (Paradox::Secretary { candidates, skip }, Lang::En) => Texts {
                title: format!("Secretary problem ({candidates} candidates)"),
                question: format!(
                    "{candidates} candidates are interviewed in random order. After each one you must hire \
                     or reject on the spot, and only the very best will do. What is your chance?"
                ),
                intuition: format!(
                    "Without seeing everyone you are guessing, so about 1/{candidates} ({}).",
                    pct(1.0 / candidates as f64)
                ),
                truth: format!(
                    "Reject the first {skip} ({}), then hire the first who beats them all: the very best \
                     is hired {} of the time.",
                    pct(skip as f64 / candidates as f64),
                    pct(p)
                ),
                why: "The first part sets the bar. As the number of candidates grows, the best share to \
                      reject and the chance of success both approach 1/e, about 36.8%."
                    .into(),
                unit: "hirings".into(),
                quantity: "the best is hired".into(),
            },
            (Paradox::Secretary { candidates, skip }, Lang::Ja) => Texts {
                title: format!("秘書問題({candidates}人の候補者)"),
                question: format!(
                    "{candidates}人の候補者をランダムな順に面接し、1人ごとにその場で採用か不採用を決めます。\
                     最も優秀な1人を採用できる確率は?"
                ),
                intuition: format!(
                    "全員を見る前に決めるのだから当てずっぽうと同じで、1/{candidates}({})程度。",
                    pct(1.0 / candidates as f64)
                ),
                truth: format!(
                    "最初の{skip}人({})は見送り、それまでの全員より良い最初の人を採用すると、{}の確率で\
                     最も優秀な人を採用できます。",
                    pct(skip as f64 / candidates as f64),
                    pct(p)
                ),
                why: "最初の何人かで基準を作るのがこつです。候補者が増えるほど、見送る割合と成功率はどちらも\
                      1/e(約36.8%)に近づきます。"
                    .into(),
                unit: "回".into(),
                quantity: "最も優秀な人を採用する確率".into(),
            },
            (Paradox::Buffon { length, spacing }, Lang::En) => Texts {
                title: format!("Buffon's needle (length {length}, lines {spacing} apart)"),
                question: format!(
                    "Drop a needle of length {length} on a floor ruled with lines {spacing} apart. How often \
                     does it cross a line?"
                ),
                intuition: "Hard to say, and surely nothing to do with circles.".into(),
                truth: format!(
                    "It crosses with probability 2l/(pi t) = {}, so counting crossings estimates pi.",
                    pct(p)
                ),
                why: "Whether it crosses depends on the needle's angle, and averaging over every angle \
                      brings in pi. The simulation itself never uses pi: each angle comes from a random \
                      point in a quarter disk."
                    .into(),
                unit: "needles".into(),
                quantity: "a needle crosses a line".into(),
            },
            (Paradox::Buffon { length, spacing }, Lang::Ja) => Texts {
                title: format!("ビュフォンの針(長さ{length}、線の間隔{spacing})"),
                question: format!(
                    "間隔{spacing}で平行線を引いた床に、長さ{length}の針を落とします。針が線と交わる確率は?"
                ),
                intuition: "見当がつかないし、円とは関係なさそう。".into(),
                truth: format!(
                    "交わる確率は2l/(πt) = {}。交わった回数を数えれば円周率を推定できます。",
                    pct(p)
                ),
                why: "交わるかどうかは針の角度で決まり、あらゆる角度で平均するとπが現れます。このシミュレーション\
                      自体はπを使いません。角度は四分円の中のランダムな点から作っています。"
                    .into(),
                unit: "本".into(),
                quantity: "針が線と交わる確率".into(),
            },
        }
    }
}
