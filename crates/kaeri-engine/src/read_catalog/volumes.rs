//! A work's chapter rows into volumes, one rule for every list that groups them: the series
//! page, the reader's chapter sheet and a series read from files.
//!
//! **Show volumes is off by default.** Sources disagree on volumes too often: one tags none,
//! another tags some, a group tags its own releases only. With the setting off the list comes
//! as one group in reading order, marked [`VolumeGroup::flat`], which a front end draws with no
//! heading. Each row still says its volume, so a download or a send by volume and the OPDS feed
//! work either way.
//!
//! **A row's volume.** A row without a volume takes the one any of the work's sources or groups
//! lists for the same chapter ([`Known`]). Rows still without one between two rows of one
//! volume take that volume, and a run of at most [`STRAY`] rows between two volumes (an extra
//! after a volume's last chapter) takes the volume before it. Rows another source lists that the
//! list keeps out ([`super::extras`]) count here as if they were listed, then go.
//!
//! **When volumes show.** With the setting on, rows are grouped by volume when at least half of
//! them have one ([`SHOWN_FROM`]) and every row without one comes after the last volume's: those
//! are the chapters not in a volume yet, in a group of their own after the volumes. Otherwise the
//! volumes are too patchy to group by, and the list is flat as with the setting off. Groups are
//! in reading order, so a front end that lists newest first, reversing them, shows "not in a
//! volume yet" at the top only when it holds the newest chapters.

use std::collections::HashMap;

use kaeri_model::reading::{ChapterReadState, ChapterRow, VolumeGroup};
use kaeri_store::Store;

/// The most rows without a volume, between two volumes, that join the volume before them.
pub const STRAY: usize = 2;

/// The share of rows that must have a volume for the list to be grouped by volume: one in two.
pub const SHOWN_FROM: (usize, usize) = (1, 2);

/// The volume each chapter of a work is listed in by any of its sources or groups: the one most
/// of them give.
#[derive(Debug, Clone, Default)]
pub struct Known(HashMap<String, String>);

impl Known {
    /// From every kept list of a work.
    pub fn of_work(store: &Store, work: &str) -> Self {
        let lists = super::chapters::lists_of(store, work);
        let mut counts: HashMap<&str, HashMap<&str, usize>> = HashMap::new();
        for (_, _, list) in &lists {
            for k in list {
                // A work whose numbering restarts per volume keys its volume in the id.
                if k.id.contains('/') {
                    continue;
                }
                if let Some(volume) = k.volume.as_deref().filter(|v| !v.trim().is_empty()) {
                    *counts.entry(&k.id).or_default().entry(volume).or_default() += 1;
                }
            }
        }
        Self(
            counts
                .into_iter()
                .filter_map(|(id, volumes)| {
                    let best = volumes
                        .into_iter()
                        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))?;
                    Some((id.to_owned(), best.0.to_owned()))
                })
                .collect(),
        )
    }

    /// Give the rows without a volume the one they have elsewhere, then fill the gaps between
    /// volumes as the module says.
    pub fn fill(&self, rows: &mut [ChapterRow]) {
        for row in rows.iter_mut().filter(|r| r.volume.is_none()) {
            row.volume = self.0.get(row.id.as_str()).cloned();
        }
        let mut i = 0;
        while i < rows.len() {
            if rows[i].volume.is_some() {
                i += 1;
                continue;
            }
            let start = i;
            while i < rows.len() && rows[i].volume.is_none() {
                i += 1;
            }
            let before = start.checked_sub(1).and_then(|b| rows[b].volume.clone());
            let after = rows.get(i).and_then(|r| r.volume.clone());
            let fill = match (before, after) {
                (Some(b), Some(a)) if b == a => Some(b),
                (Some(b), Some(_)) if i - start <= STRAY => Some(b),
                _ => None,
            };
            if let Some(volume) = fill {
                for row in &mut rows[start..i] {
                    row.volume = Some(volume.clone());
                }
            }
        }
    }
}

/// Whether rows with their volumes filled are grouped by volume, as the module says.
fn fits(rows: &[ChapterRow]) -> bool {
    let with = rows.iter().filter(|r| r.volume.is_some()).count();
    if with == 0 || with * SHOWN_FROM.1 < rows.len() * SHOWN_FROM.0 {
        return false;
    }
    let last = rows.iter().rposition(|r| r.volume.is_some()).unwrap_or(0);
    rows[..last].iter().all(|r| r.volume.is_some())
}

/// As [`group`], with the volumes of rows kept out of the list (`beside`) filling the gaps
/// beside them first.
pub fn group_beside(
    rows: Vec<ChapterRow>,
    beside: Vec<ChapterRow>,
    known: &Known,
    show: bool,
) -> Vec<VolumeGroup> {
    if beside.is_empty() || rows.is_empty() {
        return group(rows, known, show);
    }
    let range = |r: &ChapterRow| r.id.key().and_then(|k| k.range());
    let mut all: Vec<(bool, ChapterRow)> = rows.into_iter().map(|r| (true, r)).collect();
    for row in beside {
        let Some(mine) = range(&row) else { continue };
        let at = all
            .iter()
            .position(|(_, r)| range(r).is_some_and(|theirs| theirs > mine))
            .unwrap_or(all.len());
        all.insert(at, (false, row));
    }
    let (kept, mut rows): (Vec<bool>, Vec<ChapterRow>) = all.into_iter().unzip();
    known.fill(&mut rows);
    let rows = rows.into_iter().zip(kept).filter_map(|(r, kept)| kept.then_some(r)).collect();
    group(rows, known, show)
}

/// Rows in reading order into the groups a list shows, with `show` the Show volumes setting.
pub fn group(mut rows: Vec<ChapterRow>, known: &Known, show: bool) -> Vec<VolumeGroup> {
    if rows.is_empty() {
        return Vec::new();
    }
    known.fill(&mut rows);
    let mut groups: Vec<VolumeGroup> = Vec::new();
    if show && fits(&rows) {
        for row in rows {
            match groups.iter_mut().find(|g| g.volume == row.volume) {
                Some(group) => group.chapters.push(row),
                None => groups.push(VolumeGroup {
                    volume: row.volume.clone(),
                    title: None,
                    chapters: vec![row],
                    all_read: false,
                    flat: false,
                }),
            }
        }
    } else {
        groups.push(VolumeGroup {
            volume: None,
            title: None,
            chapters: rows,
            all_read: false,
            flat: true,
        });
    }
    for group in &mut groups {
        group.all_read =
            group.chapters.iter().all(|c| matches!(c.state, ChapterReadState::Read));
    }
    groups
}

#[cfg(test)]
mod tests {
    use kaeri_core::books::ChapterId;

    use super::*;

    fn row(id: &str, volume: Option<&str>) -> ChapterRow {
        let kept = super::super::chapters::Kept {
            id: id.into(),
            source_ref: id.into(),
            label: id.into(),
            number: Some(id.into()),
            volume: volume.map(str::to_owned),
            title: None,
            version_label: None,
            language: None,
            published: None,
            published_day: None,
            pages: None,
            seen_at: None,
            part: None,
        };
        super::super::updates::row_of(&kept, &Default::default())
    }

    fn shape(groups: &[VolumeGroup]) -> Vec<(Option<&str>, Vec<&str>, bool)> {
        groups
            .iter()
            .map(|g| {
                let ids = g.chapters.iter().map(|c| c.id.as_str()).collect();
                (g.volume.as_deref(), ids, g.flat)
            })
            .collect()
    }

    #[test]
    fn volumes_show_only_when_asked_and_whole_enough() {
        let rows = || {
            vec![
                row("1", Some("1")),
                row("2", None),
                row("3", Some("1")),
                row("3.5", None),
                row("4", Some("2")),
                row("5", None),
            ]
        };
        // Off: one flat group, each row with its volume filled all the same.
        let off = group(rows(), &Known::default(), false);
        assert_eq!(shape(&off), vec![(None, vec!["1", "2", "3", "3.5", "4", "5"], true)]);
        let volumes: Vec<Option<&str>> =
            off[0].chapters.iter().map(|c| c.volume.as_deref()).collect();
        assert_eq!(volumes, vec![Some("1"), Some("1"), Some("1"), Some("1"), Some("2"), None]);
        // On: 2 lies inside volume 1, the extra 3.5 after its last chapter joins it, and 5 is
        // after the last volume.
        let on = group(rows(), &Known::default(), true);
        assert_eq!(
            shape(&on),
            vec![
                (Some("1"), vec!["1", "2", "3", "3.5"], false),
                (Some("2"), vec!["4"], false),
                (None, vec!["5"], false),
            ]
        );
    }

    #[test]
    fn a_volume_another_source_gives_fills_the_row_and_patchy_volumes_stay_flat() {
        let rows: Vec<ChapterRow> = (1..=10).map(|n| row(&n.to_string(), None)).collect();
        let known = Known(
            [("1", "1"), ("2", "1"), ("9", "4")].map(|(c, v)| (c.into(), v.into())).into(),
        );
        let groups = group(rows.clone(), &known, true);
        assert_eq!(groups.len(), 1, "three of ten chapters have a volume");
        assert!(groups[0].flat);
        // Six of ten have one, but chapters 3 to 5 sit between volume 1 and volume 4 with
        // none: more than a stray extra, so still flat.
        let mut most = rows;
        for (i, row) in most.iter_mut().enumerate() {
            row.volume = match i {
                0 | 1 => Some("1".into()),
                5..=8 => Some("4".into()),
                _ => None,
            };
        }
        assert!(group(most.clone(), &Known::default(), true)[0].flat);
        // With the gap closed it groups, the chapter after the last volume last.
        for row in &mut most[2..5] {
            row.volume = Some("2".into());
        }
        let groups = group(most, &Known::default(), true);
        let volumes: Vec<Option<&str>> = groups.iter().map(|g| g.volume.as_deref()).collect();
        assert_eq!(volumes, vec![Some("1"), Some("2"), Some("4"), None]);
        assert_eq!(groups[3].chapters[0].id, ChapterId::new("10"));
    }

    /// Rows kept out of the list, such as another source's extras, still lend their volumes to
    /// the rows beside them, so the list stays grouped.
    #[test]
    fn rows_kept_out_still_lend_their_volumes() {
        let rows = || {
            vec![
                row("28", Some("3")),
                row("28.5", Some("3")),
                row("29", None),
                row("29.1", None),
                row("30", None),
                row("30.1", None),
                row("31", Some("4")),
            ]
        };
        let held = vec![row("29.5", Some("4")), row("30.5", Some("4"))];
        assert!(group(rows(), &Known::default(), true)[0].flat, "four rows with none");
        let groups = group_beside(rows(), held, &Known::default(), true);
        assert_eq!(
            shape(&groups),
            vec![
                (Some("3"), vec!["28", "28.5", "29", "29.1"], false),
                (Some("4"), vec!["30", "30.1", "31"], false),
            ]
        );
    }
}
