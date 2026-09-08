mod prose;

use crate::spell;
use harper_core::linting::LintGroup;
use harper_core::spell::FstDictionary;
use harper_core::Dialect;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

#[cxx::bridge(namespace = "blogawrite")]
mod ffi {
    /// Something the checker found in a block: where it is, counted in the UTF-16 units Qt
    /// counts a QString in; what is wrong with it, in a few words of markdown; and what
    /// could stand in its place, likeliest first.
    ///
    /// A misspelling and a turn of phrase are the same thing here. They are marked the
    /// same way, offered the same way, and accepted the same way; only the message says
    /// which of the two it was.
    #[derive(Clone)]
    struct Lint {
        at: u32,
        len: u32,
        message: String,
        replacements: Vec<String>,
        /// The misspelled word itself, where that is what this is: the one the writer
        /// would be taking into their dictionary. Empty for a turn of phrase, which is
        /// nothing a dictionary has an opinion about.
        word: String,
        /// Whether the replacements are still to be worked out. Asking the dictionary
        /// what a misspelled word should have been costs more than checking the block it
        /// is in, and only the one lint the cursor stands in is ever read.
        pending: bool,
    }

    extern "Rust" {
        /// Whether the checker is up yet. Its dictionaries and its rules take the better
        /// part of a second between them — longer than the window takes to appear — so
        /// until it is loaded nothing is checked, and the editor is told so rather than
        /// made to wait.
        fn checker_ready() -> bool;
        /// Changes whenever cached findings may have changed. Qt polls this small value;
        /// checking itself stays off the UI thread.
        fn checker_generation() -> u64;

        /// Cached findings for one block, scheduling the work when they are not ready yet.
        fn request_check(text: &str, markdown: bool) -> Vec<Lint>;

        /// Take a word into the writer's own dictionary. Everything checked since is
        /// checked against it, and it is theirs again the next time they open the editor.
        fn learn(word: &str);
    }
}

pub use ffi::Lint;

const CACHE_LIMIT: usize = 256;

type CheckKey = (String, bool);

struct CheckCache {
    found: HashMap<CheckKey, Vec<Lint>>,
    order: VecDeque<CheckKey>,
    pending: HashSet<CheckKey>,
}

struct Checker {
    #[cfg(test)]
    group: Arc<Mutex<LintGroup>>,
    cache: Arc<Mutex<CheckCache>>,
    requests: mpsc::Sender<CheckKey>,
}

static CHECKER: OnceLock<Checker> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Whether the writer wants to hear from the checker at all. They are writing; being told
/// what is wrong with a sentence half-thought is not always help, so it can be turned off
/// and on from the keyboard. On to begin with — that is what the checker is there for.
static CHECKING: AtomicBool = AtomicBool::new(true);

/// Build the checker on threads of its own, so that the four hundred milliseconds it
/// takes are spent while Qt is still bringing itself up.
pub fn preload() {
    spell::preload();
    std::thread::spawn(|| {
        let mut rules = LintGroup::new_curated(FstDictionary::curated(), Dialect::American);
        // Personal words and lazy suggestions need the spelling pass below. Do not also
        // run Harper's spell checker only to throw every one of its results away.
        rules.config.set_rule_enabled("SpellCheck", false);

        let group = Arc::new(Mutex::new(rules));
        let cache = Arc::new(Mutex::new(CheckCache {
            found: HashMap::new(),
            order: VecDeque::new(),
            pending: HashSet::new(),
        }));
        let (send, receive) = mpsc::channel();
        if CHECKER
            .set(Checker {
                #[cfg(test)]
                group: group.clone(),
                cache: cache.clone(),
                requests: send,
            })
            .is_err()
        {
            return;
        }
        GENERATION.fetch_add(1, Ordering::Release);

        for key in receive {
            let found = prose::run(&group, &key.0, key.1);
            remember_check(&mut cache.lock().unwrap(), key, found);
            GENERATION.fetch_add(1, Ordering::Release);
        }
    });
}

fn checker_ready() -> bool {
    CHECKER.get().is_some() && spell::ready()
}

pub fn checking() -> bool {
    CHECKING.load(Ordering::Acquire)
}

/// Turn the checker off, or on again. What was found while it was on is kept — the
/// blocks are the same blocks — so putting it back on marks them again without waiting.
/// The generation moves either way: that is what has the blocks on screen look again.
pub fn set_checking(on: bool) {
    CHECKING.store(on, Ordering::Release);
    GENERATION.fetch_add(1, Ordering::Release);
}

/// The checker, when there is anything to be had from it: loaded, and switched on.
fn working() -> Option<&'static Checker> {
    CHECKER.get().filter(|_| spell::ready() && checking())
}

fn checker_generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

/// Return findings already worked out for this block. A miss schedules one and returns
/// immediately; the generation change has Qt ask again when the worker finishes.
pub fn request_check(text: &str, markdown: bool) -> Vec<Lint> {
    let Some(checker) = working() else {
        return Vec::new();
    };
    let key = (text.to_string(), markdown);
    let mut cache = checker.cache.lock().unwrap();
    if let Some(found) = cache.found.get(&key) {
        return found.clone();
    }
    if cache.pending.insert(key.clone()) {
        let _ = checker.requests.send(key);
    }
    Vec::new()
}

/// Synchronous checking is kept inside the Rust core for focused tests. UI callers use
/// [`request_check`] and never wait for Harper.
#[cfg(test)]
fn check(text: &str, markdown: bool) -> Vec<Lint> {
    let Some(checker) = working() else {
        return Vec::new();
    };
    prose::run(&checker.group, text, markdown)
}

fn remember_check(cache: &mut CheckCache, key: CheckKey, found: Vec<Lint>) {
    cache.pending.remove(&key);
    if cache.found.insert(key.clone(), found).is_none() {
        cache.order.push_back(key);
    }
    while cache.order.len() > CACHE_LIMIT {
        if let Some(oldest) = cache.order.pop_front() {
            cache.found.remove(&oldest);
        }
    }
}

/// What the checker makes of the place the cursor is standing in a block, if anything.
/// The narrowest lint wins where several overlap — it is the one that names the words
/// under the cursor — and it is the only one whose replacements are worth working out.
pub fn at(text: &str, cursor: i32) -> Option<Lint> {
    let cursor = u32::try_from(cursor).ok()?;
    #[cfg(test)]
    let checked = check(text, true);
    #[cfg(not(test))]
    let checked = request_check(text, true);
    let mut found = checked
        .into_iter()
        .filter(|lint| cursor >= lint.at && cursor <= lint.at + lint.len)
        .min_by_key(|lint| lint.len)?;
    if found.pending {
        found.replacements = spell::suggestions(&found.word);
        found.pending = false;
    }
    Some(found)
}

/// Take a word into the writer's own dictionary, and forget what was made of the block it
/// was found in: it is spelled right from here on, and the block is asked about again.
pub fn learn(word: &str) {
    spell::learn(word);
    if let Some(checker) = CHECKER.get() {
        let mut cache = checker.cache.lock().unwrap();
        cache.found.clear();
        cache.order.clear();
        cache.pending.clear();
        GENERATION.fetch_add(1, Ordering::Release);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::MutexGuard;

    /// The piece of `text` a lint covers, taken in the UTF-16 units the lint counts in.
    fn covered(text: &str, lint: &Lint) -> String {
        let units: Vec<u16> = text
            .encode_utf16()
            .skip(lint.at as usize)
            .take(lint.len as usize)
            .collect();
        String::from_utf16_lossy(&units)
    }

    /// The checker loads on threads of its own, and the switch that turns it off is one
    /// thing for the whole editor. The tests share both: each waits for the one and takes
    /// the other for as long as it holds what this hands back.
    fn checker() -> MutexGuard<'static, ()> {
        preload();
        while !checker_ready() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        TURN.lock().unwrap_or_else(|held| held.into_inner())
    }

    static TURN: Mutex<()> = Mutex::new(());

    /// The lints of a block, as (what they cover, what is offered for it).
    fn found(text: &str) -> Vec<(String, Vec<String>)> {
        check(text, true)
            .into_iter()
            .map(|lint| {
                let covering = covered(text, &lint);
                let offered = if lint.pending {
                    spell::suggestions(&covering)
                } else {
                    lint.replacements.clone()
                };
                (covering, offered)
            })
            .collect()
    }

    /// What the block covers and what it offers where the cursor is standing.
    fn under_cursor(text: &str, cursor: i32) -> Option<(String, Vec<String>)> {
        let lint = at(text, cursor)?;
        Some((covered(text, &lint), lint.replacements))
    }

    fn covers(text: &str) -> Vec<String> {
        found(text).into_iter().map(|(covering, _)| covering).collect()
    }

    #[test]
    fn finds_a_typo_and_offers_what_was_meant() {
        let _turn = checker();
        let (word, offered) = under_cursor("I recieve mail.", 4).expect("the typo is found");
        assert_eq!(word, "recieve");
        assert!(offered.contains(&"receive".to_string()), "{offered:?}");
    }

    #[test]
    fn finds_a_turn_of_phrase_and_offers_what_was_meant() {
        let _turn = checker();
        let (phrase, offered) =
            under_cursor("This is very unique writing.", 14).expect("the phrase is found");
        assert_eq!(phrase, "very unique");
        assert!(offered.len() > 1, "{offered:?}");
    }

    /// The point of the exercise: one kind of finding, one shape, one way to accept it.
    /// A block with a typo and a bad turn of phrase gives two lints in reading order,
    /// each covering its own words and each with something to put there.
    #[test]
    fn marks_a_typo_and_a_phrase_the_same_way() {
        let _turn = checker();
        let lints = found("This is very unique and I recieve it.");
        let covering: Vec<String> = lints.iter().map(|(word, _)| word.clone()).collect();
        assert_eq!(covering, ["very unique", "recieve"]);
        for (word, offered) in lints {
            assert!(!offered.is_empty(), "nothing offered for {word}");
        }
    }

    #[test]
    fn leaves_alone_what_was_not_written_as_prose() {
        let _turn = checker();
        assert_eq!(covers("Call `recieve_this` now."), Vec::<String>::new());
        assert_eq!(covers("Read\n\n```\nrecieve\n```\n"), Vec::<String>::new());
        assert_eq!(covers("See [the exampel](http://a.test/pge)."), Vec::<String>::new());
        assert_eq!(
            covers("| Naem |\n| --- |\n| tpyo |\n"),
            Vec::<String>::new()
        );
        assert_eq!(covers("Mail me@exampel.com or see exampel.com now."), Vec::<String>::new());
        assert_eq!(covers("The snake_case_naem and the h1 and utf8."), Vec::<String>::new());
        assert_eq!(covers("Read ~/notes/thnig now."), Vec::<String>::new());
    }

    /// Only the link itself is left alone; the sentence it stands in is prose like any other.
    #[test]
    fn keeps_the_prose_a_link_stands_in() {
        let _turn = checker();
        assert_eq!(covers("A tpyo beside [a link](http://a.test/pge)."), ["tpyo"]);
    }

    #[test]
    fn keeps_the_full_stop_that_ends_a_sentence() {
        let _turn = checker();
        assert_eq!(covers("A tpyo. Another sentence."), ["tpyo"]);
    }

    #[test]
    fn counts_positions_the_way_qt_does() {
        let _turn = checker();
        // The emoji is two UTF-16 units, so the word after it starts at 3, not 2.
        let lints = check("🙂 recieve it.", true);
        assert_eq!(lints.len(), 1);
        assert_eq!((lints[0].at, lints[0].len), (3, 7));
    }

    /// Where a typo sits inside something the checker objects to as a whole, standing in
    /// the typo offers the typo: the narrower lint is the one that names those words.
    #[test]
    fn offers_the_narrowest_thing_the_cursor_is_standing_in() {
        let _turn = checker();
        let text = "This is very unique writing.";
        let (whole, _) = under_cursor(text, 8).expect("the phrase is found");
        assert_eq!(whole, "very unique");
    }

    /// The switch is the writer telling the checker to keep its opinions to itself:
    /// nothing is marked and nothing is offered until they ask for it again.
    #[test]
    fn says_nothing_at_all_while_the_checking_is_off() {
        let _turn = checker();
        set_checking(false);
        let quiet = covers("I recieve mail.");
        let offered = at("I recieve mail.", 4);
        set_checking(true);
        assert_eq!(quiet, Vec::<String>::new());
        assert!(offered.is_none());
        assert_eq!(covers("I recieve mail."), ["recieve"]);
    }

    /// The wash on the words is drawn from the other entry point, and the blocks are only
    /// told to look again by the generation moving — so turning the switch has to move it.
    #[test]
    fn takes_the_wash_off_the_words_as_well() {
        let _turn = checker();
        // What the highlighters ask. The first ask only schedules the work.
        while request_check("I recieve mail.", true).is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let generation = checker_generation();
        set_checking(false);
        let quiet = request_check("I recieve mail.", true);
        let moved = checker_generation() != generation;
        set_checking(true);
        assert!(quiet.is_empty(), "{} left marked", quiet.len());
        assert!(moved, "the blocks were never told to look again");
        assert!(!request_check("I recieve mail.", true).is_empty());
    }

    #[test]
    fn checks_until_it_is_told_not_to() {
        let _turn = checker();
        assert!(checking());
    }

    #[test]
    fn says_nothing_where_there_is_nothing_wrong() {
        let _turn = checker();
        assert!(at("This sentence is fine.", 3).is_none());
        assert!(covers("This sentence is fine.").is_empty());
    }
}
