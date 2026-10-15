//! The words around each paradox, in English and Japanese.

use crate::exact;
use crate::paradox::{coins_text, Chord, Lang, Paradox};

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

// One argument per field of `Texts`, in order, to keep each paradox's texts readable.
#[allow(clippy::too_many_arguments)]
fn t(
    title: String,
    question: String,
    intuition: String,
    truth: String,
    why: String,
    unit: &str,
    one: &str,
    quantity: &str,
) -> Texts {
    Texts {
        title,
        question,
        intuition,
        truth,
        why,
        unit: unit.into(),
        one: one.into(),
        quantity: quantity.into(),
    }
}

pub fn texts(paradox: &Paradox, lang: Lang) -> Texts {
    let p = paradox.theory();
    match (paradox, lang) {
        (Paradox::MontyHall { doors, host_knows: true }, Lang::En) => t(
            format!("Monty Hall problem ({doors} doors, the host knows)"),
            format!(
                "There are {doors} doors: a car behind one, goats behind the rest. You pick a door. \
                 The host, who knows where the car is, opens {}, and offers you the one door left. \
                 Should you switch?",
                others(doors - 2)
            ),
            "Two doors are left, so it is 50/50 and switching makes no difference.".into(),
            format!("Switching wins {}/{doors} of the time ({}).", doors - 1, pct(p)),
            format!(
                "Your first pick is right only 1/{doors} of the time. The host never opens the car, \
                 so in every other case the car is behind the door he leaves closed."
            ),
            "games",
            "Game",
            "switching wins",
        ),
        (Paradox::MontyHall { doors, host_knows: true }, Lang::Ja) => t(
            format!("モンティ・ホール問題(ドア{doors}枚、司会者は正解を知っている)"),
            format!(
                "ドアが{doors}枚あり、1枚の後ろに車、残りはヤギです。あなたが1枚選ぶと、正解を知っている\
                 司会者が残りから{}枚のヤギのドアを開け、閉じたまま残った1枚に変えてもよいと言います。変えるべき?",
                doors - 2
            ),
            "残りは2枚なので五分五分。変えても変えなくても同じ。".into(),
            format!("変えると{}/{doors}の確率({})で当たります。", doors - 1, pct(p)),
            format!(
                "最初の選択が当たっているのは1/{doors}だけ。司会者は車のドアを開けないので、それ以外の\
                 場合はすべて、司会者が閉じたまま残したドアに車があります。"
            ),
            "回",
            "ゲーム目",
            "変えて当たる確率",
        ),
        (Paradox::MontyHall { doors, host_knows: false }, Lang::En) => t(
            format!("Monty Hall, careless host ({doors} doors, the host does not know)"),
            format!(
                "The same game, but the host does not know where the car is: he opens {} at random, \
                 and this time {} a goat. Should you switch?",
                if *doors == 3 { "another door".to_string() } else { format!("{} other doors", doors - 2) },
                if *doors == 3 { "it happens to hide" } else { "every one happens to hide" }
            ),
            format!("It looks just like the classic puzzle, so switching should win {}/{doors}.", doors - 1),
            "Switching wins only 1/2 of the games where the host happened to reveal no car.".into(),
            "A random host reveals the car in many games. Keeping only the games where he did not \
             throws away most of the cases where switching would have won. Games where the car \
             was revealed are left out of the count."
                .into(),
            "games",
            "Game",
            "switching wins",
        ),
        (Paradox::MontyHall { doors, host_knows: false }, Lang::Ja) => t(
            format!("うっかり司会者のモンティ・ホール(ドア{doors}枚、司会者は正解を知らない)"),
            format!(
                "同じゲームですが、司会者は車の場所を知らず、残りから{}枚をでたらめに開けます。今回は\
                 たまたま全部ヤギでした。変えるべき?",
                doors - 2
            ),
            format!("元の問題と同じに見えるので、変えれば{}/{doors}で当たるはず。", doors - 1),
            "司会者がたまたま車を開けなかったゲームに限ると、変えて当たるのは1/2です。".into(),
            "でたらめに開ける司会者は、多くのゲームで車を開けてしまいます。車が出なかったゲームだけを\
             残すと、変えれば当たっていたはずのケースの多くが除かれます。車が開いたゲームは数えません。"
                .into(),
            "回",
            "ゲーム目",
            "変えて当たる確率",
        ),
        (Paradox::Birthday { people, days, weights }, Lang::En) => t(
            format!(
                "Birthday paradox ({people} people, {days} days{})",
                if weights.is_some() { ", with your weights" } else { "" }
            ),
            format!(
                "In a group of {people} people, how likely is it that at least two share a birthday? \
                 ({})",
                if weights.is_some() {
                    format!("Each of {days} days with the chance you gave it.")
                } else {
                    format!("Each of {days} days equally likely.")
                }
            ),
            if people > days {
                "With more people than days, a shared birthday is certain (the pigeonhole principle).".into()
            } else {
                format!(
                    "{people} people for {days} days covers only {} of the days, so a shared birthday \
                     should be rare.",
                    pct(*people as f64 / *days as f64)
                )
            },
            format!("At least two share a birthday with probability {}.", pct(p)),
            format!(
                "What matters is pairs, not people: {}.{}",
                pairs(*people, Lang::En),
                if weights.is_some() {
                    " Uneven chances make a match more likely, never less, than equal ones."
                } else {
                    ""
                }
            ),
            "groups",
            "Group",
            "a shared birthday",
        ),
        (Paradox::Birthday { people, days, weights }, Lang::Ja) => t(
            format!(
                "誕生日のパラドックス({people}人、{days}日{})",
                if weights.is_some() { "、指定した重み" } else { "" }
            ),
            format!(
                "{people}人のグループで、誕生日が同じ人が少なくとも1組いる確率は?({})",
                if weights.is_some() {
                    format!("誕生日は{days}日それぞれ指定した確率")
                } else {
                    format!("誕生日は{days}日のどれも同じ確率")
                }
            ),
            if people > days {
                "日数より人数が多いので、必ず誰かの誕生日が重なる(鳩の巣原理)。".into()
            } else {
                format!(
                    "{days}日に対して{people}人は日数の{}にすぎないので、同じ誕生日はめったにないはず。",
                    pct(*people as f64 / *days as f64)
                )
            },
            format!("少なくとも1組が同じ誕生日になる確率は{}です。", pct(p)),
            format!(
                "効くのは人数ではなく組の数です。{}。{}",
                pairs(*people, Lang::Ja),
                if weights.is_some() { "確率に偏りがあると、均等な場合より一致しやすくなります(しにくくはなりません)。" } else { "" }
            ),
            "グループ",
            "グループ目",
            "同じ誕生日がいる確率",
        ),
        (Paradox::Coupon { items, weights, budget: None }, Lang::En) => {
            let rare = weights.as_ref().map(|w| w.probs.iter().copied().fold(f64::INFINITY, f64::min));
            t(
                format!(
                    "Coupon collector, or completing a gacha ({items} items{})",
                    if weights.is_some() { ", unequal rates" } else { "" }
                ),
                match rare {
                    Some(r) => format!(
                        "A gacha has {items} items with the rates you gave; the rarest comes {} of the \
                         time. How many draws does it take, on average, to collect them all?",
                        pct(r)
                    ),
                    None => format!(
                        "A gacha has {items} items, each equally likely on every draw. How many draws \
                         does it take, on average, to collect them all?"
                    ),
                },
                format!("About {items} draws, maybe a few more."),
                match rare {
                    Some(_) => format!("{p:.1} draws on average (computed exactly from the rates)."),
                    None => format!("{p:.1} draws on average ({items} x H_{items})."),
                },
                match rare {
                    Some(r) => format!(
                        "The rarest item dominates: it alone takes {:.1} draws on average to show up.",
                        1.0 / r
                    ),
                    None => format!(
                        "The last items are the hard part: with one item missing, each draw finds it \
                         with probability 1/{items}, so that last item alone takes {items} draws on average."
                    ),
                },
                "collectors",
                "Collector",
                "draws to complete",
            )
        }
        (Paradox::Coupon { items, weights, budget: None }, Lang::Ja) => {
            let rare = weights.as_ref().map(|w| w.probs.iter().copied().fold(f64::INFINITY, f64::min));
            t(
                format!(
                    "コンプリートガチャ(クーポン収集問題、全{items}種{})",
                    if weights.is_some() { "、排出率に偏り" } else { "" }
                ),
                match rare {
                    Some(r) => format!(
                        "全{items}種のガチャで、排出率は指定どおり(いちばん出にくい種は{})。全種そろえるまでに\
                         平均何回引く?",
                        pct(r)
                    ),
                    None => format!("全{items}種のガチャで、毎回どれも同じ確率で出ます。全種そろえるまでに平均何回引く?"),
                },
                format!("{items}回くらい、多くてもそれより少し多い程度。"),
                match rare {
                    Some(_) => format!("平均{p:.1}回です(排出率から厳密に計算)。"),
                    None => format!("平均{p:.1}回です({items} x H_{items})。"),
                },
                match rare {
                    Some(r) => format!("いちばん出にくい種が効きます。その1種だけで、出るまで平均{:.1}回かかります。", 1.0 / r),
                    None => format!(
                        "大変なのは最後の数種です。残り1種のときは1回あたり1/{items}でしか出ないので、最後の\
                         1種だけで平均{items}回かかります。"
                    ),
                },
                "人",
                "人目",
                "そろうまでの回数",
            )
        }
        (Paradox::Coupon { items, weights, budget: Some(b) }, Lang::En) => {
            let mean = match weights {
                Some(w) => exact::coupon_mean_weighted(&w.probs),
                None => exact::coupon_mean_equal(*items),
            };
            t(
                format!("Completing a gacha within {b} draws ({items} items)"),
                format!(
                    "A gacha has {items} items{}. How likely are you to collect them all within {b} draws?",
                    if weights.is_some() { " with the rates you gave" } else { ", each equally likely" }
                ),
                format!("{b} draws for {items} items sounds like plenty."),
                format!("You complete it with probability {}.", pct(p)),
                format!(
                    "On average {mean:.1} draws are needed, and the spread is wide: the last few items can \
                     take a long time to show up."
                ),
                "collectors",
                "Collector",
                "complete within the budget",
            )
        }
        (Paradox::Coupon { items, weights, budget: Some(b) }, Lang::Ja) => {
            let mean = match weights {
                Some(w) => exact::coupon_mean_weighted(&w.probs),
                None => exact::coupon_mean_equal(*items),
            };
            t(
                format!("{b}回以内のコンプリート(全{items}種)"),
                format!(
                    "全{items}種のガチャ{}で、{b}回以内に全種そろう確率は?",
                    if weights.is_some() { "(排出率は指定どおり)" } else { "(どれも同じ確率)" }
                ),
                format!("{items}種に{b}回なら十分そう。"),
                format!("{b}回以内にそろう確率は{}です。", pct(p)),
                format!("そろうまでの平均は{mean:.1}回で、ばらつきも大きく、最後の数種がなかなか出ないことがあります。"),
                "人",
                "人目",
                "予算内にそろう確率",
            )
        }
        (Paradox::Secretary { candidates, skip }, Lang::En) => t(
            format!("Secretary problem ({candidates} candidates)"),
            format!(
                "{candidates} candidates are interviewed in random order. After each one you must hire \
                 or reject on the spot, and only the very best will do. What is your chance?"
            ),
            format!(
                "Without seeing everyone you are guessing, so about 1/{candidates} ({}).",
                pct(1.0 / *candidates as f64)
            ),
            format!(
                "Reject the first {skip} ({}), then hire the first who beats them all: the very best \
                 is hired {} of the time.",
                pct(*skip as f64 / *candidates as f64),
                pct(p)
            ),
            "The first part sets the bar. As the number of candidates grows, the best share to \
             reject and the chance of success both approach 1/e, about 36.8%."
                .into(),
            "hirings",
            "Hiring",
            "the best is hired",
        ),
        (Paradox::Secretary { candidates, skip }, Lang::Ja) => t(
            format!("秘書問題({candidates}人の候補者)"),
            format!(
                "{candidates}人の候補者をランダムな順に面接し、1人ごとにその場で採用か不採用を決めます。\
                 最も優秀な1人を採用できる確率は?"
            ),
            format!(
                "全員を見る前に決めるのだから当てずっぽうと同じで、1/{candidates}({})程度。",
                pct(1.0 / *candidates as f64)
            ),
            format!(
                "最初の{skip}人({})は見送り、それまでの全員より良い最初の人を採用すると、{}の確率で\
                 最も優秀な人を採用できます。",
                pct(*skip as f64 / *candidates as f64),
                pct(p)
            ),
            "最初の何人かで基準を作るのがこつです。候補者が増えるほど、見送る割合と成功率はどちらも\
             1/e(約36.8%)に近づきます。"
                .into(),
            "回",
            "回目",
            "最も優秀な人を採用する確率",
        ),
        (Paradox::Buffon { length, spacing }, Lang::En) => t(
            format!("Buffon's needle (length {}, lines {} apart)", num(*length), num(*spacing)),
            format!(
                "Drop a needle of length {} on a floor ruled with lines {} apart. How often does it \
                 cross a line?",
                num(*length),
                num(*spacing)
            ),
            "Hard to say, and surely nothing to do with circles.".into(),
            format!("It crosses with probability 2l/(pi t) = {}, so counting crossings estimates pi.", pct(p)),
            "Whether it crosses depends on the needle's angle, and averaging over every angle \
             brings in pi. The simulation itself never uses pi: each angle comes from a random \
             point in a quarter disk."
                .into(),
            "needles",
            "Needle",
            "a needle crosses a line",
        ),
        (Paradox::Buffon { length, spacing }, Lang::Ja) => t(
            format!("ビュフォンの針(長さ{}、線の間隔{})", num(*length), num(*spacing)),
            format!(
                "間隔{}で平行線を引いた床に、長さ{}の針を落とします。針が線と交わる確率は?",
                num(*spacing),
                num(*length)
            ),
            "見当がつかないし、円とは関係なさそう。".into(),
            format!("交わる確率は2l/(πt) = {}。交わった回数を数えれば円周率を推定できます。", pct(p)),
            "交わるかどうかは針の角度で決まり、あらゆる角度で平均するとπが現れます。このシミュレーション\
             自体はπを使いません。角度は四分円の中のランダムな点から作っています。"
                .into(),
            "本",
            "本目",
            "針が線と交わる確率",
        ),
        (Paradox::BaseRate { prevalence, sensitivity, specificity }, lang) => {
            let (tp, fp) = (
                (10_000.0 * prevalence * sensitivity).round(),
                (10_000.0 * (1.0 - prevalence) * (1.0 - specificity)).round(),
            );
            match lang {
                Lang::En => t(
                    format!("Base rate fallacy (a disease in {} of people)", pct(*prevalence)),
                    format!(
                        "A disease affects {} of people. A test finds {} of the sick and wrongly flags {} \
                         of the healthy. You test positive. How likely are you to be sick?",
                        pct(*prevalence),
                        pct(*sensitivity),
                        pct(1.0 - specificity)
                    ),
                    format!("The test is right about {} of the time, so about that.", pct(*sensitivity)),
                    format!("Only {}.", pct(p)),
                    format!(
                        "The healthy are so many that their false alarms outnumber the true positives: in \
                         10,000 people, about {tp} sick people and {fp} healthy people test positive."
                    ),
                    "positive tests",
                    "Person",
                    "sick, given a positive test",
                ),
                Lang::Ja => t(
                    format!("基準率の誤り(有病率{}の病気)", pct(*prevalence)),
                    format!(
                        "ある病気の有病率は{}。検査は病気の人の{}を陽性と判定し、健康な人の{}を誤って陽性と\
                         判定します。あなたは陽性でした。本当に病気である確率は?",
                        pct(*prevalence),
                        pct(*sensitivity),
                        pct(1.0 - specificity)
                    ),
                    format!("検査は{}当たるのだから、それくらい。", pct(*sensitivity)),
                    format!("わずか{}です。", pct(p)),
                    format!(
                        "健康な人がずっと多いので、その誤報が本当の陽性を上回ります。1万人なら、陽性になる\
                         病気の人は約{tp}人、健康な人は約{fp}人です。"
                    ),
                    "件",
                    "人目",
                    "陽性のとき病気である確率",
                ),
            }
        }
        (Paradox::Penney { first, second }, lang) => {
            let (a, b, n) = (coins_text(first), coins_text(second), first.len());
            let best = coins_text(&exact::penney_best_reply(first));
            let exact = paradox.exact().unwrap_or_default();
            match lang {
                Lang::En => t(
                    format!("Penney's game ({a} against {b})"),
                    format!(
                        "Two players each pick {n} coin flips, and a coin is flipped until one of the two \
                         sequences appears. The first player picks {a}, the second {b}. Who is more likely \
                         to win?"
                    ),
                    format!("Every sequence of {n} flips is equally likely, so it is 50/50."),
                    format!("The second player wins {} of the time ({exact}).", pct(p)),
                    format!(
                        "Sequences overlap with themselves and with each other in different ways, so some \
                         tend to come first. Whatever the first player picks, the second can answer with one \
                         that wins more often: against {a}, Conway's reply is {best}."
                    ),
                    "games",
                    "Game",
                    "the second player wins",
                ),
                Lang::Ja => t(
                    format!("ペニーのゲーム({a}対{b})"),
                    format!(
                        "2人がそれぞれ{n}回分の表裏の並びを選び、どちらかの並びが出るまでコインを投げます。\
                         先手は{a}、後手は{b}。どちらが勝ちやすい?"
                    ),
                    format!("{n}回分の並びはどれも同じ確率で出るので、五分五分。"),
                    format!("後手が{}の確率で勝ちます({exact})。", pct(p)),
                    format!(
                        "並びは自分自身や相手との重なり方が違うため、先に出やすい並びがあります。先手が何を選んでも、\
                         後手はより勝ちやすい並びを選べます。{a}に対するコンウェイの答えは{best}です。"
                    ),
                    "回",
                    "回目",
                    "後手が勝つ確率",
                ),
            }
        }
        (Paradox::BoyGirl { tuesday: true }, Lang::En) => t(
            "Boy or girl paradox, the Tuesday boy".into(),
            "A family has two children, and at least one of them is a boy born on a Tuesday. How \
             likely is it that both are boys?"
                .into(),
            "The other child is a boy or a girl, so 1/2; and the Tuesday surely changes nothing.".into(),
            format!("13/27 ({}).", pct(p)),
            "Of the 196 equally likely ways two children can have a sex and a weekday, 27 include a \
             Tuesday boy, and 13 of those are two boys. This assumes the family was picked because it \
             has such a boy; families without one are left out of the count."
                .into(),
            "families",
            "Family",
            "both are boys",
        ),
        (Paradox::BoyGirl { tuesday: true }, Lang::Ja) => t(
            "男女の謎(火曜日生まれの男の子)".into(),
            "子どもが2人いる家族で、少なくとも1人は火曜日生まれの男の子です。2人とも男の子である確率は?".into(),
            "もう1人は男の子か女の子なので1/2。火曜日かどうかは関係ないはず。".into(),
            format!("13/27({})です。", pct(p)),
            "2人の子どもの性別と曜日の組み合わせ196通りのうち、火曜日生まれの男の子を含むのは27通りで、\
             そのうち2人とも男の子は13通りです。そういう男の子がいることで選ばれた家族だと考えています。\
             条件に合わない家族は数えません。"
                .into(),
            "家族",
            "家族目",
            "2人とも男の子である確率",
        ),
        (Paradox::BoyGirl { tuesday: false }, Lang::En) => t(
            "Boy or girl paradox".into(),
            "A family has two children, and at least one of them is a boy. How likely is it that both \
             are boys?"
                .into(),
            "The other child is a boy or a girl, so 1/2.".into(),
            format!("1/3 ({}).", pct(p)),
            "Of the four equally likely families (boy-boy, boy-girl, girl-boy, girl-girl), three have a \
             boy, and only one of those has two boys. Families without a boy are left out of the count."
                .into(),
            "families",
            "Family",
            "both are boys",
        ),
        (Paradox::BoyGirl { tuesday: false }, Lang::Ja) => t(
            "男女の謎".into(),
            "子どもが2人いる家族で、少なくとも1人は男の子です。2人とも男の子である確率は?".into(),
            "もう1人は男の子か女の子なので1/2。".into(),
            format!("1/3({})です。", pct(p)),
            "同じ確率の4通り(男男、男女、女男、女女)のうち男の子がいるのは3通りで、2人とも男の子は\
             そのうち1通りだけです。男の子のいない家族は数えません。"
                .into(),
            "家族",
            "家族目",
            "2人とも男の子である確率",
        ),
        (Paradox::Bertrand { method }, Lang::En) => t(
            format!("Bertrand's paradox ({})", match method {
                Chord::Endpoints => "random endpoints",
                Chord::Radius => "a random point on a random radius",
                Chord::Midpoint => "a random midpoint",
            }),
            "Draw a random chord of a circle. How likely is it to be longer than a side of the \
             equilateral triangle inscribed in the circle?"
                .into(),
            "There must be one right answer.".into(),
            format!(
                "Drawn this way, {} ({}). But random endpoints give 1/3, a random point on a random radius \
                 1/2, and a random midpoint 1/4.",
                paradox.exact().unwrap_or_default(),
                pct(p)
            ),
            "\"A random chord\" means nothing until you say how it is drawn, and the three natural ways \
             give three answers. Try --method endpoints, radius or midpoint."
                .into(),
            "chords",
            "Chord",
            "the chord is longer",
        ),
        (Paradox::Bertrand { method }, Lang::Ja) => t(
            format!("ベルトランの逆説({})", match method {
                Chord::Endpoints => "円周上の2点をランダムに選ぶ",
                Chord::Radius => "ランダムな半径上のランダムな点",
                Chord::Midpoint => "中点をランダムに選ぶ",
            }),
            "円にランダムな弦を引きます。その弦が、円に内接する正三角形の1辺より長い確率は?".into(),
            "答えは1つに決まるはず。".into(),
            format!(
                "この引き方なら{}({})。ですが、端点をランダムに選ぶと1/3、ランダムな半径上の点なら1/2、\
                 中点をランダムに選ぶと1/4になります。",
                paradox.exact().unwrap_or_default(),
                pct(p)
            ),
            "「ランダムな弦」は引き方を決めるまで意味が定まらず、自然な3つの引き方で答えが3つに分かれます。\
             --method endpoints、radius、midpointで試せます。"
                .into(),
            "本",
            "本目",
            "弦が長い確率",
        ),
    }
}
