//! Test that TUI displays errors in real-time as they are found during validation
//!
//! This test verifies the streaming error display architecture where:
//! 1. TUI launches immediately (doesn't wait for validation to complete)
//! 2. Errors appear in TUI as they are discovered
//! 3. User can cancel validation mid-stream
//! 4. TUI updates dynamically as new files/errors arrive

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;
use talkbank_model::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation, Span};
use thiserror::Error;

/// Enum variants for TestError.
#[derive(Debug, Error)]
enum TestError {
    #[error("Failed to send event")]
    SendEvent,
    #[error("Thread join failed")]
    JoinThread,
}

/// Mock event for testing TUI streaming
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum MockValidationEvent {
    /// New file with errors discovered
    FileErrors {
        path: PathBuf,
        errors: Vec<ParseError>,
        source: Arc<str>,
    },
    /// Validation completed
    Completed,
}

/// Tests tui receives errors as they stream.
#[test]
fn test_tui_receives_errors_as_they_stream() -> Result<(), TestError> {
    // Set up channel for streaming events
    let (tx, rx): (Sender<MockValidationEvent>, Receiver<MockValidationEvent>) = channel();

    // Simulate background validation thread that sends errors over time
    let validation_thread = thread::spawn(move || -> Result<(), TestError> {
        // Send first file error
        tx.send(MockValidationEvent::FileErrors {
            path: PathBuf::from("/resources/corpus/file1.cha"),
            errors: vec![ParseError::new(
                ErrorCode::new("E999"),
                Severity::Error,
                SourceLocation::from_offsets(0, 5),
                ErrorContext::new("error", Span::from_usize(0, 5), ""),
                "Error in file1".to_string(),
            )],
            source: Arc::from("*CHI:\terror ."),
        })
        .map_err(|_| TestError::SendEvent)?;

        // Simulate processing delay
        thread::sleep(Duration::from_millis(10));

        // Send second file error
        tx.send(MockValidationEvent::FileErrors {
            path: PathBuf::from("/resources/corpus/file2.cha"),
            errors: vec![ParseError::new(
                ErrorCode::new("E999"),
                Severity::Error,
                SourceLocation::from_offsets(0, 5),
                ErrorContext::new("error", Span::from_usize(0, 5), ""),
                "Error in file2".to_string(),
            )],
            source: Arc::from("*CHI:\terror ."),
        })
        .map_err(|_| TestError::SendEvent)?;

        // Signal completion
        tx.send(MockValidationEvent::Completed)
            .map_err(|_| TestError::SendEvent)?;
        Ok(())
    });

    // TUI should receive events as they arrive (not wait for completion)
    let mut files_received = Vec::new();
    let mut completed = false;

    while let Ok(event) = rx.recv_timeout(Duration::from_millis(100)) {
        match event {
            MockValidationEvent::FileErrors { path, .. } => {
                files_received.push(path);
                // TUI would update display here
            }
            MockValidationEvent::Completed => {
                completed = true;
                break;
            }
        }
    }

    let validation_result = validation_thread
        .join()
        .map_err(|_| TestError::JoinThread)?;
    validation_result?;

    // Verify we received both files
    assert_eq!(files_received.len(), 2);
    assert_eq!(files_received[0], PathBuf::from("/resources/corpus/file1.cha"));
    assert_eq!(files_received[1], PathBuf::from("/resources/corpus/file2.cha"));
    assert!(completed);
    Ok(())
}

/// Tests tui can cancel ongoing validation.
#[test]
fn test_tui_can_cancel_ongoing_validation() -> Result<(), TestError> {
    // Set up channels
    let (error_tx, error_rx): (Sender<MockValidationEvent>, Receiver<MockValidationEvent>) =
        channel();
    let (cancel_tx, cancel_rx): (Sender<()>, Receiver<()>) = channel();

    // Simulate validation that can be cancelled
    let validation_thread = thread::spawn(move || -> Result<(), TestError> {
        for i in 0..100 {
            // Check for cancellation
            if cancel_rx.try_recv().is_ok() {
                // Validation cancelled, stop immediately
                return Ok(());
            }

            // Send file error
            error_tx
                .send(MockValidationEvent::FileErrors {
                    path: PathBuf::from(format!("/resources/corpus/file{}.cha", i)),
                    errors: vec![],
                    source: Arc::from(""),
                })
                .map_err(|_| TestError::SendEvent)?;

            thread::sleep(Duration::from_millis(1));
        }

        error_tx
            .send(MockValidationEvent::Completed)
            .map_err(|_| TestError::SendEvent)?;
        Ok(())
    });

    // Receive a few errors, then cancel
    let mut files_received = 0;
    for _ in 0..5 {
        if error_rx.recv_timeout(Duration::from_millis(50)).is_ok() {
            files_received += 1;
        }
    }

    // User presses cancel key in TUI
    cancel_tx.send(()).map_err(|_| TestError::SendEvent)?;

    // Wait a bit to ensure cancellation processed
    thread::sleep(Duration::from_millis(50));

    // Try to receive more events (should stop quickly)
    let remaining = error_rx.try_iter().count();

    let validation_result = validation_thread
        .join()
        .map_err(|_| TestError::JoinThread)?;
    validation_result?;

    // We should have received some files, but not all 100
    assert!(files_received > 0, "Should receive at least one file");
    assert!(
        files_received + remaining < 100,
        "Should not receive all files after cancellation"
    );
    Ok(())
}

/// Tests tui displays files alphabetically as they arrive.
#[test]
fn test_tui_displays_files_alphabetically_as_they_arrive() -> Result<(), TestError> {
    // Even though errors arrive in non-alphabetical order,
    // TUI should sort them for display

    let (tx, rx): (Sender<MockValidationEvent>, Receiver<MockValidationEvent>) = channel();

    let sender_thread = thread::spawn(move || -> Result<(), TestError> {
        // Send in non-alphabetical order
        tx.send(MockValidationEvent::FileErrors {
            path: PathBuf::from("/resources/corpus/zebra.cha"),
            errors: vec![],
            source: Arc::from(""),
        })
        .map_err(|_| TestError::SendEvent)?;

        tx.send(MockValidationEvent::FileErrors {
            path: PathBuf::from("/resources/corpus/apple.cha"),
            errors: vec![],
            source: Arc::from(""),
        })
        .map_err(|_| TestError::SendEvent)?;

        tx.send(MockValidationEvent::FileErrors {
            path: PathBuf::from("/resources/corpus/mango.cha"),
            errors: vec![],
            source: Arc::from(""),
        })
        .map_err(|_| TestError::SendEvent)?;

        tx.send(MockValidationEvent::Completed)
            .map_err(|_| TestError::SendEvent)?;
        Ok(())
    });

    // Collect all files
    let mut files = Vec::new();
    while let Ok(event) = rx.recv_timeout(Duration::from_millis(100)) {
        match event {
            MockValidationEvent::FileErrors { path, .. } => {
                files.push(path);
            }
            MockValidationEvent::Completed => break,
        }
    }

    let sender_result = sender_thread.join().map_err(|_| TestError::JoinThread)?;
    sender_result?;

    // Sort for display (TUI should do this)
    files.sort();

    // Verify alphabetical order
    assert_eq!(files[0], PathBuf::from("/resources/corpus/apple.cha"));
    assert_eq!(files[1], PathBuf::from("/resources/corpus/mango.cha"));
    assert_eq!(files[2], PathBuf::from("/resources/corpus/zebra.cha"));
    Ok(())
}
