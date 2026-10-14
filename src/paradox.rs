//! The paradoxes: what each one asks, its exact answer, and one random trial.

use std::fmt::Write as _;

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
    /// What one trial is called, singular, for --explain ("Game", "Group").
    pub one: String,
    /// What the estimate is ("switching wins").
    pub quantity: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paradox {
    MontyHall {
        doors: u32,
        host_knows: bool,
    },
    Birthday {
        people: u32,
        days: u32,
    },
    Coupon {
        items: u32,
    },
    Secretary {
        candidates: u32,
        skip: u32,
    },
    /// A needle of `length` on lines `spacing` apart; only the ratio matters.
    Buffon {
        length: f64,
        spacing: f64,
    },
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

/// The cutoff that maximizes `secretary_success`, in one pass: the tail sum
/// is built from the top down. On a tie (within rounding) the smaller cutoff
/// wins, so for two candidates it is 0.
pub fn secretary_best_skip(n: u32) -> u32 {
    const TIE: f64 = 1e-15;
    let mut best: Option<(u32, f64)> = None;
    let mut tail = 0.0;
    for k in (1..n).rev() {
        tail += 1.0 / k as f64;
        let p = k as f64 / n as f64 * tail;
        // Going down, a later k is smaller, so it takes a tie.
        if best.is_none_or(|(_, b)| p >= b - TIE) {
            best = Some((k, p));
        }
    }
    match best {
        Some((k, p)) if p > 1.0 / n as f64 + TIE => k,
        _ => 0,
    }
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

/// A percentage with enough decimals to show a small value.
pub fn pct(p: f64) -> String {
    let v = p * 100.0;
    if v == 0.0 || v >= 0.1 {
        format!("{v:.1}%")
    } else {
        let decimals = ((-v.log10()).ceil() as usize + 1).clamp(2, 12);
        format!("{v:.decimals$}%")
    }
}

/// A parameter for text: plain for ordinary sizes, scientific otherwise.
pub fn num(x: f64) -> String {
    let a = x.abs();
    if a == 0.0 || (1e-4..1e6).contains(&a) {
        format!("{x}")
    } else {
        format!("{x:e}")
    }
}

/// Pairs among `people`, in words.
fn pairs(people: u32, lang: Lang) -> String {
    let p = people as u64 * (people as u64).saturating_sub(1) / 2;
    match (lang, people) {
        (Lang::En, 1) => "a single person makes no pairs".to_string(),
        (Lang::En, _) => format!("{people} people make {p} pairs, and any pair can match"),
        (Lang::Ja, 1) => "1人では組ができません".to_string(),
        (Lang::Ja, _) => format!("{people}人なら{p}組あり、どの組でも一致しえます"),
    }
}

/// "another door, a goat" or "N other doors, all goats".
fn others(k: u32) -> String {
    if k == 1 {
        "another door, a goat".to_string()
    } else {
        format!("{k} other doors, all goats")
    }
}

/// The story of one trial, for --explain: a buffer and its language.
pub type Story<'a> = Option<(&'a mut String, Lang)>;

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

    /// Whether some trials may not count (a random Monty Hall host).
    pub fn may_reject(&self) -> bool {
        matches!(
            self,
            Paradox::MontyHall {
                host_knows: false,
                ..
            }
        )
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
            Paradox::Buffon { length, spacing } => 2.0 * (length / spacing) / std::f64::consts::PI,
        }
    }

    /// The exact variance of one trial: p(1 - p) for a probability, and for
    /// the coupon collector n^2 * sum 1/k^2 - n * H_n.
    pub fn theory_variance(&self) -> f64 {
        match *self {
            Paradox::Coupon { items } => {
                let n = items as f64;
                let squares: f64 = (1..=items as u64).rev().map(|k| 1.0 / (k * k) as f64).sum();
                n * n * squares - n * harmonic(items as u64)
            }
            _ => {
                let p = self.theory();
                p * (1.0 - p)
            }
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
            Paradox::Buffon { length, spacing } => {
                vec![
                    ("length", format!("{length:e}")),
                    ("spacing", format!("{spacing:e}")),
                ]
            }
        }
    }

    /// One trial, or None when the trial does not count (a random Monty Hall
    /// host who revealed the car).
    pub fn trial(&self, rng: &mut Rng, scratch: &mut Vec<u32>) -> Option<f64> {
        self.run(rng, scratch, None)
    }

    /// One trial with its story told: the same code and the same random
    /// numbers as `trial`, so the story is exactly what was simulated.
    pub fn explain(
        &self,
        rng: &mut Rng,
        scratch: &mut Vec<u32>,
        lang: Lang,
    ) -> (Option<f64>, String) {
        let mut s = String::new();
        let result = self.run(rng, scratch, Some((&mut s, lang)));
        (result, s)
    }

    fn run(&self, rng: &mut Rng, scratch: &mut Vec<u32>, mut story: Story) -> Option<f64> {
        match *self {
            Paradox::MontyHall { doors, host_knows } => {
                let n = doors as u64;
                let car = rng.below(n);
                let pick = rng.below(n);
                // The host opens every door except your pick and one other:
                // `kept` is the one he leaves closed for you to switch to.
                // A host who knows keeps the car closed when you missed it;
                // otherwise he keeps a door chosen at random.
                let other_than_pick = |k: u64| if k >= pick { k + 1 } else { k };
                let kept = if host_knows && car != pick {
                    car
                } else {
                    other_than_pick(rng.below(n - 1))
                };
                let revealed = car != pick && car != kept;
                let wins = kept == car;
                if let Some((s, lang)) = story.as_mut() {
                    let (c, p, k, o) = (car + 1, pick + 1, kept + 1, n - 2);
                    let _ = match (lang, revealed) {
                        (Lang::En, true) => write!(
                            s,
                            "car behind door {c}, you pick door {p}; the host, opening doors at random, \
                             leaves door {k} closed and reveals the car: this game does not count"
                        ),
                        (Lang::En, false) => write!(
                            s,
                            "car behind door {c}, you pick door {p}; the host leaves door {k} closed and \
                             opens {}; switching to door {k} {}",
                            if o == 1 { "the other door".to_string() } else { format!("the other {o}") },
                            if wins { "wins" } else { "loses" }
                        ),
                        (Lang::Ja, true) => write!(
                            s,
                            "車はドア{c}、あなたはドア{p}を選択。でたらめに開ける司会者がドア{k}を残して開けたら\
                             車が出てしまった: このゲームは数えない"
                        ),
                        (Lang::Ja, false) => write!(
                            s,
                            "車はドア{c}、あなたはドア{p}を選択。司会者はドア{k}を残して他の{o}枚を開ける。\
                             ドア{k}に変えると{}",
                            if wins { "当たり" } else { "はずれ" }
                        ),
                    };
                }
                if revealed {
                    // Only possible with a random host: not the situation asked about.
                    return None;
                }
                Some(if wins { 1.0 } else { 0.0 })
            }
            Paradox::Birthday { people, days } => {
                // scratch[d] holds the generation that last saw day d, and
                // scratch[days] the current generation, so the days are not
                // cleared on every trial.
                let d_len = days as usize;
                if scratch.len() != d_len + 1 {
                    scratch.clear();
                    scratch.resize(d_len + 1, 0);
                }
                let mut generation = scratch[d_len].wrapping_add(1);
                if generation == 0 {
                    scratch.iter_mut().for_each(|x| *x = 0);
                    generation = 1;
                }
                scratch[d_len] = generation;
                for person in 1..=people {
                    let d = rng.below(days as u64) as usize;
                    if scratch[d] == generation {
                        if let Some((s, lang)) = story.as_mut() {
                            let _ = match lang {
                                Lang::En => write!(
                                    s,
                                    "person {person} has day {} like someone before: a shared birthday",
                                    d + 1
                                ),
                                Lang::Ja => write!(
                                    s,
                                    "{person}人目の誕生日{}日目は先に来た人と同じ: 一致あり",
                                    d + 1
                                ),
                            };
                        }
                        return Some(1.0);
                    }
                    scratch[d] = generation;
                }
                if let Some((s, lang)) = story.as_mut() {
                    let _ = match lang {
                        Lang::En => write!(s, "all {people} birthdays differ: no shared birthday"),
                        Lang::Ja => write!(s, "{people}人全員の誕生日が異なる: 一致なし"),
                    };
                }
                Some(0.0)
            }
            Paradox::Coupon { items } => {
                scratch.clear();
                scratch.resize(items as usize, 0);
                let (mut missing, mut draws) = (items, 0u64);
                let mut seen: Vec<u64> = Vec::new();
                let mut last = 0;
                while missing > 0 {
                    draws += 1;
                    let i = rng.below(items as u64) as usize;
                    if story.is_some() && seen.len() < 40 {
                        seen.push(i as u64 + 1);
                    }
                    if scratch[i] == 0 {
                        scratch[i] = 1;
                        missing -= 1;
                        last = i + 1;
                    }
                }
                if let Some((s, lang)) = story.as_mut() {
                    let list = seen
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(" ");
                    let more = if draws as usize > seen.len() {
                        " ..."
                    } else {
                        ""
                    };
                    let _ = match lang {
                        Lang::En => write!(
                            s,
                            "drew {list}{more}: complete after {draws} draws, the last new item was {last}"
                        ),
                        Lang::Ja => write!(
                            s,
                            "{list}{more} と引いて{draws}回でコンプリート。最後にそろったのは{last}番"
                        ),
                    };
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
                let hired = scratch[k..]
                    .iter()
                    .position(|&s| bar.is_none_or(|b| s > b))
                    .map(|i| (k + i, scratch[k + i]));
                let best = hired.is_some_and(|(_, s)| s == candidates - 1);
                if let Some((s, lang)) = story.as_mut() {
                    let bar_text = match (*lang, bar) {
                        (Lang::En, Some(b)) => {
                            format!("the first {k} set the bar at score {}", b + 1)
                        }
                        (Lang::En, None) => "no one is skipped".to_string(),
                        (Lang::Ja, Some(b)) => format!("最初の{k}人で基準はスコア{}", b + 1),
                        (Lang::Ja, None) => "見送りなし".to_string(),
                    };
                    let _ = match (*lang, hired) {
                        (Lang::En, Some((i, sc))) => write!(
                            s,
                            "{bar_text}; candidate {} beats it with score {} of {candidates}: {}",
                            i + 1,
                            sc + 1,
                            if best { "the best, hired" } else { "hired, but not the best" }
                        ),
                        (Lang::En, None) => write!(
                            s,
                            "{bar_text}; no one after beats it, because the best was among the first {k}"
                        ),
                        (Lang::Ja, Some((i, sc))) => write!(
                            s,
                            "{bar_text}。{}人目がスコア{}(最高{candidates})で上回り採用: {}",
                            i + 1,
                            sc + 1,
                            if best { "最も優秀な人" } else { "最も優秀な人ではない" }
                        ),
                        (Lang::Ja, None) => write!(
                            s,
                            "{bar_text}。最も優秀な人が最初の{k}人にいたので、誰も上回れない"
                        ),
                    };
                }
                Some(if best { 1.0 } else { 0.0 })
            }
            Paradox::Buffon { length, spacing } => {
                // In units of the line spacing, so only the ratio matters and
                // no product or quotient of the raw inputs can overflow.
                let r = length / spacing;
                // Distance from the needle's center to the nearest line.
                let y = rng.f64() / 2.0;
                // A uniform angle without using pi: a uniform point in the
                // quarter disk has a uniform angle.
                let sin = loop {
                    let (a, b) = (rng.f64(), rng.f64());
                    let r2 = a * a + b * b;
                    if r2 > 0.0 && r2 <= 1.0 {
                        break b / r2.sqrt();
                    }
                };
                let reach = r / 2.0 * sin;
                let crosses = y <= reach;
                if let Some((s, lang)) = story.as_mut() {
                    let _ =
                        match lang {
                            Lang::En => write!(
                            s,
                            "center {y:.3} spacings from a line, the needle reaches {reach:.3}: {}",
                            if crosses { "crosses" } else { "misses" }
                        ),
                            Lang::Ja => write!(
                            s,
                            "中心から線までの距離{y:.3}(間隔を1とする)、針が届く距離{reach:.3}: {}",
                            if crosses { "交わる" } else { "交わらない" }
                        ),
                        };
                }
                Some(if crosses { 1.0 } else { 0.0 })
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
                one: "Game".into(),
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
                one: "ゲーム目".into(),
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
                one: "Game".into(),
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
                one: "ゲーム目".into(),
                quantity: "変えて当たる確率".into(),
            },
            (Paradox::Birthday { people, days }, Lang::En) => Texts {
                title: format!("Birthday paradox ({people} people, {days} days)"),
                question: format!(
                    "In a group of {people} people, how likely is it that at least two share a birthday? \
                     (Each of {days} days equally likely.)"
                ),
                intuition: if people > days {
                    "With more people than days, a shared birthday is certain (the pigeonhole principle).".into()
                } else {
                    format!(
                        "{people} people for {days} days covers only {} of the days, so a shared birthday \
                         should be rare.",
                        pct(people as f64 / days as f64)
                    )
                },
                truth: format!("At least two share a birthday with probability {}.", pct(p)),
                why: format!("What matters is pairs, not people: {}.", pairs(people, Lang::En)),
                unit: "groups".into(),
                one: "Group".into(),
                quantity: "a shared birthday".into(),
            },
            (Paradox::Birthday { people, days }, Lang::Ja) => Texts {
                title: format!("誕生日のパラドックス({people}人、{days}日)"),
                question: format!(
                    "{people}人のグループで、誕生日が同じ人が少なくとも1組いる確率は?(誕生日は{days}日のどれも同じ確率)"
                ),
                intuition: if people > days {
                    "日数より人数が多いので、必ず誰かの誕生日が重なる(鳩の巣原理)。".into()
                } else {
                    format!(
                        "{days}日に対して{people}人は日数の{}にすぎないので、同じ誕生日はめったにないはず。",
                        pct(people as f64 / days as f64)
                    )
                },
                truth: format!("少なくとも1組が同じ誕生日になる確率は{}です。", pct(p)),
                why: format!("効くのは人数ではなく組の数です。{}。", pairs(people, Lang::Ja)),
                unit: "グループ".into(),
                one: "グループ目".into(),
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
                one: "Collector".into(),
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
                one: "人目".into(),
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
                one: "Hiring".into(),
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
                one: "回目".into(),
                quantity: "最も優秀な人を採用する確率".into(),
            },
            (Paradox::Buffon { length, spacing }, Lang::En) => Texts {
                title: format!("Buffon's needle (length {}, lines {} apart)", num(length), num(spacing)),
                question: format!(
                    "Drop a needle of length {} on a floor ruled with lines {} apart. How often does it \
                     cross a line?",
                    num(length),
                    num(spacing)
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
                one: "Needle".into(),
                quantity: "a needle crosses a line".into(),
            },
            (Paradox::Buffon { length, spacing }, Lang::Ja) => Texts {
                title: format!("ビュフォンの針(長さ{}、線の間隔{})", num(length), num(spacing)),
                question: format!(
                    "間隔{}で平行線を引いた床に、長さ{}の針を落とします。針が線と交わる確率は?",
                    num(spacing),
                    num(length)
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
                one: "本目".into(),
                quantity: "針が線と交わる確率".into(),
            },
        }
    }
}
