//! The built-in emoji the `:` picker searches: a shortcode (without the colons) and the emoji. A fixed table of common
//! emoji; the full set is a follow-up.

/// Shortcode and emoji, in the order the picker shows them when the query is empty.
pub const EMOJI: &[(&str, &str)] = &[
    ("smile", "😄"),
    ("grin", "😁"),
    ("joy", "😂"),
    ("rofl", "🤣"),
    ("smiley", "😃"),
    ("wink", "😉"),
    ("blush", "😊"),
    ("heart_eyes", "😍"),
    ("kissing_heart", "😘"),
    ("yum", "😋"),
    ("sunglasses", "😎"),
    ("nerd", "🤓"),
    ("thinking", "🤔"),
    ("neutral_face", "😐"),
    ("expressionless", "😑"),
    ("unamused", "😒"),
    ("roll_eyes", "🙄"),
    ("grimacing", "😬"),
    ("relieved", "😌"),
    ("pensive", "😔"),
    ("sleeping", "😴"),
    ("mask", "😷"),
    ("confused", "😕"),
    ("worried", "😟"),
    ("cry", "😢"),
    ("sob", "😭"),
    ("angry", "😠"),
    ("rage", "😡"),
    ("scream", "😱"),
    ("flushed", "😳"),
    ("astonished", "😲"),
    ("exploding_head", "🤯"),
    ("partying", "🥳"),
    ("hugs", "🤗"),
    ("melting", "🫠"),
    ("skull", "💀"),
    ("ghost", "👻"),
    ("alien", "👽"),
    ("robot", "🤖"),
    ("poop", "💩"),
    ("clap", "👏"),
    ("wave", "👋"),
    ("ok_hand", "👌"),
    ("v", "✌️"),
    ("crossed_fingers", "🤞"),
    ("raised_hands", "🙌"),
    ("pray", "🙏"),
    ("muscle", "💪"),
    ("thumbsup", "👍"),
    ("thumbsdown", "👎"),
    ("point_up", "☝️"),
    ("point_right", "👉"),
    ("point_left", "👈"),
    ("point_down", "👇"),
    ("raised_hand", "✋"),
    ("fist", "✊"),
    ("handshake", "🤝"),
    ("eyes", "👀"),
    ("brain", "🧠"),
    ("heart", "❤️"),
    ("orange_heart", "🧡"),
    ("yellow_heart", "💛"),
    ("green_heart", "💚"),
    ("blue_heart", "💙"),
    ("purple_heart", "💜"),
    ("black_heart", "🖤"),
    ("broken_heart", "💔"),
    ("sparkling_heart", "💖"),
    ("star", "⭐"),
    ("star2", "🌟"),
    ("sparkles", "✨"),
    ("zap", "⚡"),
    ("fire", "🔥"),
    ("boom", "💥"),
    ("100", "💯"),
    ("tada", "🎉"),
    ("confetti", "🎊"),
    ("balloon", "🎈"),
    ("gift", "🎁"),
    ("trophy", "🏆"),
    ("medal", "🏅"),
    ("crown", "👑"),
    ("gem", "💎"),
    ("bulb", "💡"),
    ("warning", "⚠️"),
    ("x", "❌"),
    ("white_check_mark", "✅"),
    ("heavy_check_mark", "✔️"),
    ("question", "❓"),
    ("exclamation", "❗"),
    ("information", "ℹ️"),
    ("no_entry", "⛔"),
    ("stop_sign", "🛑"),
    ("bell", "🔔"),
    ("loudspeaker", "📢"),
    ("memo", "📝"),
    ("pencil", "✏️"),
    ("book", "📖"),
    ("books", "📚"),
    ("bookmark", "🔖"),
    ("link", "🔗"),
    ("paperclip", "📎"),
    ("pushpin", "📌"),
    ("round_pushpin", "📍"),
    ("calendar", "📅"),
    ("clock", "🕒"),
    ("hourglass", "⌛"),
    ("rocket", "🚀"),
    ("airplane", "✈️"),
    ("car", "🚗"),
    ("bike", "🚲"),
    ("house", "🏠"),
    ("office", "🏢"),
    ("earth", "🌍"),
    ("moon", "🌙"),
    ("sun", "☀️"),
    ("cloud", "☁️"),
    ("rain", "🌧️"),
    ("snowflake", "❄️"),
    ("rainbow", "🌈"),
    ("tree", "🌳"),
    ("seedling", "🌱"),
    ("flower", "🌸"),
    ("cat", "🐱"),
    ("dog", "🐶"),
    ("fox", "🦊"),
    ("owl", "🦉"),
    ("penguin", "🐧"),
    ("unicorn", "🦄"),
    ("bee", "🐝"),
    ("apple", "🍎"),
    ("pizza", "🍕"),
    ("hamburger", "🍔"),
    ("taco", "🌮"),
    ("coffee", "☕"),
    ("beer", "🍺"),
    ("cake", "🍰"),
    ("gamepad", "🎮"),
    ("headphones", "🎧"),
    ("musical_note", "🎵"),
    ("film", "🎞️"),
    ("camera", "📷"),
    ("movie_camera", "🎥"),
    ("art", "🎨"),
    ("computer", "💻"),
    ("keyboard", "⌨️"),
    ("phone", "📱"),
    ("satellite", "📡"),
    ("battery", "🔋"),
    ("hammer", "🔨"),
    ("wrench", "🔧"),
    ("gear", "⚙️"),
    ("lock", "🔒"),
    ("unlock", "🔓"),
    ("key", "🔑"),
    ("shield", "🛡️"),
    ("mag", "🔍"),
    ("chart", "📈"),
    ("bar_chart", "📊"),
    ("package", "📦"),
    ("dart", "🎯"),
    ("construction", "🚧"),
    ("recycle", "♻️"),
    ("arrow_right", "➡️"),
    ("arrow_left", "⬅️"),
    ("arrow_up", "⬆️"),
    ("arrow_down", "⬇️"),
    ("repeat", "🔁"),
    ("infinity", "♾️"),
    ("hundred_points", "💯"),
    ("sos", "🆘"),
    ("new", "🆕"),
    ("free", "🆓"),
    ("ok", "🆗"),
    ("up", "🆙"),
    ("cool", "🆒"),
    ("zzz", "💤"),
    ("speech_balloon", "💬"),
    ("thought_balloon", "💭"),
    ("crystal_ball", "🔮"),
];

/// The entries whose shortcode starts with `query` (lowercase), then the ones that contain it, at most 40.
/// An empty query gives the first 40 entries of the table.
pub fn search(query: &str) -> Vec<(&'static str, &'static str)> {
    let query = query.to_lowercase();
    EMOJI
        .iter()
        .filter(|(code, _)| code.starts_with(query.as_str()))
        .chain(
            EMOJI.iter().filter(|(code, _)| {
                !code.starts_with(query.as_str()) && code.contains(query.as_str())
            }),
        )
        .copied()
        .take(40)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcodes_are_unique() {
        let mut codes: Vec<&str> = EMOJI.iter().map(|(code, _)| *code).collect();
        codes.sort_unstable();
        let before = codes.len();
        codes.dedup();
        assert_eq!(before, codes.len());
    }

    #[test]
    fn a_prefix_match_comes_first() {
        let found = search("sm");
        assert_eq!(found.first().map(|(code, _)| *code), Some("smile"));
    }

    #[test]
    fn the_table_has_about_150_entries() {
        assert!(EMOJI.len() >= 140, "only {} entries", EMOJI.len());
    }
}
