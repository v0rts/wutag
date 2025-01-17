#![allow(dead_code)]

use wutag_core::tag::Tag;

use anyhow::{Context, Result};
use colored::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct EntryData {
    path: PathBuf,
}

impl EntryData {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub type EntryId = usize;

#[derive(Default, Deserialize, Serialize)]
pub struct TagRegistry {
    tags: HashMap<Tag, Vec<EntryId>>,
    entries: HashMap<EntryId, EntryData>,
    path: PathBuf,
}

impl TagRegistry {
    /// Creates a new instance of `TagRegistry` with a `path` without loading it.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    /// Loads a registry from the specified `path`.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let data = fs::read(path).context("failed to read saved registry")?;

        serde_cbor::from_slice(&data).context("failed to deserialize tag registry")
    }

    /// Saves the registry serialized to the path from which it was loaded.
    pub fn save(&self) -> Result<()> {
        let serialized = serde_cbor::to_vec(&self).context("failed to serialize tag registry")?;
        fs::write(&self.path, &serialized).context("failed to save registry")
    }

    /// Clears this tag registry by removing all entries and tags.
    pub fn clear(&mut self) {
        self.tags.clear();
        self.entries.clear();
    }

    /// Updates the entry or adds it if it is not present.
    pub fn add_or_update_entry(&mut self, entry: EntryData) -> EntryId {
        let pos = self
            .list_entries_and_ids()
            .find(|(_, e)| **e == entry)
            .map(|(idx, _)| *idx);

        let pos = if let Some(pos) = pos {
            let e = self.entries.get_mut(&pos).expect("entry");
            *e = entry;
            pos
        } else {
            let timestamp = chrono::Utc::now().timestamp_nanos();
            let timestamp = if timestamp < 0 {
                timestamp.abs() as usize
            } else {
                timestamp as usize
            };
            self.entries.insert(timestamp, entry);
            timestamp
        };

        pos
    }

    fn mut_tag_entries(&mut self, tag: &Tag) -> &mut Vec<EntryId> {
        let exists = self.tags.iter().find(|(t, _)| t == &tag);

        if exists.is_none() {
            self.tags.insert(tag.clone(), Vec::new());
        }

        self.tags.get_mut(tag).unwrap()
    }

    /// Adds the `tag` to an entry with `entry` id. Returns the id if the entry was already tagged
    /// or `None` if the tag was added.
    pub fn tag_entry(&mut self, tag: &Tag, entry: EntryId) -> Option<EntryId> {
        let entries = self.mut_tag_entries(tag);

        if let Some(entry) = entries.iter().find(|&e| *e == entry) {
            return Some(*entry);
        }
        entries.push(entry);

        None
    }

    fn clean_tag_if_no_entries(&mut self, tag: &Tag) {
        let remove = if let Some(entries) = self.tags.get(tag) {
            entries.is_empty()
        } else {
            false
        };

        if remove {
            self.tags.remove(tag);
        }
    }

    /// Removes the `tag` from an entry with `entry` id. Returns the entry data if it has no tags
    /// left or `None` otherwise.
    pub fn untag_entry(&mut self, tag: &Tag, entry: EntryId) -> Option<EntryData> {
        let entries = self.mut_tag_entries(tag);

        if let Some(pos) = entries.iter().position(|e| *e == entry) {
            let entry = entries.remove(pos);

            self.clean_tag_if_no_entries(tag);

            if self.list_entry_tags(entry).is_none() {
                return self.entries.remove(&entry);
            }
        }

        None
    }

    /// Removes the tag with the `tag_name` from the `entry` returning the entry if it has no tags
    /// left or `None` otherwise.
    pub fn untag_by_name(&mut self, tag_name: &str, entry: EntryId) -> Option<EntryData> {
        let tag = self.get_tag(tag_name)?.to_owned();
        self.untag_entry(&tag, entry)
    }

    /// Clears all tags of the `entry`.
    pub fn clear_entry(&mut self, entry: EntryId) {
        let mut to_remove = vec![];
        self.tags.iter_mut().for_each(|(tag, entries)| {
            if let Some(idx) = entries.iter().copied().position(|e| e == entry) {
                entries.remove(idx);
            }
            if entries.is_empty() {
                to_remove.push(tag.to_owned());
            }
        });

        for tag in to_remove {
            self.tags.remove(&tag);
        }

        self.entries.remove(&entry);
    }

    /// Finds the entry by a `path`. Returns the id of the entry if found.
    pub fn find_entry<P: AsRef<Path>>(&self, path: P) -> Option<EntryId> {
        self.entries
            .iter()
            .find(|(_, entry)| entry.path == path.as_ref())
            .map(|(idx, _)| *idx)
    }

    /// Lists tags of the `entry` if such entry exists.
    pub fn list_entry_tags(&self, entry: EntryId) -> Option<Vec<&Tag>> {
        let tags = self
            .tags
            .iter()
            .fold(Vec::new(), |mut acc, (tag, entries)| {
                if entries.iter().any(|id| entry == *id) {
                    acc.push(tag);
                }
                acc
            });

        if tags.is_empty() {
            None
        } else {
            Some(tags)
        }
    }

    /// Returns entries that have all of the `tags`.
    pub fn list_entries_with_tags<T, S>(&self, tags: T) -> Vec<EntryId>
    where
        T: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut entries = tags.into_iter().fold(Vec::new(), |mut acc, tag| {
            if let Some(entries) = self
                .tags
                .iter()
                .find(|(t, _)| t.name() == tag.as_ref())
                .map(|(_, e)| e)
            {
                acc.extend_from_slice(&entries[..]);
            }
            acc
        });

        entries.dedup();

        entries
    }

    /// Lists ids of all entries present in the registry.
    pub fn list_entries_ids(&self) -> impl Iterator<Item = &EntryId> {
        self.entries.keys()
    }

    /// Lists data of all entries present in the registry.
    pub fn list_entries(&self) -> impl Iterator<Item = &EntryData> {
        self.entries.values()
    }

    /// Lists ids and data of all entries present in the registry.
    pub fn list_entries_and_ids(&self) -> impl Iterator<Item = (&EntryId, &EntryData)> {
        self.entries.iter()
    }

    /// Lists available tags.
    pub fn list_tags(&self) -> impl Iterator<Item = &Tag> {
        self.tags.keys()
    }

    /// Returns data of the entry with `id` if such entry exists.
    pub fn get_entry(&self, id: EntryId) -> Option<&EntryData> {
        self.entries.get(&id)
    }

    /// Returns the tag with the name `tag` if it exists.
    pub fn get_tag<T: AsRef<str>>(&self, tag: T) -> Option<&Tag> {
        self.tags.keys().find(|t| t.name() == tag.as_ref())
    }

    /// Updates the color of the `tag`. Returns `true` if the tag was found and updated and `false`
    /// otherwise.
    pub fn update_tag_color<T: AsRef<str>>(&mut self, tag: T, color: Color) -> bool {
        if let Some(mut t) = self.tags.keys().find(|t| t.name() == tag.as_ref()).cloned() {
            let data = self.tags.remove(&t).expect("removed tag");
            t.set_color(&color);
            self.tags.insert(t, data);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_COLORS;
    use colored::Color::{Black, Red};

    #[test]
    fn adds_and_tags_entry() {
        let path = PathBuf::from("/tmp");
        let entry = EntryData::new(path.clone());
        let mut registry = TagRegistry::default();
        registry.add_or_update_entry(entry.clone());
        let id = registry.find_entry(&path).unwrap();

        let _entry = registry.get_entry(id).unwrap();
        assert_eq!(_entry.path, entry.path);

        let tag = Tag::random("test", DEFAULT_COLORS);
        let second = Tag::random("second", DEFAULT_COLORS);

        assert_eq!(registry.tag_entry(&tag, id), None);
        assert_eq!(registry.list_entry_tags(id), Some(vec![&tag]));
        assert_eq!(registry.tag_entry(&second, id), None);
        assert!(registry.list_entry_tags(id).unwrap().contains(&&tag));
        assert!(registry.list_entry_tags(id).unwrap().contains(&&second));
        assert_eq!(registry.untag_entry(&tag, id), None);
        assert_eq!(registry.list_entry_tags(id), Some(vec![&second]));
        assert_eq!(registry.untag_entry(&tag, id), None);
        assert_eq!(registry.untag_entry(&second, id), Some(entry));
        assert_eq!(registry.list_entry_tags(id), None);
    }

    #[test]
    fn adds_multiple_entries() {
        let mut registry = TagRegistry::default();

        let entry = EntryData::new("/tmp");
        let fst_id = registry.add_or_update_entry(entry.clone());
        let snd_entry = EntryData::new("/tmp/123");
        let snd_id = registry.add_or_update_entry(snd_entry.clone());

        assert_eq!(registry.list_entries().count(), 2);

        let entries: Vec<_> = registry.list_entries_and_ids().collect();
        assert!(entries.contains(&(&fst_id, &entry)));
        assert!(entries.contains(&(&snd_id, &snd_entry)));
    }

    #[test]
    fn updates_tag_color() {
        let entry = EntryData::new("/tmp");

        let mut registry = TagRegistry::default();
        let id = registry.add_or_update_entry(entry);

        let tag = Tag::new("test", Black);

        assert!(registry.tag_entry(&tag, id).is_none());
        assert!(registry.update_tag_color("test", Red));
        assert_eq!(registry.list_tags().next().unwrap().color(), &Red);
    }

    #[test]
    fn removes_an_entry_when_no_tags_left() {
        let entry = EntryData::new("/tmp");

        let mut registry = TagRegistry::default();
        let id = registry.add_or_update_entry(entry.clone());

        let tag1 = Tag::new("test", Black);
        let tag2 = Tag::new("test2", Red);

        assert!(registry.tag_entry(&tag1, id).is_none());
        assert_eq!(registry.tags.iter().next(), Some((&tag1, &vec![id])));
        assert_eq!(registry.list_entries().count(), 1);
        assert_eq!(registry.untag_entry(&tag1, id), Some(entry.clone()));
        assert_eq!(registry.list_entries().count(), 0);
        assert!(registry.tags.is_empty());

        let id = registry.add_or_update_entry(entry.clone());
        assert!(registry.tag_entry(&tag2, id).is_none());
        assert_eq!(registry.tags.iter().next(), Some((&tag2, &vec![id])));
        assert_eq!(registry.list_entries().count(), 1);
        assert_eq!(registry.untag_by_name(tag2.name(), id), Some(entry.clone()));
        assert_eq!(registry.list_entries().count(), 0);
        assert!(registry.tags.is_empty());

        let id = registry.add_or_update_entry(entry);
        assert!(registry.tag_entry(&tag1, id).is_none());
        assert!(registry.tag_entry(&tag2, id).is_none());
        let tags: Vec<_> = registry.tags.iter().collect();
        assert!(tags.contains(&(&tag1, &vec![id])));
        assert!(tags.contains(&(&tag2, &vec![id])));
        assert_eq!(registry.list_entries().count(), 1);
        registry.clear_entry(id);
        assert_eq!(registry.list_entries().count(), 0);
        assert!(registry.tags.is_empty());
    }

    #[test]
    fn lists_entry_tags() {
        let mut registry = TagRegistry::default();

        let tag1 = Tag::new("src", Black);
        let tag2 = Tag::new("code", Red);

        let entry = EntryData::new("/tmp");

        let id = registry.add_or_update_entry(entry);
        registry.tag_entry(&tag1, id);
        registry.tag_entry(&tag2, id);

        let tags = registry.list_entry_tags(id).unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&&tag1));
        assert!(tags.contains(&&tag2));
    }

    #[test]
    fn lists_entries_with_tags() {
        let mut registry = TagRegistry::default();

        let tag1 = Tag::new("src", Black);
        let tag2 = Tag::new("code", Red);

        let entry = EntryData::new("/tmp");
        let entry1 = EntryData::new("/tmp/1");
        let entry2 = EntryData::new("/tmp/2");
        let entry3 = EntryData::new("/tmp/3");

        let id = registry.add_or_update_entry(entry);
        let id1 = registry.add_or_update_entry(entry1);
        let id2 = registry.add_or_update_entry(entry2);
        let id3 = registry.add_or_update_entry(entry3);

        registry.tag_entry(&tag1, id);
        registry.tag_entry(&tag1, id2);

        registry.tag_entry(&tag2, id1);
        registry.tag_entry(&tag2, id3);

        let entries1 = registry.list_entries_with_tags(vec![tag1.name()]);
        assert_eq!(entries1.len(), 2);
        assert!(entries1.contains(&id));
        assert!(entries1.contains(&id2));

        let entries2 = registry.list_entries_with_tags(vec![tag2.name()]);
        assert_eq!(entries2.len(), 2);
        assert!(entries2.contains(&id1));
        assert!(entries2.contains(&id3));

        let entries = registry.list_entries_with_tags(vec![tag2.name(), tag1.name()]);
        assert_eq!(entries.len(), 4);
        assert!(entries.contains(&id));
        assert!(entries.contains(&id1));
        assert!(entries.contains(&id2));
        assert!(entries.contains(&id3));
    }

    #[test]
    fn saves_and_loads() {
        let tmp_dir = tempdir::TempDir::new("registry-test").unwrap();
        let registry_path = tmp_dir.path().join("wutag.registry");

        let mut registry = TagRegistry::new(&registry_path);

        let tag = Tag::new("src", Black);
        let entry = EntryData::new("/tmp");

        let id = registry.add_or_update_entry(entry.clone());
        registry.tag_entry(&tag, id);

        registry.save().unwrap();

        let registry = TagRegistry::load(registry_path).unwrap();
        let mut entries = registry.list_entries_and_ids();
        let (got_id, got_entry) = entries.next().unwrap();
        assert!(entries.next().is_none());
        assert_eq!(got_id, &id);
        assert_eq!(got_entry, &entry);
        assert_eq!(registry.list_entries_with_tags(vec![tag.name()]), vec![id]);
    }
}
