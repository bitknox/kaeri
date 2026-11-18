//! Shows and series kept always private on this device.
//!
//! Kept as the model's keys (`show:<id>`, `series:<work>`), which this does not read. Nothing
//! here emits a sync register: the list stays on this device, and in the profile that set it.

use crate::{Result, Store};

impl Store {
    /// Every title kept always private, by key.
    pub fn private_titles(&self) -> Result<Vec<String>> {
        self.with_personal(|c| {
            let mut statement = c.prepare("SELECT title FROM private_title ORDER BY title")?;
            let rows =
                statement.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Keep `title` always private, or with `on` false no longer.
    pub fn set_private_title(&self, title: &str, on: bool, at: i64) -> Result<()> {
        self.with_personal_tx(|c| {
            if on {
                c.execute(
                    "INSERT INTO private_title (title, updated_at) VALUES (?1, ?2)
                     ON CONFLICT(title) DO UPDATE SET updated_at = excluded.updated_at",
                    rusqlite::params![title, at],
                )?;
            } else {
                c.execute("DELETE FROM private_title WHERE title = ?1", [title])?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Store;

    #[test]
    fn a_title_is_kept_and_let_go_without_a_sync_register() {
        let store = Store::open_in_memory().unwrap();
        store.set_private_title("show:4242", true, 1).unwrap();
        store.set_private_title("series:a:30001", true, 2).unwrap();
        store.set_private_title("show:4242", true, 3).unwrap();
        assert_eq!(store.private_titles().unwrap(), ["series:a:30001", "show:4242"]);
        store.set_private_title("show:4242", false, 4).unwrap();
        assert_eq!(store.private_titles().unwrap(), ["series:a:30001"]);
        let ops: i64 = store
            .with_personal(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM sync_op", [], |r| r.get(0))?)
            })
            .unwrap();
        let regs: i64 = store
            .with_personal(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM sync_reg", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!((ops, regs), (0, 0), "the flag never reaches sync");
    }
}
