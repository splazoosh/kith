//! App state and the single sync/async seam.
//!
//! [`Store`] is synchronous and `Clone` (its r2d2 pool is `Arc` internally, so
//! cloning is cheap); Tauri commands are `async`. [`AppState::with_store`] is
//! the *only* place the two meet, and it is the whole sync/async correctness of
//! the crate: it **locks → clones the `Store` → drops the guard → `spawn_blocking`**,
//! so no `MutexGuard` ever crosses an `.await` (`async-no-lock-await` /
//! `anti-lock-across-await`) and no SQLite call runs on the UI thread
//! (`async-spawn-blocking`).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

use kith_core::prelude::{Deletion, Store};

use crate::error::CommandError;

/// The undo stack's hard cap — a runaway backstop, not a product limit. Past it,
/// the oldest deletion is dropped.
const UNDO_CAP: usize = 50;

/// The managed application state: the open database (if any), the path it was
/// opened from, and the in-session undo stack. `std::sync::Mutex` (not
/// `tokio::sync::Mutex`) is correct here precisely because no guard is ever held
/// across an `.await` — the undo helpers lock, mutate, and drop the guard before
/// any `with_store` await.
#[derive(Default)]
pub struct AppState {
    /// The open store, or `None` when no database is open.
    pub db: Mutex<Option<Store>>,
    /// The path of the open database, mirrored for the "what's open" header and
    /// restart persistence.
    pub last_path: Mutex<Option<PathBuf>>,
    /// The session undo stack: each delete pushes the [`Deletion`] it captured;
    /// `undo` pops + restores. Bounded by [`UNDO_CAP`] and **cleared whenever the
    /// open database changes** (a snapshot is only valid against its own database).
    undo: Mutex<VecDeque<Deletion>>,
}

impl AppState {
    /// Pushes a captured [`Deletion`] onto the undo stack, dropping the oldest
    /// entry past [`UNDO_CAP`].
    pub fn push_undo(&self, deletion: Deletion) {
        let mut stack = self.undo.lock().expect("AppState.undo mutex poisoned");
        stack.push_back(deletion);
        while stack.len() > UNDO_CAP {
            stack.pop_front();
        }
    }

    /// Pops the most recent [`Deletion`], or `None` if the stack is empty.
    pub fn pop_undo(&self) -> Option<Deletion> {
        self.undo
            .lock()
            .expect("AppState.undo mutex poisoned")
            .pop_back()
    }

    /// Empties the undo stack — called whenever the open database changes
    /// (create / open / close / import), since a snapshot only restores into the
    /// database it came from.
    pub fn clear_undo(&self) {
        self.undo
            .lock()
            .expect("AppState.undo mutex poisoned")
            .clear();
    }

    /// The number of deletions currently on the stack (the `remaining` the undo
    /// command reports so the UI stays in sync).
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.undo
            .lock()
            .expect("AppState.undo mutex poisoned")
            .len()
    }
}

impl AppState {
    /// Runs a synchronous [`Store`] operation off the UI thread.
    ///
    /// Locks the state only long enough to **clone** the open `Store`, drops the
    /// guard, then runs `f` on a blocking thread. Core errors are mapped through
    /// the single [`From<CoreError>`](CommandError) point.
    ///
    /// # Errors
    /// [`CommandError::no_database`] (`io`) if no database is open; the mapped
    /// [`CoreError`](kith_core::prelude::CoreError) if `f` fails; or
    /// [`CommandError::unexpected`] if the blocking worker panics.
    pub async fn with_store<T, F>(&self, f: F) -> Result<T, CommandError>
    where
        F: FnOnce(Store) -> kith_core::prelude::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let store = {
            // Invariant: the lock body only clones an `Option<Store>` and never
            // panics, so the mutex cannot be poisoned (`err-expect-bugs-only`).
            let guard = self.db.lock().expect("AppState.db mutex poisoned");
            guard.clone().ok_or_else(CommandError::no_database)?
        }; // guard dropped here — before any await
        tauri::async_runtime::spawn_blocking(move || f(store))
            .await
            .map_err(|e| CommandError::unexpected(format!("background task failed: {e}")))?
            .map_err(CommandError::from)
    }
}
