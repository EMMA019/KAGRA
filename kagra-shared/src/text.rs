//! Text layout: lines that fit a width, broken the way a Japanese typesetter breaks them.
//!
//! [`wrap`] measures with the real glyph advances (`font`), keeps Latin words and numbers
//! whole, and follows the line-breaking rules of Japanese text (kinsoku):
//!
//! - a line never starts with `、。！？）」ー` or a small kana (`っゃゅょ`),
//! - a line never ends with an opening bracket (`「（『`),
//! - a number stays with its counter (`12人`, `10年`, `3-2`),
//! - `……` and `――` stay in one piece,
//! - the last line is not left with a single letter.
//!
//! Lines are returned as strings, so a kit draws them with its own colours and spacing.
//! [`Wrap`] adds a line limit with an ellipsis; [`fit_size`] shrinks one line to a width;
//! [`reveal`] shows the first letters of wrapped lines (text typed out letter by letter
//! without words jumping between lines).

use crate::font;
use crate::scene::TextFace;

/// May a line not start with this letter?
fn no_line_start(c: char) -> bool {
    matches!(
        c,
        '、' | '。'
            | '，'
            | '．'
            | '・'
            | '：'
            | '；'
            | '？'
            | '！'
            | 'ヽ'
            | 'ヾ'
            | '々'
            | 'ー'
            | '）'
            | '〕'
            | '］'
            | '｝'
            | '〉'
            | '》'
            | '」'
            | '』'
            | '】'
            | '〙'
            | '〗'
            | '〟'
            | '’'
            | '”'
            | '｠'
            | '»'
            | '‥'
            | '…'
            | '―'
            | '—'
            | '〜'
            | '～'
            | '％'
            | '℃'
            | '°'
            | 'ぁ'
            | 'ぃ'
            | 'ぅ'
            | 'ぇ'
            | 'ぉ'
            | 'っ'
            | 'ゃ'
            | 'ゅ'
            | 'ょ'
            | 'ゎ'
            | 'ァ'
            | 'ィ'
            | 'ゥ'
            | 'ェ'
            | 'ォ'
            | 'ッ'
            | 'ャ'
            | 'ュ'
            | 'ョ'
            | 'ヮ'
            | 'ヵ'
            | 'ヶ'
            | ')'
            | ']'
            | '}'
            | ','
            | '.'
            | '!'
            | '?'
            | ':'
            | ';'
            | '%'
    )
}

/// May a line not end with this letter?
fn no_line_end(c: char) -> bool {
    matches!(
        c,
        '（' | '〔'
            | '［'
            | '｛'
            | '〈'
            | '《'
            | '「'
            | '『'
            | '【'
            | '〘'
            | '〖'
            | '〝'
            | '‘'
            | '“'
            | '｟'
            | '«'
            | '('
            | '['
            | '{'
    )
}

/// Letters that break anywhere (kana, kanji, full-width forms, symbols), unlike Latin words.
fn breaks_anywhere(c: char) -> bool {
    let u = c as u32;
    (0x2E80..=0x9FFF).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFE30..=0xFE4F).contains(&u)
        || (0xFF00..=0xFFEF).contains(&u)
        || (0x2190..=0x2BFF).contains(&u)
        || (0x20000..=0x2FFFF).contains(&u)
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || ('０'..='９').contains(&c)
}

/// A kanji (a counter after a number: 人, 年, 回).
fn is_kanji(c: char) -> bool {
    (0x4E00..=0x9FFF).contains(&(c as u32))
}

/// A piece of a paragraph that is never split: a Latin word, a number with its counter,
/// one kana or kanji with the brackets and marks that hang on it. `space` pieces are the
/// gaps between words; they are dropped at the ends of a line.
#[derive(Clone, Debug, PartialEq)]
struct Piece {
    text: String,
    width: f32,
    space: bool,
}

fn pieces(para: &str, size: f32, face: TextFace) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    // The piece being built takes Latin letters (a word in progress).
    let mut word_open = false;
    // The piece being built ends with an opening bracket: the next letter joins it.
    let mut open_bracket = false;
    for c in para.chars() {
        if c == ' ' || c == '\u{3000}' || c == '\t' {
            out.push(Piece {
                text: c.to_string(),
                width: 0.0,
                space: true,
            });
            word_open = false;
            open_bracket = false;
            continue;
        }
        let last = out.last().filter(|p| !p.space);
        let prev = last.and_then(|p| p.text.chars().last());
        // Digits behave like Latin letters, full-width ones too (１２人).
        let anywhere = breaks_anywhere(c) && !is_digit(c);
        let join = match (last, prev) {
            (Some(_), Some(p)) => {
                open_bracket
                    || no_line_start(c)
                    // A word goes on.
                    || (word_open && !anywhere)
                    // The number's counter: 12人, 10年.
                    || (is_digit(p) && is_kanji(c) && word_open)
                    // …… and ―― in one piece.
                    || (p == c && matches!(c, '…' | '‥' | '―' | '—'))
            }
            _ => false,
        };
        if join {
            let p = out.last_mut().expect("joined to a piece");
            p.text.push(c);
        } else {
            out.push(Piece {
                text: c.to_string(),
                width: 0.0,
                space: false,
            });
        }
        open_bracket = no_line_end(c);
        // A Latin letter or a digit keeps the word open; a kana, a kanji (the counter
        // just joined) or a Japanese mark ends it.
        word_open = !anywhere;
    }
    for p in &mut out {
        p.width = font::measure_text_face(&p.text, size, face);
    }
    out
}

/// How to break and limit a text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wrap {
    pub size: f32,
    pub width: f32,
    pub face: TextFace,
    /// At most this many lines; the last one ends with `…` when text is left over. 0: no limit.
    pub max_lines: usize,
}

impl Wrap {
    pub fn new(size: f32, width: f32) -> Self {
        Self {
            size,
            width,
            face: TextFace::Ui,
            max_lines: 0,
        }
    }

    pub fn face(mut self, face: TextFace) -> Self {
        self.face = face;
        self
    }

    pub fn max_lines(mut self, n: usize) -> Self {
        self.max_lines = n;
        self
    }

    /// The lines of `text`. `\n` starts a new line; an empty paragraph is an empty line.
    pub fn lines(&self, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            if para.trim().is_empty() {
                out.push(String::new());
                continue;
            }
            self.paragraph(para, &mut out);
        }
        // Blank lines at the very end are not lines.
        while out.last().is_some_and(|l| l.is_empty()) {
            out.pop();
        }
        if self.max_lines > 0 && out.len() > self.max_lines {
            out.truncate(self.max_lines);
            if let Some(last) = out.last_mut() {
                *last = self.with_ellipsis(last);
            }
        }
        out
    }

    fn paragraph(&self, para: &str, out: &mut Vec<String>) {
        let ps = pieces(para, self.size, self.face);
        let space_w = font::measure_text_face(" ", self.size, self.face);
        // Lines as runs of piece indices.
        let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
        let mut used = 0.0f32;
        // Spaces seen since the last word on this line (drawn only between words).
        let mut gap = 0usize;
        for (i, p) in ps.iter().enumerate() {
            if p.space {
                gap += 1;
                continue;
            }
            let line = lines.last_mut().expect("a line");
            let lead = if line.is_empty() {
                0.0
            } else {
                gap as f32 * space_w
            };
            if !line.is_empty() && used + lead + p.width > self.width + 0.01 {
                lines.push(vec![i]);
                used = p.width;
            } else {
                // Keep the gap's spaces with the word that follows them.
                if !line.is_empty() {
                    for k in i - gap..i {
                        line.push(k);
                    }
                }
                line.push(i);
                used += lead + p.width;
            }
            gap = 0;
        }
        // No single letter alone on the last line: bring one piece down to it.
        if lines.len() >= 2 {
            let n = lines.len();
            let last_chars: usize = lines[n - 1]
                .iter()
                .map(|&k| ps[k].text.chars().count())
                .sum();
            let words_above = lines[n - 2].iter().filter(|&&k| !ps[k].space).count();
            if lines[n - 1].len() == 1 && last_chars <= 2 && words_above >= 3 {
                let mut moved = Vec::new();
                while let Some(k) = lines[n - 2].pop() {
                    moved.push(k);
                    if !ps[k].space {
                        break;
                    }
                }
                moved.reverse();
                let w: f32 = moved
                    .iter()
                    .chain(lines[n - 1].iter())
                    .map(|&k| if ps[k].space { space_w } else { ps[k].width })
                    .sum();
                if w <= self.width + 0.01 {
                    let tail = std::mem::take(&mut lines[n - 1]);
                    lines[n - 1] = moved.into_iter().chain(tail).collect();
                    // The line above may now end with a space.
                    while lines[n - 2].last().is_some_and(|&k| ps[k].space) {
                        lines[n - 2].pop();
                    }
                } else {
                    lines[n - 2].extend(moved);
                }
            }
        }
        for line in lines {
            let mut s = String::new();
            for k in line {
                s.push_str(&ps[k].text);
            }
            let s = s.trim().to_string();
            // One piece wider than the line (a long word, a URL): cut it by letters.
            if font::measure_text_face(&s, self.size, self.face) > self.width + 0.01
                && s.chars().count() > 1
            {
                self.hard_break(&s, out);
            } else {
                out.push(s);
            }
        }
    }

    fn hard_break(&self, s: &str, out: &mut Vec<String>) {
        let mut line = String::new();
        let mut used = 0.0;
        for c in s.chars() {
            let w = font::measure_text_face(c.encode_utf8(&mut [0; 4]), self.size, self.face);
            if !line.is_empty() && used + w > self.width + 0.01 {
                out.push(std::mem::take(&mut line));
                used = 0.0;
            }
            line.push(c);
            used += w;
        }
        if !line.is_empty() {
            out.push(line);
        }
    }

    /// `line` cut so that it and a closing `…` fit the width.
    fn with_ellipsis(&self, line: &str) -> String {
        let dots = "…";
        let dots_w = font::measure_text_face(dots, self.size, self.face);
        let mut s: String = line.trim_end().to_string();
        while !s.is_empty()
            && font::measure_text_face(&s, self.size, self.face) + dots_w > self.width + 0.01
        {
            s.pop();
        }
        // No mark that hangs on the letter before it in front of the dots.
        while s
            .chars()
            .last()
            .is_some_and(|c| c == ' ' || c == '、' || c == '。' || no_line_end(c))
        {
            s.pop();
        }
        s.push_str(dots);
        s
    }
}

/// Lines of `text` no wider than `width` at `size` (the UI face, no line limit).
pub fn wrap(text: &str, size: f32, width: f32) -> Vec<String> {
    Wrap::new(size, width).lines(text)
}

/// The width of the widest line.
pub fn widest(lines: &[String], size: f32, face: TextFace) -> f32 {
    lines
        .iter()
        .map(|l| font::measure_text_face(l, size, face))
        .fold(0.0, f32::max)
}

/// The text size up to `size` at which `text` fits in `room` on one line.
pub fn fit_size(text: &str, size: f32, room: f32) -> f32 {
    let wide = font::measure_text(text, size);
    if wide > room && wide > 0.0 {
        size * (room.max(1.0) / wide)
    } else {
        size
    }
}

/// The first `letters` letters of wrapped `lines` (a text typed out: the lines are those
/// of the whole text, so no word jumps to the next line as it grows).
pub fn reveal(lines: &[String], letters: usize) -> Vec<String> {
    let mut left = letters;
    let mut out = Vec::new();
    for l in lines {
        if left == 0 {
            break;
        }
        let n = l.chars().count();
        if n <= left {
            out.push(l.clone());
            left -= n;
        } else {
            out.push(l.chars().take(left).collect());
            left = 0;
        }
    }
    out
}

/// The number of letters in wrapped lines (what [`reveal`] counts up to).
pub fn letters(lines: &[String]) -> usize {
    lines.iter().map(|l| l.chars().count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fits(lines: &[String], size: f32, width: f32) {
        for l in lines {
            let w = font::measure_text(l, size);
            assert!(w <= width + 0.5, "'{l}' is {w:.1} wide in {width}");
        }
    }

    /// The rules, checked on every width from narrow to wide: no line starts with a
    /// closing mark or a small kana, none ends with an opening bracket, numbers stay whole,
    /// nothing is lost, and every line fits.
    #[test]
    fn japanese_lines_follow_kinsoku_at_every_width() {
        let text =
            "4月。あなたは日向高校野球部の新しい監督。部員は12人、夏はもう10年も勝てていない。\
                    スコアブックを抱えた1年生が駆けてきた。「マネージャーの七瀬ことりです！三年間、\
                    よろしくお願いします！」キャッチャーが「ナイスボール……！」と叫んだ（3-2）。";
        let size = 16.0;
        for width in (90..=360).step_by(7) {
            let width = width as f32;
            let lines = wrap(text, size, width);
            fits(&lines, size, width);
            assert_eq!(
                lines.concat(),
                text,
                "nothing is lost or added at width {width}"
            );
            for (i, l) in lines.iter().enumerate() {
                let first = l.chars().next().unwrap();
                let last = l.chars().last().unwrap();
                assert!(!no_line_start(first), "line {i} starts with '{first}': {l}");
                assert!(!no_line_end(last), "line {i} ends with '{last}': {l}");
                if i + 1 < lines.len() {
                    let next = lines[i + 1].chars().next().unwrap();
                    assert!(
                        !(is_digit(last) && (is_digit(next) || is_kanji(next))),
                        "a number is split at width {width}: '{l}' / '{}'",
                        lines[i + 1]
                    );
                    assert!(
                        !(last == '…' && next == '…'),
                        "…… is split at width {width}"
                    );
                }
            }
        }
    }

    #[test]
    fn english_breaks_at_spaces_and_keeps_words() {
        let text = "The coach names number 1 before the summer: 148 km/h, a 3-2 count, don't rush.";
        for width in (70..=400).step_by(11) {
            let width = width as f32;
            let lines = wrap(text, 15.0, width);
            fits(&lines, 15.0, width);
            assert_eq!(lines.join(" "), text, "width {width}");
            for l in &lines {
                assert!(!l.starts_with(' ') && !l.ends_with(' '), "'{l}'");
            }
        }
    }

    #[test]
    fn mixed_text_keeps_latin_words_inside_japanese() {
        let lines = wrap(
            "今日のMVPはShortstopの田中、打率は.345でOPS1.021！",
            16.0,
            120.0,
        );
        fits(&lines, 16.0, 120.0);
        let all = lines.concat();
        assert_eq!(all, "今日のMVPはShortstopの田中、打率は.345でOPS1.021！");
        for word in ["MVP", "Shortstop", ".345", "OPS1.021"] {
            assert!(
                lines.iter().any(|l| l.contains(word)),
                "'{word}' is split: {lines:?}"
            );
        }
    }

    #[test]
    fn a_word_wider_than_the_line_is_cut_by_letters() {
        let lines = wrap("https://example.invalid/a/very/long/address", 14.0, 80.0);
        assert!(lines.len() > 2);
        fits(&lines, 14.0, 80.0);
        assert_eq!(
            lines.concat(),
            "https://example.invalid/a/very/long/address"
        );
    }

    #[test]
    fn newlines_and_blank_lines_are_kept() {
        let lines = wrap("一行目\n\n三行目\n", 16.0, 300.0);
        assert_eq!(lines, vec!["一行目", "", "三行目"]);
        assert!(wrap("", 16.0, 300.0).is_empty());
        assert!(wrap("   ", 16.0, 300.0).is_empty());
    }

    #[test]
    fn the_last_line_is_not_one_letter() {
        // Find a width where the plain greedy break would leave one letter, and check
        // that it never does.
        let text = "夏の大会まであと少し、気持ちをひとつにして練習しよう";
        for width in (60..=300).step_by(3) {
            let lines = wrap(text, 16.0, width as f32);
            fits(&lines, 16.0, width as f32);
            if lines.len() >= 2 && width >= 80 {
                let last = lines.last().unwrap().chars().count();
                assert!(last >= 2, "one letter alone at width {width}: {lines:?}");
            }
            assert_eq!(lines.concat(), text);
        }
    }

    #[test]
    fn a_line_limit_ends_with_an_ellipsis_that_fits() {
        let text =
            "部員は12人、夏はもう10年も勝てていない。スコアブックを抱えた1年生が駆けてきた。";
        let w = Wrap::new(16.0, 150.0).max_lines(2);
        let lines = w.lines(text);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'), "{lines:?}");
        fits(&lines, 16.0, 150.0);
        // Short text is left alone.
        assert_eq!(w.lines("短い"), vec!["短い"]);
    }

    #[test]
    fn reveal_types_out_the_final_lines() {
        let lines = wrap(
            "「ナイスボール！」と叫んだ。次の一球に集中しよう。",
            16.0,
            150.0,
        );
        let n = letters(&lines);
        assert_eq!(reveal(&lines, n), lines);
        assert!(reveal(&lines, 0).is_empty());
        for k in 1..n {
            let shown = reveal(&lines, k);
            assert_eq!(letters(&shown), k);
            // Every shown line is the start of the final line.
            for (a, b) in shown.iter().zip(&lines) {
                assert!(b.starts_with(a.as_str()));
            }
        }
    }

    #[test]
    fn fit_size_shrinks_only_what_does_not_fit() {
        assert_eq!(fit_size("GO", 15.0, 80.0), 15.0);
        let s = fit_size("NEXT BATTER", 15.0, 60.0);
        assert!(s < 15.0);
        assert!(font::measure_text("NEXT BATTER", s) <= 60.01);
    }

    #[test]
    fn the_display_face_measures_with_its_own_letters() {
        let w = Wrap::new(20.0, 120.0).face(TextFace::Display);
        let lines = w.lines("STAGE 12 CLEAR BONUS");
        for l in &lines {
            assert!(font::measure_text_face(l, 20.0, TextFace::Display) <= 120.5);
        }
        assert_eq!(lines.join(" "), "STAGE 12 CLEAR BONUS");
        assert!(widest(&lines, 20.0, TextFace::Display) <= 120.5);
    }
}
