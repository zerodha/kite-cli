mod buffer;
mod pad;

use crate::tui::format;
use crate::util;
pub use buffer::{NoteBuffer, cursor_row, editor_scroll, wrap};
use pad::{Guarded, load_guarded};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use pad::Persist;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NoteInstrument {
    #[serde(default)]
    pub exchange: String,
    #[serde(default)]
    pub tradingsymbol: String,
}

impl NoteInstrument {
    pub fn label(&self) -> String {
        crate::tui::format::instrument_label(&self.exchange, &self.tradingsymbol)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Note {
    pub id: u64,
    pub body: String,
    pub created: u64,
    pub updated: u64,
    pub instrument: Option<NoteInstrument>,
}

impl Note {
    pub fn instrument_label(&self) -> String {
        self.instrument
            .as_ref()
            .map_or_else(|| GENERAL.to_string(), NoteInstrument::label)
    }

    pub fn preview(&self) -> String {
        self.body
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("")
            .to_string()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Notes {
    #[serde(default)]
    notes: Vec<Note>,
}

impl Notes {
    pub fn from_vec(notes: Vec<Note>) -> Self {
        Self { notes }.normalized()
    }

    fn normalized(mut self) -> Self {
        let mut used: Vec<u64> = self.notes.iter().map(|note| note.id).collect();
        let mut seen: Vec<u64> = Vec::with_capacity(self.notes.len());
        for note in &mut self.notes {
            if note.id == 0 || seen.contains(&note.id) {
                note.id = unused_id(&used);
                used.push(note.id);
            }
            seen.push(note.id);
        }
        self
    }

    fn next_id(&self) -> u64 {
        let used: Vec<u64> = self.notes.iter().map(|note| note.id).collect();
        unused_id(&used)
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    pub fn get(&self, id: u64) -> Option<&Note> {
        self.notes.iter().find(|note| note.id == id)
    }

    pub fn newest_first(&self) -> Vec<&Note> {
        let mut ordered: Vec<&Note> = self.notes.iter().collect();
        ordered.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
        ordered
    }

    pub fn ids_newest_first(&self) -> Vec<u64> {
        self.newest_first().iter().map(|note| note.id).collect()
    }

    pub fn upsert(
        &mut self,
        id: Option<u64>,
        body: &str,
        instrument: Option<NoteInstrument>,
        now: u64,
    ) -> Option<u64> {
        let blank = body.trim().is_empty();
        if let Some(id) = id {
            let pos = self.notes.iter().position(|note| note.id == id)?;
            if blank {
                self.notes.remove(pos);
                return None;
            }
            let note = &mut self.notes[pos];
            if note.body != body || note.instrument != instrument {
                note.body = body.to_string();
                note.instrument = instrument;
                note.updated = now;
            }
            Some(id)
        } else {
            if blank {
                return None;
            }
            let id = self.next_id();
            self.notes.push(Note {
                id,
                body: body.to_string(),
                created: now,
                updated: now,
                instrument,
            });
            Some(id)
        }
    }

    pub fn remove(&mut self, id: u64) -> Option<Note> {
        let pos = self.notes.iter().position(|note| note.id == id)?;
        Some(self.notes.remove(pos))
    }
}

fn unused_id(used: &[u64]) -> u64 {
    let highest = used.iter().copied().max().unwrap_or(0);
    highest.checked_add(1).unwrap_or_else(|| {
        (1..=u64::MAX)
            .find(|id| !used.contains(id))
            .expect("a Vec cannot hold u64::MAX notes")
    })
}

pub fn from_json(raw: &str) -> Option<Notes> {
    let raw = raw.trim_start_matches('\u{feff}');
    if raw.trim().is_empty() {
        return Some(Notes::default());
    }
    if let Ok(notes) = serde_json::from_str::<Notes>(raw) {
        return Some(notes.normalized());
    }
    if let Ok(notes) = serde_json::from_str::<Vec<Note>>(raw) {
        return Some(Notes::from_vec(notes));
    }
    None
}

pub type Pad = Guarded<Notes>;

impl Pad {
    pub fn persist(&mut self, path: &Path) -> Persist {
        self.persist_with(|notes| save_to(path, notes))
    }
}

pub fn config_file_path() -> Option<PathBuf> {
    let mut path = dirs::config_dir()?;
    path.push("kite");
    path.push("notes.json");
    Some(path)
}

pub fn load_from(path: &Path) -> Pad {
    load_guarded(path, from_json)
}

pub fn load() -> Pad {
    match config_file_path() {
        Some(path) => load_from(&path),
        None => Pad::default(),
    }
}

pub fn save_to(path: &Path, notes: &Notes) -> Result<(), String> {
    util::save_json_atomic(path, notes)
}

pub const BLOCKED_WARNING: &str = "notes.json could not be read, so nothing typed here is being saved, move or fix that file, then restart";

pub fn list_title(count: usize) -> String {
    format!("Notes / {count}")
}

pub fn list_subtitle(blocked: bool, attach: Option<&str>) -> String {
    if blocked {
        return "Local notes | nothing typed here is saved".to_string();
    }
    match attach {
        Some(label) => format!("Local notes | n attaches {label} | N creates a general note"),
        None => "Local notes | n creates a general note".to_string(),
    }
}

pub fn editor_title(label: &str) -> String {
    format!("Note / {label}")
}

pub fn editor_subtitle(stamps: Option<(u64, u64)>) -> String {
    match stamps {
        Some((created, updated)) => {
            format!("Written {} | edited {}", stamp(created), stamp(updated))
        }
        None => "New note | nothing on disk yet".to_string(),
    }
}

pub const GENERAL: &str = "General";

pub const STAMP_WIDTH: usize = 16;

pub const INSTRUMENT_WIDTH: usize = 20;

pub fn stamp(unix: u64) -> String {
    let (y, m, d) = format::date_ist(unix);
    let sod = (unix + 19_800) % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        sod / 3600,
        (sod % 3600) / 60
    )
}

pub fn row_columns(note: &Note, preview_width: usize) -> [String; 3] {
    [
        format::pad_or_clip(&stamp(note.created), STAMP_WIDTH),
        format::pad_or_clip(&note.instrument_label(), INSTRUMENT_WIDTH),
        format::pad_or_clip(&note.preview(), preview_width),
    ]
}

#[derive(Debug)]
pub struct NoteEditor {
    pub id: Option<u64>,
    pub instrument: Option<NoteInstrument>,
    pub buffer: NoteBuffer,
    pub stamps: Option<(u64, u64)>,
}

impl NoteEditor {
    pub fn draft(instrument: Option<NoteInstrument>) -> Self {
        Self {
            id: None,
            instrument,
            buffer: NoteBuffer::default(),
            stamps: None,
        }
    }

    pub fn editing(note: &Note) -> Self {
        Self {
            id: Some(note.id),
            instrument: note.instrument.clone(),
            buffer: NoteBuffer::from_text(&note.body),
            stamps: Some((note.created, note.updated)),
        }
    }

    pub fn label(&self) -> String {
        self.instrument
            .as_ref()
            .map_or_else(|| GENERAL.to_string(), NoteInstrument::label)
    }
}

#[derive(Debug, Default)]
pub struct NotesState {
    pub selected: usize,
    pub attach: Option<NoteInstrument>,
    pub editor: Option<NoteEditor>,
}

#[cfg(test)]
mod tests {
    use super::{
        Note, NoteInstrument, Notes, Pad, Persist, from_json, load_from, row_columns, save_to,
        stamp,
    };
    use std::path::{Path, PathBuf};

    fn parsed(raw: &str) -> Notes {
        from_json(raw).expect("input must parse")
    }

    fn note(id: u64, body: &str, created: u64) -> Note {
        Note {
            id,
            body: body.to_string(),
            created,
            updated: created,
            instrument: None,
        }
    }

    #[test]
    fn garbage_is_not_a_notes_file_at_all() {
        assert_eq!(from_json("not json at all"), None);
        assert_eq!(from_json(""), Some(Notes::default()));
        assert_eq!(
            from_json("{\n  \"notes\": [\n    {\n      \"id\": 1,\n      \"bo"),
            None
        );
        assert_eq!(from_json("{\"notes\":[]}"), Some(Notes::default()));
    }

    #[test]
    fn empty_object_yields_empty_notes() {
        assert!(parsed("{}").is_empty());
    }

    #[test]
    fn parses_bare_array_form() {
        let raw = r#"[{"id":3,"body":"watch 1520","created":10,"updated":10}]"#;
        let notes = parsed(raw);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes.get(3).unwrap().preview(), "watch 1520");
    }

    #[test]
    fn tolerates_missing_fields_and_repairs_ids() {
        let notes = parsed(r#"{"notes":[{"body":"thesis"},{"body":"other"}]}"#);
        assert_eq!(notes.len(), 2);
        let ids = notes.ids_newest_first();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        assert!(ids.iter().all(|id| *id != 0));
    }

    #[test]
    fn duplicate_ids_are_made_unique_so_a_delete_removes_one_note() {
        let raw = r#"{"notes":[
            {"id":1,"body":"first","created":1,"updated":1},
            {"id":1,"body":"second","created":2,"updated":2}
        ]}"#;
        let mut notes = parsed(raw);
        assert_eq!(notes.len(), 2);
        let ids = notes.ids_newest_first();
        assert_ne!(ids[0], ids[1]);
        notes.remove(ids[0]);
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn a_persisted_max_id_still_allocates_unused_ids() {
        let raw = format!(
            r#"{{"notes":[
            {{"id":{max},"body":"edge","created":1,"updated":1}},
            {{"body":"no id","created":2,"updated":2}}
        ]}}"#,
            max = u64::MAX
        );
        let mut notes = parsed(&raw);
        assert!(notes.get(u64::MAX).is_some(), "the persisted id is kept");
        let fresh = notes
            .upsert(None, "new", None, 3)
            .expect("a note is created");
        let ids = notes.ids_newest_first();
        assert_eq!(ids.len(), 3);
        assert!(ids.iter().all(|id| *id != 0));
        assert!(ids.contains(&fresh));
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 3, "every id is distinct: {ids:?}");
    }

    #[test]
    fn save_shaped_json_round_trips() {
        let mut notes = Notes::default();
        notes.upsert(None, "buy the dip", None, 100);
        notes.upsert(
            None,
            "₹1520 is the level",
            Some(NoteInstrument {
                exchange: "NSE".to_string(),
                tradingsymbol: "INFY".to_string(),
            }),
            200,
        );
        let raw = serde_json::to_string_pretty(&notes).unwrap();
        assert_eq!(parsed(&raw), notes);
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kite-notes-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn fingerprint(path: &Path) -> (String, std::time::SystemTime, u64) {
        let meta = std::fs::metadata(path).expect("file exists");
        #[cfg(unix)]
        let inode = std::os::unix::fs::MetadataExt::ino(&meta);
        #[cfg(not(unix))]
        let inode = 0;
        (
            std::fs::read_to_string(path).expect("readable"),
            meta.modified().expect("mtime"),
            inode,
        )
    }

    fn with_note(body: &str, created: u64) -> Notes {
        let mut notes = Notes::default();
        notes.upsert(None, body, None, created);
        notes
    }

    #[test]
    #[cfg(unix)]
    fn a_save_replaces_the_file_by_rename_rather_than_truncating_it() {
        let path = scratch("atomic").join("notes.json");
        save_to(&path, &with_note("first", 100)).expect("first save");
        let before = fingerprint(&path);
        save_to(&path, &with_note("second", 200)).expect("second save");
        let after = fingerprint(&path);
        assert_ne!(
            before.2, after.2,
            "the live notes.json must be replaced, never truncated and rewritten in place"
        );
        assert_eq!(load_from(&path).newest_first()[0].body, "second");
    }

    #[test]
    fn an_orphan_temp_file_does_not_interfere_with_saves() {
        let path = scratch("interrupted").join("notes.json");
        save_to(&path, &with_note("the only copy", 100)).expect("save");
        let before = fingerprint(&path);

        std::fs::write(
            path.with_file_name(".notes.json.0.0.tmp"),
            "{\n  \"notes\": [\n    {\n      \"bo",
        )
        .expect("partial temp");
        assert_eq!(
            fingerprint(&path),
            before,
            "the live file must be untouched"
        );

        let mut pad = load_from(&path);
        assert_eq!(pad.len(), 1, "the previous notes must still load");
        assert_eq!(pad.newest_first()[0].body, "the only copy");

        pad.upsert(None, "written after the crash", None, 200);
        assert_eq!(pad.persist(&path), Persist::Written);
        assert_eq!(load_from(&path).len(), 2);
    }

    #[test]
    fn an_unmodified_open_then_close_writes_nothing() {
        let path = scratch("unmodified").join("notes.json");
        save_to(&path, &with_note("a thesis", 100)).expect("save");
        let before = fingerprint(&path);

        let mut pad = load_from(&path);
        assert!(!pad.is_dirty());
        assert_eq!(pad.persist(&path), Persist::Skipped);
        assert_eq!(
            fingerprint(&path),
            before,
            "an unchanged pad rewrote the file"
        );

        pad.upsert(None, "a second thought", None, 200);
        assert!(pad.is_dirty());
        assert_eq!(pad.persist(&path), Persist::Written);
        assert_ne!(fingerprint(&path), before);
        assert_eq!(pad.persist(&path), Persist::Skipped);
    }

    #[test]
    fn a_corrupt_file_survives_an_open_then_close() {
        let path = scratch("corrupt").join("notes.json");
        std::fs::write(
            &path,
            "{\n  \"notes\": [\n    {\n      \"body\": \"half a th",
        )
        .expect("corrupt file");
        let before = fingerprint(&path);

        let mut pad = load_from(&path);
        assert!(pad.is_empty(), "an unreadable file loads as an empty pad");
        assert!(!pad.is_dirty(), "there is nothing it is allowed to write");
        assert_eq!(pad.persist(&path), Persist::Blocked);
        assert_eq!(pad.persist(&path), Persist::Skipped);
        assert_eq!(
            fingerprint(&path),
            before,
            "the unreadable file must be left exactly as it was"
        );

        pad.upsert(None, "typed into the empty-looking pad", None, 200);
        assert_eq!(pad.persist(&path), Persist::Skipped);
        assert_eq!(fingerprint(&path), before);
    }

    #[test]
    fn a_missing_file_is_a_first_run_and_saves_normally() {
        let path = scratch("firstrun").join("notes.json");
        let mut pad = load_from(&path);
        assert!(pad.is_empty());
        assert_eq!(pad.persist(&path), Persist::Skipped, "nothing to write");
        pad.upsert(None, "first ever note", None, 100);
        assert_eq!(pad.persist(&path), Persist::Written);
        assert_eq!(load_from(&path).len(), 1);
    }

    #[test]
    fn save_creates_the_config_directory() {
        let path = scratch("mkdir").join("nested").join("notes.json");
        save_to(&path, &with_note("x", 1)).expect("save creates parents");
        assert_eq!(load_from(&path).len(), 1);
    }

    #[test]
    fn a_delete_makes_the_pad_dirty() {
        let mut pad = Pad::loaded(with_note("doomed", 100));
        assert!(!pad.is_dirty());
        let id = pad.ids_newest_first()[0];
        assert!(pad.get(id).is_some());
        pad.remove(id);
        assert!(pad.is_dirty());
        assert!(pad.is_empty());
    }

    #[test]
    fn newest_first_orders_by_created_and_breaks_ties_deterministically() {
        let notes = Notes::from_vec(vec![
            note(1, "oldest", 10),
            note(2, "same second", 30),
            note(3, "same second", 30),
            note(4, "middle", 20),
        ]);
        assert_eq!(notes.ids_newest_first(), vec![3, 2, 4, 1]);
    }

    #[test]
    fn newest_first_is_stable_across_an_edit() {
        let mut notes = Notes::from_vec(vec![note(1, "a", 10), note(2, "b", 20)]);
        let before = notes.ids_newest_first();
        notes.upsert(Some(1), "a edited", None, 999);
        assert_eq!(notes.ids_newest_first(), before);
        assert_eq!(notes.get(1).unwrap().updated, 999);
        assert_eq!(notes.get(1).unwrap().created, 10);
    }

    #[test]
    fn upsert_assigns_an_id_unique_among_the_notes_that_exist() {
        let mut notes = Notes::default();
        let first = notes.upsert(None, "one", None, 1).unwrap();
        let second = notes.upsert(None, "two", None, 2).unwrap();
        let third = notes.upsert(None, "three", None, 3).unwrap();
        assert_ne!(first, second);
        assert_ne!(second, third);
        notes.remove(second);
        let fourth = notes.upsert(None, "four", None, 4).unwrap();
        assert_ne!(fourth, first);
        assert_ne!(fourth, third);
        assert_eq!(notes.len(), 3);
    }

    #[test]
    fn upsert_discards_a_blank_draft_and_deletes_a_blanked_note() {
        let mut notes = Notes::default();
        assert_eq!(notes.upsert(None, "   \n\n ", None, 1), None);
        assert!(notes.is_empty(), "a blank draft must leave no trace");

        let id = notes.upsert(None, "real", None, 1).unwrap();
        assert_eq!(notes.upsert(Some(id), "", None, 2), None);
        assert!(notes.is_empty(), "blanking a note deletes it");
    }

    #[test]
    fn upsert_of_an_unchanged_body_does_not_bump_updated() {
        let mut notes = Notes::default();
        let id = notes.upsert(None, "same", None, 100).unwrap();
        notes.upsert(Some(id), "same", None, 500);
        assert_eq!(notes.get(id).unwrap().updated, 100);
    }

    #[test]
    fn upsert_of_a_vanished_id_is_a_noop() {
        let mut notes = Notes::default();
        assert_eq!(notes.upsert(Some(42), "ghost", None, 1), None);
        assert!(notes.is_empty());
    }

    #[test]
    fn instrument_label_falls_back_for_general_notes_and_missing_exchange() {
        assert_eq!(note(1, "x", 0).instrument_label(), "General");
        let attached = Note {
            instrument: Some(NoteInstrument {
                exchange: String::new(),
                tradingsymbol: "token:408065".to_string(),
            }),
            ..note(1, "x", 0)
        };
        assert_eq!(attached.instrument_label(), "token:408065");
    }

    #[test]
    fn preview_is_the_first_line_with_content() {
        let n = note(1, "\n   \nthe real point\nmore", 0);
        assert_eq!(n.preview(), "the real point");
        assert_eq!(note(1, "   ", 0).preview(), "");
    }

    #[test]
    fn a_blocked_pad_offers_nothing_it_cannot_keep() {
        use super::{editor_subtitle, editor_title, list_subtitle, list_title};
        assert_eq!(list_title(3), "Notes / 3");
        let offer = list_subtitle(false, Some("NSE:INFY"));
        assert!(offer.contains("n attaches NSE:INFY"), "{offer}");
        assert!(offer.contains("N creates a general note"), "{offer}");
        assert_eq!(
            list_subtitle(false, None),
            "Local notes | n creates a general note"
        );
        let blocked = list_subtitle(true, Some("NSE:INFY"));
        assert!(blocked.contains("nothing typed here is saved"), "{blocked}");
        assert!(!blocked.contains("attaches"), "{blocked}");

        assert_eq!(editor_title("NSE:INFY"), "Note / NSE:INFY");
        assert_eq!(editor_title(super::GENERAL), "Note / General");
        let stamps = editor_subtitle(Some((0, 60)));
        assert!(
            stamps.starts_with("Written 1970-01-01 05:30 | edited "),
            "{stamps}"
        );
        assert_eq!(editor_subtitle(None), "New note | nothing on disk yet");
    }

    #[test]
    fn blocked_outlives_the_one_time_toast() {
        let mut pad = Pad::unreadable();
        assert!(pad.blocked());
        pad.upsert(None, "typed into a pad that cannot save", None, 100);
        let path = Path::new("nonexistent").join("notes.json");
        assert_eq!(pad.persist(&path), Persist::Blocked);
        assert_eq!(pad.persist(&path), Persist::Skipped);
        assert!(pad.blocked(), "the pad still refuses every save");
        assert!(!Pad::loaded(with_note("fine", 1)).blocked());
        assert!(!Pad::default().blocked());
    }

    #[test]
    fn stamp_renders_ist_minutes() {
        assert_eq!(stamp(1_755_000_000), "2025-08-12 17:30");
        assert_eq!(stamp(0), "1970-01-01 05:30");
    }

    #[test]
    fn row_columns_are_padded_to_exact_widths_so_nothing_bleeds() {
        let long = Note {
            instrument: Some(NoteInstrument {
                exchange: "NFO".to_string(),
                tradingsymbol: "MIDCPNIFTY26AUG13400CE".to_string(),
            }),
            ..note(
                1,
                "a very long opening line that will not fit",
                1_755_000_000,
            )
        };
        let [when, inst, preview] = row_columns(&long, 12);
        assert_eq!(when.chars().count(), super::STAMP_WIDTH);
        assert_eq!(inst.chars().count(), super::INSTRUMENT_WIDTH);
        assert_eq!(preview.chars().count(), 12);
        assert!(inst.ends_with('…'), "instrument label must clip: {inst:?}");
        assert!(preview.ends_with('…'), "preview must clip: {preview:?}");
        assert_eq!(row_columns(&long, 0)[2], "");
    }
    #[test]
    fn blank_and_bom_notes_are_readable_empty_pads() {
        for text in ["", " \n\t", "\u{feff}", "\u{feff}{\"notes\":[]}"] {
            assert_eq!(from_json(text), Some(Notes::default()));
        }
    }
}
