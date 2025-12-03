# Async State Management Implementation Plan

## Overview

Add async state management to the GitHub code search TUI to support real API calls, loading indicators, search history persistence, and automatic pagination.

## Current Architecture Issues

- Event loop uses blocking `event::read()` which prevents async operations
- Tokio runtime available but not integrated with UI
- Currently uses mock data from `resp.json`, never calls real API
- No mechanism for background tasks to communicate with render loop

## Solution: Message-Based Architecture with tokio::select!

### Core Pattern

Replace blocking event loop with non-blocking poll using `tokio::select!` to multiplex:
- Terminal events (keyboard input via `event::poll()`)
- Background task messages (API responses via `mpsc` channel)
- Periodic rendering ticks

### Message Types

```rust
enum AppMessage {
    SearchComplete { results: CodeResultsWithPagination, query: String },
    SearchError { error: String },
    PaginationComplete { results: CodeResultsWithPagination, page: u32 },
    PaginationError { error: String },
    HistoryLoaded { searches: Vec<String> },
}
```

### State Machine

```rust
enum SearchState {
    Idle,
    Loading { query: String },
    Loaded {
        query: String,
        results: CodeResults,
        pagination: Option<PaginationInfo>,
        current_page: u32,
    },
    LoadingMore {
        query: String,
        results: CodeResults,  // Keep visible during load
        pagination: Option<PaginationInfo>,
        current_page: u32,
    },
}

struct SearchHistory {
    searches: Vec<String>,
    selected_idx: Option<usize>,
}
```

## Implementation Steps

### 1. Create History Module (`src/history.rs`)

New file for search history persistence:
- `SearchHistory` struct with list and selected index
- `load_history()` - async load from `~/.config/ghs/history.json`
- `save_history()` - async save, deduplicate (move existing to top)
- Format: JSON array of strings
- Limit to last 100 searches

**Dependencies:** Add `dirs = "5.0"` to Cargo.toml for XDG paths

### 2. Add Message Infrastructure (`src/app.rs`)

- Define `AppMessage` enum at top of file
- Add `message_tx: UnboundedSender<AppMessage>` to `App` struct
- Create `tokio::sync::mpsc::unbounded_channel()` in `App::run()`

### 3. Refactor Event Loop (`src/app.rs`)

Replace blocking `event::read()` with:

```rust
let (message_tx, mut message_rx) = mpsc::unbounded_channel();

loop {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(16)) => {
            if event::poll(Duration::ZERO)? {
                let event = event::read()?;
                // Handle event
            }
        }
        Some(msg) = message_rx.recv() => {
            // Handle message
        }
    }
    // Render frame
}
```

### 4. Replace App State (`src/app.rs`)

- Replace `App.code: CodeResults` with `App.search_state: SearchState`
- Add `App.search_history: SearchHistory`
- Update render logic to match on `search_state` and extract results

### 5. Integrate Real API Calls (`src/app.rs`)

On Enter key in SearchPrompt screen:
- Clone `message_tx` and query string
- Spawn async task calling `api::fetch_code_results()`
- Set `search_state = SearchState::Loading { query }`
- Switch to SearchResults screen
- Handle `SearchComplete` message: transition to `Loaded`, update results

Pattern:
```rust
let tx = self.message_tx.clone();
let query = self.input_state.input.clone();
tokio::spawn(async move {
    match fetch_code_results(&query, None).await {
        Ok(data) => tx.send(AppMessage::SearchComplete { results: data, query }).unwrap(),
        Err(e) => tx.send(AppMessage::SearchError { error: e.to_string() }).unwrap(),
    }
});
```

### 6. Add Loading Indicators (`src/app.rs`)

Update `render_search_results_screen()`:
- When `SearchState::Loading`: Show spinner or "Loading..." text
- When `SearchState::LoadingMore`: Show "Loading more..." at bottom, keep results visible
- When `SearchState::Loaded`: Render results normally

### 7. Implement Search History (`src/app.rs`, `src/history.rs`)

**On startup:**
- Spawn task to load history
- Handle `HistoryLoaded` message to populate state

**In SearchPrompt screen:**
- Render history list below input
- Add Up/Down key handling to select items (`selected_idx`)
- Enter on selected item: populate input and trigger search
- Highlight selected item

**After successful search:**
- Spawn fire-and-forget task to save updated history

### 8. Add Pagination (`src/app.rs`, `src/widgets/search_results.rs`)

**Trigger detection in `SearchResultsState::handle_key()`:**
- After updating `selected_item_idx` on j/k/arrow keys
- Check: `selected_item_idx >= total_items - 5`
- Return enum indicating need for pagination:
  ```rust
  enum KeyHandleResult { Handled, NeedsPagination }
  ```

**In main event handler:**
- When `NeedsPagination` returned AND `search_state` is `Loaded` with `pagination.next`
- Extract page number from pagination info
- Spawn task to fetch next page
- Transition to `LoadingMore` state
- Handle `PaginationComplete`: append results, update pagination, return to `Loaded`

### 9. Error Handling

Per user requirement: Let errors crash with color_eyre
- Async tasks use `.unwrap()` or `?` which panics
- color_eyre catches and displays friendly error message
- No error recovery UI needed

## File Changes

### New Files
- `src/history.rs` - Search history persistence logic

### Modified Files
- `src/main.rs` - Add history module declaration
- `src/app.rs` - Major refactoring (event loop, state machine, message handling)
- `src/widgets/search_results.rs` - Return pagination trigger signal from `handle_key()`
- `Cargo.toml` - Add `dirs = "5.0"` dependency

### Reference Only (no changes)
- `src/api.rs` - Use existing `fetch_code_results(query, page)`
- `src/results.rs` - Use existing types

## Key Design Decisions

1. **Message Passing**: `mpsc` channel for clean ownership, no mutex contention
2. **Non-Blocking Loop**: `event::poll()` with `tokio::select!` for smooth UI (60 FPS)
3. **Explicit State Machine**: Type-safe states prevent invalid combinations
4. **History Format**: JSON array in `~/.config/ghs/history.json` (human-readable)
5. **Pagination Trigger**: Check on navigation within 5 items of end (user-initiated)
6. **Error Handling**: Panic and crash (simple, matches requirement)

## Implementation Order

1. History module (independent, testable separately)
2. Message infrastructure (channels, AppMessage enum)
3. Event loop refactoring (non-blocking poll, tokio::select!)
4. SearchState enum and state refactoring
5. Real API integration (spawn tasks, handle messages)
6. Loading indicators (render based on state)
7. Search history UI (load, render, navigate, save)
8. Pagination (trigger detection, auto-load next page)
9. Testing with real GitHub API
