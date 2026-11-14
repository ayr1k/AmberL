// SPDX-License-Identifier: MPL-2.0
use crate::auth::Secret;
use std::{
    collections::VecDeque,
    io::Read,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
pub(super) struct Reader {
    worker: thread::JoinHandle<()>,
    tail: Arc<Mutex<VecDeque<String>>>,
}
pub(super) fn drain(mut pipe: impl Read + Send + 'static, token: Secret) -> Reader {
    let tail = Arc::new(Mutex::new(VecDeque::new()));
    let shared = tail.clone();
    let worker = thread::spawn(move || {
        let mut buffer = [0; 4096];
        let mut line = Vec::new();
        let mut overflow = false;
        while let Ok(n) = pipe.read(&mut buffer) {
            if n == 0 {
                break;
            }
            for &byte in &buffer[..n] {
                if byte == b'\n' {
                    save(&shared, &line, overflow, &token);
                    line.clear();
                    overflow = false;
                } else if line.len() < 32768 {
                    line.push(byte);
                } else {
                    overflow = true;
                }
            }
        }
        if !line.is_empty() || overflow {
            save(&shared, &line, overflow, &token);
        }
    });
    Reader { worker, tail }
}
fn save(tail: &Mutex<VecDeque<String>>, line: &[u8], overflow: bool, token: &Secret) {
    // Drop an entire oversized line: truncating it could disclose a token prefix.
    let safe = if overflow {
        "[oversized game output omitted]".into()
    } else {
        String::from_utf8_lossy(line).replace(token.expose(), "[REDACTED]")
    };
    if let Ok(mut tail) = tail.lock() {
        if tail.len() == 40 {
            tail.pop_front();
        }
        tail.push_back(safe);
    }
}
pub(super) fn finish(reader: Reader) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !reader.worker.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    // A descendant might retain an inherited pipe. Do not block the launcher forever.
    reader
        .tail
        .lock()
        .map(|t| t.iter().cloned().collect())
        .unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_secrets_and_oversized_lines_do_not_escape_the_bounded_tail() {
        let secret = Secret::new("sensitive-token".into()).unwrap();
        let mut input = vec![b'a'; 4090];
        input.extend_from_slice(b"sensitive-token\n");
        input.extend(vec![b'x'; 32767]);
        input.extend_from_slice(b"sensitive-token\nend\n");
        let tail = finish(drain(std::io::Cursor::new(input), secret));
        assert!(!tail.join("\n").contains("sensitive"));
        assert!(tail[0].ends_with("[REDACTED]"));
        assert_eq!(tail[1], "[oversized game output omitted]");
        assert_eq!(tail[2], "end");
    }
}
